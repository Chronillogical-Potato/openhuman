//! Agent-facing browser backed by the TinyBrowser module.
use crate::modules::browser::BrowserClient;
use crate::security::approval::{ApprovalGate, GateOutcome};
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex as StdMutex, OnceLock},
    time::{Duration, Instant},
};
use tinybrowser_bus::{
    Action, DownloadState, DownloadWaitRequest, LocateBy, Locator, NavigateRequest, ReadRequest,
    ScrollDirection, SessionId, SessionOptions, Snapshot, SnapshotRequest, Target, WaitState,
};
use tinybrowser_control::{BrowserControl, BrowserControlError, TaskRequest, TaskStatus};
use tinytools::{Tool, ToolCallOptions, ToolResult, ToolRunContext};
use tokio::sync::Mutex;

struct Pending {
    session: SessionId,
    action: Action,
    url: String,
    token: String,
}

const MAX_THREAD_SESSIONS: usize = 6;
const SESSION_IDLE_TTL: Duration = Duration::from_secs(30 * 60);

struct ThreadSession {
    id: SessionId,
    client: Arc<BrowserClient>,
    last_used: Instant,
    config_fingerprint: String,
    bound_origin: Option<String>,
}

static THREAD_SESSIONS: OnceLock<Mutex<HashMap<String, ThreadSession>>> = OnceLock::new();

fn thread_sessions() -> &'static Mutex<HashMap<String, ThreadSession>> {
    THREAD_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn browser_session_fingerprint(client: &BrowserClient) -> String {
    let config = client.config();
    // Include the complete browser settings so a newly assembled tool never
    // inherits a Chrome session opened with older profile, viewport, timeout,
    // download, or module settings. The shared web allowlist and override also
    // determine the module's allowed_origins at OPEN_SESSION time.
    let allow_all = matches!(
        std::env::var("OPENHUMAN_BROWSER_ALLOW_ALL").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    );
    let bytes = serde_json::to_vec(&json!({
        "browser": &config.browser,
        "allowed_domains": &config.http_request.allowed_domains,
        "allow_all": allow_all,
    }))
    .expect("browser configuration is JSON serializable");
    format!("{:x}", Sha256::digest(bytes))
}

fn requires_rebind(bound: Option<&str>, requested: Option<&str>) -> bool {
    requested.is_some_and(|origin| bound != Some(origin))
}

fn evict_thread_sessions(
    sessions: &mut HashMap<String, ThreadSession>,
    now: Instant,
    reserve_slot: bool,
) -> Vec<ThreadSession> {
    let mut removed = Vec::new();
    let expired = sessions
        .iter()
        .filter(|(_, value)| now.duration_since(value.last_used) >= SESSION_IDLE_TTL)
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    for key in expired {
        if let Some(value) = sessions.remove(&key) {
            removed.push(value);
        }
    }
    while reserve_slot && sessions.len() >= MAX_THREAD_SESSIONS {
        let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, value)| value.last_used)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        if let Some(value) = sessions.remove(&oldest) {
            removed.push(value);
        }
    }
    removed
}

impl Pending {
    fn matches(&self, args: &Value) -> bool {
        args["token"].as_str() == Some(self.token.as_str())
            && serde_json::to_value(&self.action).ok().as_ref() == Some(&args["pending_action"])
    }
}

struct BudgetedBrowser<'a> {
    client: &'a BrowserClient,
    security: &'a SecurityPolicy,
}

fn needs_host_confirmation(action: &Action) -> bool {
    matches!(
        action,
        Action::Click { .. }
            | Action::DoubleClick { .. }
            | Action::Fill { .. }
            | Action::Type { .. }
            | Action::Press { .. }
            | Action::Select { .. }
            | Action::Check { .. }
    )
}

