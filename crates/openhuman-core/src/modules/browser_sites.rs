//! What browser tasks learn about a site, kept for its next task.
//!
//! TinyComputer returns, with every finished task, the plan it ran
//! (`TaskReport.flow`) and the elements its steps found
//! (`TaskReport.learned`), and takes both back on `StartTask`: a ready `flow`
//! skips planning, and `memory` lets a step confirm a remembered element with
//! one yes/no question instead of searching the page for it. The module keeps
//! nothing between tasks; the host keeps it here, per site, in
//! `<workspace>/state/computer/sites/<site>.json`, readable by its owner
//! alone.
//!
//! - Only a task that finished (`done`, or stopped at its payment checkpoint)
//!   teaches anything.
//! - A plan is reused only for the same goal, with the same facts, on the
//!   same site. It is kept only when its run needed no rescue and its steps
//!   hold no fact value the goal does not. A task that fails, or needs a
//!   rescue, on a reused plan forgets it, and so does a module that refuses
//!   it, the task then being planned afresh.
//! - An element whose name, key or place holds a fact value, however the
//!   module spells it, or whose name reads like page text rather than a
//!   control's label, is not kept.
//! - What no finished run has used for 30 days is dropped, and a site's file
//!   goes 30 days after it last changed.
//!
//! Nothing here reaches the agent's memory or its prompts: plans and elements
//! go to the module alone. `browser.learn_from_tasks` turns learning off, and
//! `modules.browser_forget_sites` forgets one site or every site, also for
//! tasks still running there.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::future::Future;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tinycomputer_bus::agent::{StartTaskRequest, TaskId, TaskReport, TaskStatus, TaskView};
use tinycomputer_bus::{Flow, GroundingHint};
use tokio::io::AsyncWriteExt;

use super::browser_task::BrowserTask;
use crate::config::Config;

/// Most elements kept per site; the oldest go first.
const MAX_HINTS: usize = 50;
/// Most plans kept per site; the one finished on longest ago goes first.
const MAX_PLANS: usize = 20;
/// How long a plan or an element is kept without a finished run using it,
/// and a site's file without a change.
const KEEP_SECS: u64 = 30 * 24 * 60 * 60;
/// Longest element name kept: a longer one is page text, not a label.
const MAX_NAME_CHARS: usize = 80;
/// Shortest fact value looked for: a shorter one (a count, an initial) is in
/// too many labels to tell anything.
const MIN_FACT_CHARS: usize = 3;
/// How many started tasks are followed at once; the oldest is let go first.
const FOLLOWED: usize = 64;
/// Largest site file read: anything bigger was not written here.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// A pending `TaskReport` call.
type ReportFetch<'a> = Pin<Box<dyn Future<Output = Result<TaskReport, String>> + Send + 'a>>;

/// What finished tasks left about one site.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct SiteMemory {
    #[serde(default)]
    plans: Vec<SavedPlan>,
    #[serde(default)]
    hints: Vec<SavedHint>,
}

/// A plan that reached a goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedPlan {
    /// The goal it reached, as [`goal_key`] spells it.
    goal: String,
    /// The facts it ran with, as [`facts_id`] spells them.
    #[serde(default)]
    facts_id: String,
    flow: Flow,
    /// When a run last finished on it, in seconds since the epoch.
    ok_at: u64,
    /// How many finished runs it served.
    runs: u32,
}

/// An element a finished task's step found.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedHint {
    hint: GroundingHint,
    /// When a finished run last found it, in seconds since the epoch.
    ok_at: u64,
}

/// A started task, followed for what it may teach once it ends.
#[derive(Debug, Clone)]
pub(crate) struct Following {
    /// The workspace whose site files it teaches.
    workspace: PathBuf,
    site: String,
    goal: String,
    facts_id: String,
    /// Whether it runs a plan saved here, forgotten if the run fails on it.
    reused_plan: bool,
    /// Its fact values, lower-cased and as words, none of which may be kept.
    facts: Vec<String>,
}

impl Following {
    /// Whether the task runs a plan saved for its goal and facts.
    pub(crate) fn reuses_plan(&self) -> bool {
        self.reused_plan
    }
}