async fn approve_browser_action(
    client: &BrowserClient,
    session: &SessionId,
    action: &Action,
    force: bool,
) -> anyhow::Result<()> {
    if !force && !needs_host_confirmation(action) {
        return Ok(());
    }
    let gate = ApprovalGate::try_global().ok_or_else(|| {
        anyhow::anyhow!("[policy-denied] Browser action needs an interactive host approval gate")
    })?;
    let before = client
        .read_page(
            session,
            ReadRequest {
                max_chars: 1,
                ..ReadRequest::default()
            },
        )
        .await?;
    let action_json = serde_json::to_value(action)?;
    let kind = action_json["action"].as_str().unwrap_or("action");
    let origin = reqwest::Url::parse(&before.url)
        .ok()
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|| "unknown page".into());
    let digest = Sha256::digest(serde_json::to_vec(
        &json!({"url": before.url, "action": action_json}),
    )?);
    let target_ref = match action {
        Action::Click {
            target: Target::Ref { value },
            ..
        }
        | Action::DoubleClick {
            target: Target::Ref { value },
        }
        | Action::Fill {
            target: Target::Ref { value },
            ..
        }
        | Action::Select {
            target: Target::Ref { value },
            ..
        }
        | Action::Check {
            target: Target::Ref { value },
            ..
        } => Some(value.as_str()),
        Action::Type {
            target: Some(Target::Ref { value }),
            ..
        } => Some(value.as_str()),
        _ => None,
    };
    let input_detail = match action {
        Action::Fill { value, .. } => format!(" ({} input characters)", value.chars().count()),
        Action::Type { text, .. } => format!(" ({} input characters)", text.chars().count()),
        Action::Press { key } => format!(" ({key})"),
        _ => String::new(),
    };
    let target_detail = target_ref
        .map(|reference| format!(" @{reference}"))
        .unwrap_or_default();
    let digest_hex = format!("{digest:x}");
    let display_target = format!(
        "{kind}{target_detail}{input_detail} on {origin} [action {}] — review the browser tool input before allowing",
        &digest_hex[..12]
    );
    let summary = format!("Browser {display_target}");
    // A digest binds the prompt to the complete action and URL without
    // persisting form values, selectors, or sensitive URL query parameters.
    let args = json!({"action": kind, "origin": origin, "target": display_target,
        "target_ref": target_ref, "exact_action_sha256": digest_hex});
    match gate.intercept_forced("browser", &summary, args).await {
        GateOutcome::Allow => {}
        GateOutcome::Deny { reason } => anyhow::bail!("{reason}"),
    }
    let after = client
        .read_page(
            session,
            ReadRequest {
                max_chars: 1,
                ..ReadRequest::default()
            },
        )
        .await?;
    if after.url != before.url {
        anyhow::bail!("Browser page changed during host approval");
    }
    Ok(())
}

impl BrowserControl for BudgetedBrowser<'_> {
    async fn snapshot(
        &self,
        session: &SessionId,
        request: &SnapshotRequest,
    ) -> Result<Snapshot, BrowserControlError> {
        BrowserControl::snapshot(self.client, session, request).await
    }

    async fn perform(
        &self,
        session: &SessionId,
        action: &Action,
    ) -> Result<tinybrowser_bus::ActionOutcome, BrowserControlError> {
        approve_browser_action(self.client, session, action, false)
            .await
            .map_err(|error| BrowserControlError {
                name: "HostApprovalDenied".into(),
                message: error.to_string(),
            })?;
        if !self.security.record_action() {
            return Err(BrowserControlError {
                name: "ActionBudgetExceeded".into(),
                message: "Browser action budget exceeded".into(),
            });
        }
        BrowserControl::perform(self.client, session, action).await
    }
}

pub struct BrowserTool {
    security: Arc<SecurityPolicy>,
    client: Arc<BrowserClient>,
    session: Mutex<Option<SessionId>>,
    bound_origin: Mutex<Option<String>>,
    pending: Mutex<Option<Pending>>,
    thread_key: StdMutex<Option<String>>,
    max_steps: usize,
}

impl BrowserTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        client: Arc<BrowserClient>,
        max_steps: usize,
    ) -> Self {
        Self {
            security,
            client,
            session: Mutex::new(None),
            bound_origin: Mutex::new(None),
            pending: Mutex::new(None),
            thread_key: StdMutex::new(None),
            max_steps: max_steps.clamp(1, 100),
        }
    }

    async fn session(&self) -> anyhow::Result<SessionId> {
        self.session_for_url(None).await
    }

    async fn session_for_url(&self, url: Option<&str>) -> anyhow::Result<SessionId> {
        let requested_origin = url
            .map(|url| self.client.explicit_origin(url))
            .transpose()?
            .flatten();
        let mut held = self.session.lock().await;
        let mut bound = self.bound_origin.lock().await;
        let thread_key = self.thread_key.lock().ok().and_then(|key| key.clone());
        if let Some(key) = thread_key {
            let mut sessions = thread_sessions().lock().await;
            let now = Instant::now();
            let config_fingerprint = browser_session_fingerprint(&self.client);
            let expired = evict_thread_sessions(&mut sessions, now, false);
            for value in expired {
                let _ = value.client.close_session(&value.id).await;
            }
            if sessions.get(&key).is_some_and(|value| {
                value.config_fingerprint != config_fingerprint
                    || requires_rebind(value.bound_origin.as_deref(), requested_origin.as_deref())
            }) {
                let stale = sessions.get(&key).expect("entry checked above");
                *held = None;
                *bound = None;
                *self.pending.lock().await = None;
                // The previous module session retains its original allowed
                // origins. Keep its entry until close succeeds so a failed
                // close is retried before any replacement can open.
                stale.client.close_session(&stale.id).await?;
                sessions.remove(&key);
            }
            if let Some(value) = sessions.get_mut(&key) {
                value.last_used = now;
                *held = Some(value.id.clone());
                *bound = value.bound_origin.clone();
                return Ok(value.id.clone());
            }
            // The shared entry may have been evicted while this tool kept its
            // local handle. Never return that closed session to a later turn.
            *held = None;
            *bound = None;
            let capacity = evict_thread_sessions(&mut sessions, now, true);
            for value in capacity {
                let _ = value.client.close_session(&value.id).await;
            }
            let id = self.open_scoped_session(url).await?;
            sessions.insert(
                key,
                ThreadSession {
                    id: id.clone(),
                    client: self.client.clone(),
                    last_used: now,
                    config_fingerprint,
                    bound_origin: requested_origin.clone(),
                },
            );
            *held = Some(id.clone());
            *bound = requested_origin;
            return Ok(id);
        }
        if let Some(id) = held.as_ref() {
            if !requires_rebind(bound.as_deref(), requested_origin.as_deref()) {
                return Ok(id.clone());
            }
            self.client.close_session(id).await?;
            *held = None;
            *bound = None;
            *self.pending.lock().await = None;
        }
        let id = self.open_scoped_session(url).await?;
        *held = Some(id.clone());
        *bound = requested_origin;
        Ok(id)
    }

    async fn open_scoped_session(&self, url: Option<&str>) -> anyhow::Result<SessionId> {
        Ok(match url {
            Some(url) => self.client.open_session_for_url(url).await?.id,
            None => {
                self.client
                    .open_session(SessionOptions::default())
                    .await?
                    .id
            }
        })
    }

    async fn close(&self) -> anyhow::Result<Value> {
        let thread_key = self.thread_key.lock().ok().and_then(|key| key.clone());
        let mut held = self.session.lock().await;
        *self.bound_origin.lock().await = None;
        *self.pending.lock().await = None;
        let id = if let Some(key) = thread_key {
            let mut sessions = thread_sessions().lock().await;
            // A stale tool must not remove or close a replacement session
            // opened by another tool for the same conversation.
            let owns_entry = sessions
                .get(&key)
                .is_some_and(|entry| held.as_ref().is_none_or(|id| entry.id == *id));
            let removed = owns_entry.then(|| sessions.remove(&key)).flatten();
            held.take()
                .filter(|id| removed.as_ref().is_some_and(|entry| entry.id == *id))
                .or_else(|| removed.map(|entry| entry.id))
        } else {
            held.take()
        };
        drop(held);
        if let Some(id) = id {
            self.client.close_session(&id).await?;
        }
        Ok(json!({"closed": true}))
    }

    async fn task(&self, id: &SessionId, args: &Value) -> anyhow::Result<Value> {
        let mut request = TaskRequest::new(required(args, "goal")?);
        if let Some(inputs) = args["inputs"].as_object() {
            let values = inputs
                .iter()
                .map(|(k, v)| {
                    Ok((
                        k.clone(),
                        v.as_str()
                            .ok_or_else(|| anyhow::anyhow!("Task input '{k}' must be text"))?
                            .to_owned(),
                    ))
                })
                .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
            request = request.with_inputs(values);
        }
        let browser = BudgetedBrowser {
            client: &self.client,
            security: &self.security,
        };
        let result = crate::modules::browser_task::run(
            &browser,
            self.client.config(),
            id,
            request.clone(),
            self.max_steps,
        )
        .await
        .map_err(anyhow::Error::msg)?;
        // The module can follow a link or redirect outside the configured
        // origins. The host must refuse to report that state as a successful
        // task, even though strict prevention requires module-level interception.
        self.client
            .read_page(
                id,
                ReadRequest {
                    max_chars: 1,
                    ..ReadRequest::default()
                },
            )
            .await?;
        let steps = result
            .steps
            .iter()
            .map(|step| {
                json!({
                    "step": step.step,
                    "operation": format!("{:?}", step.decision.operation),
                    "page_changed": step.page_changed,
                    "outcome": format!("{:?}", step.outcome),
                })
            })
            .collect::<Vec<_>>();
        let mut output = json!({"status": format!("{:?}", result.status), "steps": steps, "final_snapshot": result.final_snapshot});
        if result.status == TaskStatus::NeedsConfirmation {
            let decision = result
                .pending
                .ok_or_else(|| anyhow::anyhow!("No pending Jev decision"))?;
            let action = decision
                .to_action(&request, 250)?
                .ok_or_else(|| anyhow::anyhow!("No pending action"))?;
            let token = uuid::Uuid::new_v4().to_string();
            *self.pending.lock().await = Some(Pending {
                session: id.clone(),
                action: action.clone(),
                url: result.final_snapshot.url,
                token: token.clone(),
            });
            output["pending"] = json!({"action": action, "token": token, "approval": "Call confirm_pending with this token and pending_action to request host approval for this exact action"});
        } else {
            *self.pending.lock().await = None;
        }
        Ok(output)
    }

    async fn confirm_pending(&self, args: &Value) -> anyhow::Result<Value> {
        let mut slot = self.pending.lock().await;
        let held = slot
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No pending browser action"))?;
        if !held.matches(args) {
            anyhow::bail!("Pending browser action does not match the approved request");
        }
        let pending = slot.take().expect("pending checked above");
        drop(slot);
        let page = self
            .client
            .read_page(
                &pending.session,
                ReadRequest {
                    max_chars: 1,
                    ..ReadRequest::default()
                },
            )
            .await?;
        if page.url != pending.url {
            anyhow::bail!("Browser page changed during confirmation");
        }
        approve_browser_action(&self.client, &pending.session, &pending.action, true).await?;
        Ok(serde_json::to_value(
            self.client
                .perform(&pending.session, pending.action)
                .await?,
        )?)
    }

    async fn run(&self, args: &Value) -> anyhow::Result<Value> {
        let verb = required(args, "action")?;
        if verb == "close" {
            return self.close().await;
        }
        if verb == "confirm_pending" {
            return self.confirm_pending(args).await;
        }
        let starting_url = match verb {
            "open" => Some(required(args, "url")?),
            "task" => args["url"].as_str().filter(|url| !url.trim().is_empty()),
            _ => None,
        };
        let id = self.session_for_url(starting_url).await?;
        match verb {
            "open" => Ok(serde_json::to_value(
                self.client
                    .navigate(&id, NavigateRequest::new(required(args, "url")?))
                    .await?,
            )?),
            "snapshot" => Ok(serde_json::to_value(
                self.client
                    .snapshot(
                        &id,
                        SnapshotRequest {
                            interactive_only: args["interactive_only"].as_bool().unwrap_or(false),
                            compact: args["compact"].as_bool().unwrap_or(true),
                            depth: args["depth"].as_u64().and_then(|v| u32::try_from(v).ok()),
                            max_chars: 50_000,
                            ..SnapshotRequest::default()
                        },
                    )
                    .await?,
            )?),
            "read_page" => Ok(serde_json::to_value(
                self.client
                    .read_page(
                        &id,
                        ReadRequest {
                            max_chars: 50_000,
                            ..ReadRequest::default()
                        },
                    )
                    .await?,
            )?),
            "get_title" | "get_url" => {
                let p = self
                    .client
                    .read_page(
                        &id,
                        ReadRequest {
                            max_chars: 1,
                            ..ReadRequest::default()
                        },
                    )
                    .await?;
                Ok(if verb == "get_title" {
                    json!({"title":p.title})
                } else {
                    json!({"url":p.url})
                })
            }
            "task" => {
                if let Some(url) = args["url"].as_str().filter(|url| !url.trim().is_empty()) {
                    self.client.navigate(&id, NavigateRequest::new(url)).await?;
                } else {
                    let page = self
                        .client
                        .read_page(
                            &id,
                            ReadRequest {
                                max_chars: 1,
                                ..ReadRequest::default()
                            },
                        )
                        .await?;
                    if page.url == "about:blank" {
                        anyhow::bail!(
                            "No browser page is open. Use browser action=open with an HTTPS URL or include url on task"
                        );
                    }
                }
                self.task(&id, args).await
            }
            "list_downloads" => Ok(serde_json::to_value(
                self.client.list_downloads(&id).await?,
            )?),
            "wait_download" => {
                let d = self
                    .client
                    .wait_download(
                        &id,
                        DownloadWaitRequest {
                            timeout_ms: args["timeout_ms"].as_u64(),
                        },
                    )
                    .await?;
                if !matches!(d.state, DownloadState::Completed) {
                    anyhow::bail!("Download did not complete: {:?}", d.state);
                }
                let path = d
                    .path
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("No permitted download path"))?;
                let file = tokio::fs::metadata(path).await?;
                if !file.is_file() || file.len() == 0 || file.len() != d.received_bytes {
                    anyhow::bail!("Downloaded file bytes differ from tracked download");
                }
                Ok(serde_json::to_value(d)?)
            }
            _ => {
                let action = parse_action(args)?;
                approve_browser_action(&self.client, &id, &action, false).await?;
                Ok(serde_json::to_value(
                    self.client.perform(&id, action).await?,
                )?)
            }
        }
    }
}