/// The site `url` belongs to: its host, lower-cased and without a leading
/// `www.`; `None` for an address that is not `http(s)` or names no plain
/// host.
#[must_use]
pub fn site_of(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    site_named(parsed.host_str()?)
}

/// `host` as a site name, or `None` when it is not a plain host name.
fn site_named(host: &str) -> Option<String> {
    let host = host.trim().to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let plain = !host.is_empty()
        && !host.starts_with('.')
        && host
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'));
    plain.then(|| host.to_owned())
}

/// A goal as plans are matched by: lower-cased, its whitespace collapsed.
fn goal_key(goal: &str) -> String {
    goal.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// `facts`, names and values, as one digest, so a site file holds no value.
/// A plan is reused only with the facts it ran with: one chosen by a value
/// (a size, a city) never meets another, and none it reads goes unsupplied.
fn facts_id(facts: &BTreeMap<String, String>) -> String {
    let bytes = serde_json::to_vec(facts).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

/// `text` as the module keys a step by: its words, lower-cased, with
/// anything between them turned to single spaces.
fn words(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The fact values worth looking for, lower-cased, each also as its words:
/// the module keys a step by its text with the values filled in, so
/// `asha.r@example.com` is kept as `asha r example com`.
fn fact_values<'a>(values: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let mut found = Vec::new();
    for value in values {
        let value = value.trim().to_lowercase();
        let spelled = words(&value);
        for form in [value, spelled] {
            if form.chars().count() >= MIN_FACT_CHARS && !found.contains(&form) {
                found.push(form);
            }
        }
    }
    found
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Records that Browser settings switched learning on or off for
/// `workspace`. A tool keeps the config it was built with, and a channel's
/// agent is built once, so the switch outranks that config.
pub(crate) fn note_switch(workspace: &Path, on: bool) {
    switched().insert(workspace.to_path_buf(), on);
}

/// Whether tasks under `config` learn: the latest switch in Browser
/// settings, else `browser.learn_from_tasks`.
fn learning(config: &Config) -> bool {
    switched()
        .get(&config.workspace_dir)
        .copied()
        .unwrap_or(config.browser.learn_from_tasks)
}

/// Fills `request` from what finished tasks on `task`'s site left: the plan
/// saved for its goal and facts, unless the caller brought a flow of its own,
/// and the elements found there. Returns the task to follow, or `None` when
/// learning is off or the task names no site.
pub(crate) async fn apply(
    config: &Config,
    task: &BrowserTask,
    request: &mut StartTaskRequest,
) -> Option<Following> {
    sweep(config, now()).await;
    if !learning(config) {
        return None;
    }
    let site = task.site.clone()?;
    let goal = goal_key(&task.goal);
    let facts_id = facts_id(&task.facts);
    let memory = load(config, &site, now()).await;
    let mut reused_plan = false;
    if request.flow.is_none() {
        if let Some(plan) = memory
            .plans
            .iter()
            .find(|plan| plan.serves(&goal, &facts_id))
        {
            request.flow = Some(plan.flow.clone());
            reused_plan = true;
        }
    }
    request.memory = memory
        .hints
        .iter()
        .map(|saved| saved.hint.clone())
        .collect();
    tracing::debug!(
        reused_plan,
        hints = request.memory.len(),
        "[browser-sites] applied what the site's finished tasks left"
    );
    Some(Following {
        workspace: config.workspace_dir.clone(),
        site,
        goal,
        facts_id,
        reused_plan,
        facts: fact_values(task.facts.values()),
    })
}

/// Follows task `id` until it ends, to learn from how it did; with `None`,
/// follows no task by that id. The module numbers its tasks afresh each time
/// it is set up again, so an id can come back for another task.
pub(crate) fn follow(id: &TaskId, following: Option<Following>) {
    let mut followed = followed();
    followed.retain(|(known, _)| known != id);
    if let Some(following) = following {
        followed.push_back((id.clone(), following));
    }
    while followed.len() > FOLLOWED {
        followed.pop_front();
    }
}

/// Adds `inputs`, the values a paused task `id` is given, to the values it
/// may not keep.
pub(crate) fn note_inputs(id: &TaskId, inputs: &BTreeMap<String, String>) {
    if inputs.is_empty() {
        return;
    }
    let mut followed = followed();
    if let Some((_, following)) = followed.iter_mut().find(|(known, _)| known == id) {
        for value in fact_values(inputs.values()) {
            if !following.facts.contains(&value) {
                following.facts.push(value);
            }
        }
    }
}

/// Forgets the saved plan `following` was to reuse, which the module refused,
/// so the task is planned afresh.
pub(crate) async fn refused(config: &Config, following: &mut Following) {
    let plan_of: &Following = following;
    update(config, &plan_of.site, |memory| {
        memory.forget_plan(plan_of);
        true
    })
    .await;
    following.reused_plan = false;
    tracing::debug!("[browser-sites] forgot a saved plan the module refused");
}

/// Learns from a followed task that has ended: what it found and the plan it
/// ran when it finished, or that the plan it reused failed. A task still
/// running or paused stays followed.
pub(crate) async fn learn(config: &Config, view: &TaskView) {
    learn_with(config, view, |id| {
        Box::pin(super::browser_task::report_with(config, id, false))
    })
    .await;
}

/// [`learn`] with the module's `TaskReport` call passed in as `fetch`.
async fn learn_with<'a>(
    config: &'a Config,
    view: &TaskView,
    fetch: impl FnOnce(TaskId) -> ReportFetch<'a>,
) {
    match &view.status {
        TaskStatus::Done { .. } | TaskStatus::Checkpoint { .. } => {
            let Some(following) = followed_as(&view.id) else {
                return;
            };
            if !learning(config) {
                stop_following(&view.id);
                return;
            }
            match fetch(view.id.clone()).await {
                Ok(report) => {
                    let learned = now();
                    let mut kept = false;
                    update(config, &following.site, |memory| {
                        // Still followed, under the save lock: a Forget that
                        // came while the report was fetched holds.
                        kept = stop_following(&view.id).is_some();
                        if kept {
                            memory.learn(&following, &report, learned);
                        }
                        kept
                    })
                    .await;
                    tracing::debug!(
                        task = %view.id,
                        kept,
                        found = report.learned.len(),
                        rescues = report.rescues.len(),
                        "[browser-sites] learned from a finished task"
                    );
                }
                Err(error) => {
                    stop_following(&view.id);
                    tracing::warn!(task = %view.id, %error, "[browser-sites] report unavailable; nothing learned");
                }
            }
        }
        TaskStatus::Failed { .. } => {
            let Some(following) = stop_following(&view.id) else {
                return;
            };
            if following.reused_plan {
                update(config, &following.site, |memory| {
                    memory.forget_plan(&following);
                    true
                })
                .await;
                tracing::debug!(task = %view.id, "[browser-sites] forgot a reused plan that failed");
            }
        }
        TaskStatus::Cancelled | TaskStatus::NeedsPlan { .. } => {
            stop_following(&view.id);
        }
        _ => {}
    }
}

/// Forgets what was learned about `site` (a host or an address), or about
/// every site when `None`, also by the tasks still running there. Returns how
/// many sites were forgotten.
///
/// # Errors
///
/// Returns an I/O error when a site's file exists and cannot be removed.
pub async fn forget(config: &Config, site: Option<&str>) -> std::io::Result<usize> {
    let site = match site {
        Some(named) => match site_of(named).or_else(|| site_named(named)) {
            Some(site) => Some(site),
            None => return Ok(0),
        },
        None => None,
    };
    let _saving = saving().lock().await;
    // A task still running there would write it all back when it ends.
    followed().retain(|(_, following)| {
        following.workspace != config.workspace_dir
            || site.as_ref().is_some_and(|site| *site != following.site)
    });
    let Some(site) = site else {
        return forget_every_site(&sites_dir(config)).await;
    };
    let path = site_path(config, &site);
    // A copy set aside as unreadable holds the same site's data.
    let set_aside = remove(&path.with_extension("json.corrupt")).await?;
    remove(&path.with_extension("json.tmp")).await?;
    let kept = remove(&path).await?;
    Ok(usize::from(kept || set_aside))
}

/// Removes every file in `dir`, returning how many sites they held.
async fn forget_every_site(dir: &Path) -> std::io::Result<usize> {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    let mut forgotten = BTreeSet::new();
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(site) = name
                .strip_suffix(".json")
                .or_else(|| name.strip_suffix(".json.corrupt"))
            {
                forgotten.insert(site.to_owned());
            }
            remove(&entry.path()).await?;
        }
    }
    Ok(forgotten.len())
}