fn required<'a>(args: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Missing '{key}' parameter"))
}

fn parse_action(args: &Value) -> anyhow::Result<Action> {
    let target = || required(args, "selector").map(Target::parse);
    Ok(match required(args, "action")? {
        "click" => Action::Click {
            target: target()?,
            new_tab: false,
        },
        "fill" => Action::Fill {
            target: target()?,
            value: required(args, "value")?.into(),
        },
        "type" => Action::Type {
            target: args["selector"].as_str().map(Target::parse),
            text: required(args, "text")?.into(),
            delay_ms: None,
        },
        "get_text" => Action::GetText { target: target()? },
        "is_visible" => Action::IsVisible { target: target()? },
        "hover" => Action::Hover { target: target()? },
        "press" => Action::Press {
            key: required(args, "key")?.into(),
        },
        "scroll" => Action::Scroll {
            direction: match required(args, "direction")? {
                "up" => ScrollDirection::Up,
                "down" => ScrollDirection::Down,
                "left" => ScrollDirection::Left,
                "right" => ScrollDirection::Right,
                x => anyhow::bail!("Invalid direction: {x}"),
            },
            pixels: args["pixels"].as_u64().and_then(|v| u32::try_from(v).ok()),
            target: None,
        },
        "wait" => Action::WaitFor {
            target: args["selector"].as_str().map(Target::parse),
            text: args["text"].as_str().map(str::to_owned),
            state: WaitState::Visible,
            ms: args["ms"].as_u64(),
            timeout_ms: args["timeout_ms"].as_u64(),
        },
        "find" => {
            let by = match required(args, "by")? {
                "role" => LocateBy::Role,
                "text" => LocateBy::Text,
                "label" => LocateBy::Label,
                "placeholder" => LocateBy::Placeholder,
                "testid" => LocateBy::TestId,
                x => anyhow::bail!("Invalid locator: {x}"),
            };
            let target = Target::locator(Locator::new(by, required(args, "value")?));
            match required(args, "find_action")? {
                "click" => Action::Click {
                    target,
                    new_tab: false,
                },
                "fill" => Action::Fill {
                    target,
                    value: required(args, "fill_value")?.into(),
                },
                "text" => Action::GetText { target },
                "hover" => Action::Hover { target },
                x => anyhow::bail!("Invalid find action: {x}"),
            }
        }
        x => anyhow::bail!("Unsupported browser action: {x}"),
    })
}