/// Removes the file at `path`, returning whether there was one.
async fn remove(path: &Path) -> std::io::Result<bool> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Removes the site files nothing changed for `KEEP_SECS` before `now`:
/// every entry in them has expired, as has a copy set aside or a write left
/// behind.
async fn sweep(config: &Config, now: u64) {
    let _saving = saving().lock().await;
    let Ok(mut entries) = tokio::fs::read_dir(sites_dir(config)).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(metadata) = entry.metadata().await else {
            continue;
        };
        let changed = metadata
            .modified()
            .ok()
            .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
            .map_or(now, |since| since.as_secs());
        if metadata.is_file() && changed.saturating_add(KEEP_SECS) <= now {
            match remove(&entry.path()).await {
                Ok(_) => {
                    tracing::debug!("[browser-sites] removed a site file unchanged for 30 days")
                }
                Err(error) => tracing::warn!(%error, "[browser-sites] old site file not removed"),
            }
        }
    }
}

impl SiteMemory {
    /// Drops what no finished run has used since `now - KEEP_SECS`.
    fn expire(&mut self, now: u64) {
        let fresh = |ok_at: u64| ok_at.saturating_add(KEEP_SECS) > now;
        self.plans.retain(|plan| fresh(plan.ok_at));
        self.hints.retain(|saved| fresh(saved.ok_at));
    }