#[async_trait]
impl Tool for BrowserTool {
    fn exposure(&self) -> tinytools::ToolExposure {
        tinytools::ToolExposure::Deferred
    }
    fn name(&self) -> &str {
        "browser"
    }
    fn description(&self) -> &str {
        concat!(
            "TinyBrowser website operations. Call action=open with the starting URL in this tool ",
            "before snapshot, read_page, or task. browser_open is a separate one-shot session. ",
            "Then use snapshot for current ",
            "accessibility refs such as @e1 and read_page for visible prose. Refs expire after ",
            "navigation or a new snapshot; take a fresh snapshot instead of guessing a stale ref. ",
            "For multi-step work, use task with an optional starting url, an observable final-state goal, and named exact ",
            "input values. Jev chooses among current refs and input names; it does not invent text ",
            "to enter. Check task status, steps, and final_snapshot before claiming completion. ",
            "DoneUnconfirmed needs direct inspection; Stuck calls for one fresh snapshot and ",
            "diagnosis; Blocked and Budget are stopping conditions. NeedsConfirmation returns an ",
            "exact pending action and token. Use confirm_pending only through the host approval ",
            "mechanism for that exact action. Direct consequential clicks and key presses also ",
            "require host approval. For downloads, inspect list_downloads and call wait_download; ",
            "success is reported only after the tracked file path and byte count are verified. ",
            "Keep the same conversation session across turns and close it when finished. Navigation ",
            "obeys the shared allowed websites list; do not try to evade a blocked destination."
        )
    }
    fn parameters_schema(&self) -> Value {
        json!({"type":"object","properties":{
        "action":{"type":"string","enum":["open","snapshot","read_page","click","fill","type","get_text","get_title","get_url","wait","press","hover","scroll","is_visible","find","task","confirm_pending","list_downloads","wait_download","close"]},
        "url":{"type":"string","description":"Starting HTTPS URL for open or an optional starting URL for task"},"selector":{"type":"string"},"value":{"type":"string"},"text":{"type":"string"},"key":{"type":"string"},"direction":{"type":"string"},"pixels":{"type":"integer"},"ms":{"type":"integer"},"timeout_ms":{"type":"integer"},"interactive_only":{"type":"boolean"},"compact":{"type":"boolean"},"depth":{"type":"integer"},"by":{"type":"string"},"find_action":{"type":"string"},"fill_value":{"type":"string"},"goal":{"type":"string"},"inputs":{"type":"object","additionalProperties":{"type":"string"}},"token":{"type":"string","description":"Token returned with the exact pending action"},"pending_action":{"type":"object","description":"Exact pending action for host approval; must match the task result"}
    },"required":["action"]})
    }
    fn external_effect_with_args(&self, args: &Value) -> bool {
        // All mutating actions, including Jev task steps, use the forced gate
        // immediately before perform. Declaring an outer effect would park the
        // same call twice and cannot cover the actions chosen inside `task`.
        let _ = args;
        false
    }
    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        if !self.security.can_act() {
            return Ok(ToolResult::error(
                "[policy-blocked] Action blocked: autonomy is read-only",
            ));
        }
        if args["action"] != "task" && !self.security.record_action() {
            return Ok(ToolResult::error("Action blocked: rate limit exceeded"));
        }
        match self.run(&args).await {
            Ok(v) => Ok(ToolResult::success(serde_json::to_string_pretty(&v)?)),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }

    async fn execute_with_context(
        &self,
        args: Value,
        _options: ToolCallOptions,
        context: Option<&dyn ToolRunContext>,
    ) -> anyhow::Result<ToolResult> {
        if let Some(thread_id) = context.and_then(ToolRunContext::thread_id) {
            let key = format!(
                "{}:{thread_id}",
                self.client.config().workspace_dir.display()
            );
            if let Ok(mut held) = self.thread_key.lock() {
                *held = Some(key);
            }
        }
        self.execute(args).await
    }
}

impl Drop for BrowserTool {
    fn drop(&mut self) {
        if self.thread_key.lock().ok().is_some_and(|key| key.is_some()) {
            // A later turn in this conversation reuses the module session.
            // Explicit `close` removes it; module shutdown owns final cleanup.
            return;
        }
        if let Ok(mut held) = self.session.try_lock() {
            if let Some(id) = held.take() {
                let client = self.client.clone();
                if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                    runtime.spawn(async move {
                        let _ = client.close_session(&id).await;
                    });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "browser_tinybrowser_tests.rs"]
mod tests;