    /// Keeps what `following`'s finished run, reported in `report`, found,
    /// and the plan it ran when it needed no rescue; forgets a reused plan
    /// that did.
    fn learn(&mut self, following: &Following, report: &TaskReport, now: u64) {
        for hint in &report.learned {
            if !keepable_hint(hint, &following.facts) {
                continue;
            }
            self.hints
                .retain(|saved| !(saved.hint.app == hint.app && saved.hint.key == hint.key));
            self.hints.push(SavedHint {
                hint: hint.clone(),
                ok_at: now,
            });
        }
        let excess = self.hints.len().saturating_sub(MAX_HINTS);
        self.hints.drain(..excess);
        if !report.rescues.is_empty() {
            if following.reused_plan {
                self.forget_plan(following);
            }
            return;
        }
        let Some(flow) = report
            .flow
            .as_ref()
            .filter(|flow| keepable_plan(flow, following))
        else {
            return;
        };
        let runs = self
            .plans
            .iter()
            .find(|plan| plan.serves(&following.goal, &following.facts_id))
            .map_or(0, |plan| plan.runs);
        self.forget_plan(following);
        self.plans.push(SavedPlan {
            goal: following.goal.clone(),
            facts_id: following.facts_id.clone(),
            flow: flow.clone(),
            ok_at: now,
            runs: runs.saturating_add(1),
        });
        self.plans.sort_by_key(|plan| plan.ok_at);
        let excess = self.plans.len().saturating_sub(MAX_PLANS);
        self.plans.drain(..excess);
    }

    /// Forgets the plan saved for `following`'s goal and facts.
    fn forget_plan(&mut self, following: &Following) {
        self.plans
            .retain(|plan| !plan.serves(&following.goal, &following.facts_id));
    }
}

impl SavedPlan {
    /// Whether it is the plan saved for `goal` with the facts `facts_id`.
    fn serves(&self, goal: &str, facts_id: &str) -> bool {
        self.goal == goal && self.facts_id == facts_id
    }
}

/// Whether `hint` may be kept: its name reads like a control's label, and
/// neither it, its key, nor its place holds one of `facts`.
fn keepable_hint(hint: &GroundingHint, facts: &[String]) -> bool {
    let name = hint.name.as_deref().unwrap_or_default();
    let mut texts = [name, hint.key.as_str()]
        .into_iter()
        .chain(hint.path.iter().map(String::as_str));
    name.chars().count() <= MAX_NAME_CHARS && !texts.any(|text| holds(text, facts))
}

/// Whether `flow` may be reused for its goal: it holds no fact value the
/// goal does not, which another task with the same goal could change.
fn keepable_plan(flow: &Flow, following: &Following) -> bool {
    let Ok(text) = serde_json::to_string(flow) else {
        return false;
    };
    let foreign: Vec<String> = following
        .facts
        .iter()
        .filter(|fact| !holds(&following.goal, std::slice::from_ref(*fact)))
        .cloned()
        .collect();
    !holds(&text, &foreign)
}

/// Whether `text`, as written or as its words, holds any of `facts`.
fn holds(text: &str, facts: &[String]) -> bool {
    let written = text.to_lowercase();
    let spelled = words(text);
    facts
        .iter()
        .any(|fact| written.contains(fact.as_str()) || spelled.contains(fact.as_str()))
}

fn sites_dir(config: &Config) -> PathBuf {
    super::computer_config::trace_dir(config).join("sites")
}

fn site_path(config: &Config, site: &str) -> PathBuf {
    sites_dir(config).join(format!("{site}.json"))
}

/// `site`'s memory as of `now`. A missing file is an empty memory; a file
/// that cannot be read as one is set aside as `<site>.json.corrupt` and
/// treated as empty.
async fn load(config: &Config, site: &str, now: u64) -> SiteMemory {
    let path = site_path(config, site);
    let read = match tokio::fs::metadata(&path).await {
        Ok(metadata) if metadata.len() <= MAX_FILE_BYTES => tokio::fs::read(&path).await,
        Ok(_) => Err(std::io::Error::other("larger than a site file can be")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return SiteMemory::default();
        }
        Err(error) => Err(error),
    };
    let parsed = read.and_then(|bytes| {
        serde_json::from_slice::<SiteMemory>(&bytes).map_err(std::io::Error::other)
    });
    match parsed {
        Ok(mut memory) => {
            memory.expire(now);
            memory
        }
        Err(error) => {
            tracing::warn!(%error, "[browser-sites] unreadable site file set aside");
            let _set_aside = tokio::fs::rename(&path, path.with_extension("json.corrupt")).await;
            SiteMemory::default()
        }
    }
}

/// Loads `site`'s memory and lets `change` change it, one update at a time,
/// writing it back when `change` says it did; a memory left empty removes
/// the file. A failure is logged, never raised.
async fn update(config: &Config, site: &str, change: impl FnOnce(&mut SiteMemory) -> bool) {
    let _saving = saving().lock().await;
    let mut memory = load(config, site, now()).await;
    if !change(&mut memory) {
        return;
    }
    let path = site_path(config, site);
    let written = if memory.plans.is_empty() && memory.hints.is_empty() {
        remove(&path).await.map(|_removed| ())
    } else {
        save(&path, &memory).await
    };
    if let Err(error) = written {
        tracing::warn!(%error, "[browser-sites] site memory not saved");
    }
}

/// Writes `memory` to `path` through a temporary file, in a directory and a
/// file only their owner can read: plans and elements tell where a person
/// browses.
async fn save(path: &Path, memory: &SiteMemory) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(memory).map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
        #[cfg(unix)]
        tokio::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).await?;
    }
    let staged = path.with_extension("json.tmp");
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&staged).await?;
    #[cfg(unix)]
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .await?;
    file.write_all(&bytes).await?;
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&staged, path).await
}

/// Serializes changes to site files across tasks.
fn saving() -> &'static tokio::sync::Mutex<()> {
    static SAVING: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    SAVING.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// Learning switched in Browser settings since the process started, by
/// workspace.
fn switched() -> MutexGuard<'static, HashMap<PathBuf, bool>> {
    static SWITCHED: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
    SWITCHED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// The tasks being followed, oldest first.
fn followed() -> MutexGuard<'static, VecDeque<(TaskId, Following)>> {
    static FOLLOWING: OnceLock<Mutex<VecDeque<(TaskId, Following)>>> = OnceLock::new();
    FOLLOWING
        .get_or_init(|| Mutex::new(VecDeque::new()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// What task `id` is followed for, if it is.
fn followed_as(id: &TaskId) -> Option<Following> {
    followed()
        .iter()
        .find(|(known, _)| known == id)
        .map(|(_, following)| following.clone())
}

/// Stops following task `id`, returning what it was followed for.
fn stop_following(id: &TaskId) -> Option<Following> {
    let mut followed = followed();
    let index = followed.iter().position(|(known, _)| known == id)?;
    followed.remove(index).map(|(_, following)| following)
}

#[cfg(test)]
#[path = "browser_sites_tests.rs"]
mod tests;
