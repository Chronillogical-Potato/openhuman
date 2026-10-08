//! A finished browser task leaves its plan and the elements it found for the
//! next task on the same site, the plan only for the same goal and facts;
//! nothing that holds a fact, reads like page text, needed a rescue, or
//! failed when reused is kept, and none of it outlives its use, the learning
//! switch, or a request to forget.

use super::*;
use serde_json::{json, Value};
use std::time::Duration;

/// A config whose workspace is inside `dir`, learning on.
fn workspace_config(dir: &tempfile::TempDir) -> Config {
    let mut config = Config::default();
    config.workspace_dir = dir.path().join("workspace");
    config
}

/// A task on `shop.test` for `goal`, typing the name "Asha Raina".
fn task(goal: &str) -> BrowserTask {
    BrowserTask {
        goal: goal.into(),
        facts: BTreeMap::from([("name".into(), "Asha Raina".into())]),
        origins: vec!["https://.shop.test".into()],
        max_actions: 40,
        flow: None,
        site: Some("shop.test".into()),
    }
}

fn view(id: &str, status: Value) -> TaskView {
    serde_json::from_value(json!({
        "id": id, "status": status, "summary": "", "progress": 1.0, "next": []
    }))
    .unwrap()
}

fn done() -> Value {
    json!({"state": "done", "answer": "ordered", "records": {}})
}

fn failed() -> Value {
    json!({"state": "failed", "step": 2, "reason": "nothing changed", "hint": "", "recoverable": true})
}

fn flow(steps: Value) -> Value {
    json!({"app": "browser", "steps": steps})
}

fn hint(key: &str, name: &str) -> Value {
    json!({"app": "browser", "key": key, "role": "button", "name": name, "path": ["main", "product"]})
}

/// A report of `view` that ran `flow`, found `learned`, and took `rescues`.
fn report(view: &TaskView, flow: Value, learned: Value, rescues: Value) -> TaskReport {
    serde_json::from_value(json!({
        "view": view, "flow": flow, "steps": [], "records": {}, "artifacts": [],
        "learned": learned, "trace": [], "rescues": rescues
    }))
    .unwrap()
}

/// A rescue that put a step in place of the failed one.
fn rescue() -> Value {
    json!([{"step": 1, "failure": "covered", "reason": "a size comes first",
            "steps": ["choose a size"], "covers": 0, "outcome": "recovered"}])
}

/// Starts `task` as the host would: the request, what the site left, and the
/// task followed as `id`.
async fn start(config: &Config, task: &BrowserTask, id: &str) -> StartTaskRequest {
    let mut request = super::super::browser_task::start_request(config, task);
    follow(
        config,
        &TaskId::new(id),
        apply(config, task, &mut request).await,
    );
    request
}

/// A change that leaves one element in a site's memory.
fn keep_one(memory: &mut SiteMemory) -> bool {
    memory.hints.push(SavedHint {
        hint: serde_json::from_value(hint("search", "Search")).unwrap(),
        ok_at: now(),
    });
    true
}

/// A report of `id` finishing on a one-step plan with one element found.
fn finished(id: &str) -> TaskReport {
    report(
        &view(id, done()),
        flow(json!(["a"])),
        json!([hint("a", "A")]),
        json!([]),
    )
}

/// Ends task `id` in `status`, its report being `reported`.
async fn end(config: &Config, id: &str, status: Value, reported: Option<TaskReport>) {
    let ended = view(id, status);
    let token = token_of(config, &TaskId::new(id));
    learn_with(config, &ended, token, move |_| {
        Box::pin(async move { reported.ok_or_else(|| "no report".to_owned()) })
    })
    .await;
}

#[test]
fn a_site_is_its_host_without_www() {
    assert_eq!(
        site_of("https://www.Amazon.in/s?k=x").as_deref(),
        Some("amazon.in")
    );
    assert_eq!(
        site_of(" http://shop.test:8080/cart ").as_deref(),
        Some("shop.test")
    );
    assert_eq!(site_of("file:///tmp/page.html"), None);
    assert_eq!(site_of("https://[::1]/"), None);
    assert_eq!(site_of("not an address"), None);
    assert_eq!(site_named(".hidden"), None);
}

#[test]
fn goals_are_matched_whatever_their_case_and_spacing() {
    assert_eq!(
        goal_key("  Order   MILK\non shop.test "),
        "order milk on shop.test"
    );
}

#[tokio::test]
async fn a_finished_task_leaves_its_plan_and_elements_for_the_next_task_there() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    let first = start(&config, &order, "t-plan-1").await;
    assert!(first.flow.is_none(), "nothing saved yet");
    assert_eq!(first.memory.len(), 0);

    let ran = flow(json!([{"browse": "https://shop.test"}, "add milk to the cart"]));
    let found = json!([
        hint("add milk to the cart", "Add to cart"),
        hint("deliver", "Deliver to Asha Raina"),
        hint("read", &"long page text ".repeat(10)),
    ]);
    let finished = view("t-plan-1", done());
    end(
        &config,
        "t-plan-1",
        done(),
        Some(report(&finished, ran.clone(), found, json!([]))),
    )
    .await;

    let second = start(&config, &order, "t-plan-2").await;
    assert_eq!(serde_json::to_value(second.flow.unwrap()).unwrap(), ran);
    let names: Vec<_> = second.memory.iter().map(|hint| hint.name.clone()).collect();
    assert_eq!(
        names,
        [Some("Add to cart".to_owned())],
        "no fact, no page text"
    );

    // Another goal on the same site gets the elements, not the plan.
    let other = start(
        &config,
        &task("Start at https://shop.test. Order eggs"),
        "t-plan-3",
    )
    .await;
    assert!(other.flow.is_none());
    assert_eq!(other.memory.len(), 1);
}

#[tokio::test]
async fn a_plan_that_needed_a_rescue_or_holds_a_fact_is_not_kept() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    let ran = flow(json!(["add milk to the cart"]));

    start(&config, &order, "t-rescued").await;
    let finished = view("t-rescued", done());
    let rescued = report(
        &finished,
        ran.clone(),
        json!([hint("add", "Add")]),
        rescue(),
    );
    end(&config, "t-rescued", done(), Some(rescued)).await;
    let next = start(&config, &order, "t-after-rescue").await;
    assert!(next.flow.is_none(), "a rescued run's plan is not kept");
    assert_eq!(next.memory.len(), 1, "its elements are");

    // A plan typing a fact the goal does not name could type the wrong one.
    let typed = flow(json!([{"enter": {"name": "Asha Raina"}}]));
    let finished = view("t-after-rescue", done());
    end(
        &config,
        "t-after-rescue",
        done(),
        Some(report(&finished, typed.clone(), json!([]), json!([]))),
    )
    .await;
    assert!(start(&config, &order, "t-fact").await.flow.is_none());

    // Named in the goal, the same value is part of what the plan is for.
    let named = task("Start at https://shop.test. Book a table for Asha Raina");
    start(&config, &named, "t-named").await;
    let finished = view("t-named", done());
    end(
        &config,
        "t-named",
        done(),
        Some(report(&finished, typed, json!([]), json!([]))),
    )
    .await;
    assert!(start(&config, &named, "t-named-again").await.flow.is_some());
}

#[tokio::test]
async fn a_plan_is_reused_only_with_the_facts_it_ran_with() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order a shirt");
    start(&config, &order, "t-facts-1").await;
    // Chosen by the size fact, not typed from it: only its facts may rerun it.
    let ran = flow(json!(["choose Medium", "add the shirt to the cart"]));
    let finished = view("t-facts-1", done());
    end(
        &config,
        "t-facts-1",
        done(),
        Some(report(
            &finished,
            ran,
            json!([hint("add", "Add to bag")]),
            json!([]),
        )),
    )
    .await;

    let mut larger = order.clone();
    larger.facts.insert("size".into(), "L".into());
    let request = start(&config, &larger, "t-facts-2").await;
    assert!(request.flow.is_none(), "other facts plan afresh");
    assert_eq!(request.memory.len(), 1, "and still get the site's elements");
    end(&config, "t-facts-2", failed(), None).await;
    assert!(start(&config, &order, "t-facts-3").await.flow.is_some());
    let saved = std::fs::read_to_string(site_path(&config, "shop.test")).unwrap();
    assert!(!saved.contains("Asha Raina"), "facts are kept as a digest");
}

#[tokio::test]
async fn a_reused_plan_that_fails_or_needs_a_rescue_is_forgotten() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    let ran = flow(json!(["add milk to the cart"]));
    let save = |id: &'static str| {
        let config = config.clone();
        let ran = ran.clone();
        let order = order.clone();
        async move {
            start(&config, &order, id).await;
            let finished = view(id, done());
            end(
                &config,
                id,
                done(),
                Some(report(&finished, ran, json!([]), json!([]))),
            )
            .await;
        }
    };

    save("t-save-1").await;
    assert!(start(&config, &order, "t-reuse-fails").await.flow.is_some());
    end(&config, "t-reuse-fails", failed(), None).await;
    assert!(
        start(&config, &order, "t-replan").await.flow.is_none(),
        "forgotten"
    );

    end(&config, "t-replan", failed(), None).await;
    save("t-save-2").await;
    start(&config, &order, "t-reuse-rescued").await;
    let finished = view("t-reuse-rescued", done());
    end(
        &config,
        "t-reuse-rescued",
        done(),
        Some(report(&finished, ran.clone(), json!([]), rescue())),
    )
    .await;
    assert!(start(&config, &order, "t-replan-2").await.flow.is_none());
}

#[tokio::test]
async fn a_flow_the_caller_brings_is_never_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    start(&config, &order, "t-own-1").await;
    let finished = view("t-own-1", done());
    end(
        &config,
        "t-own-1",
        done(),
        Some(report(&finished, flow(json!(["a"])), json!([]), json!([]))),
    )
    .await;

    let mut own = order.clone();
    own.flow = Some(serde_json::from_value(flow(json!(["b"]))).unwrap());
    let request = start(&config, &own, "t-own-2").await;
    assert_eq!(
        serde_json::to_value(request.flow.unwrap()).unwrap(),
        flow(json!(["b"]))
    );
    // Failing on its own flow does not forget the saved one.
    end(&config, "t-own-2", failed(), None).await;
    assert!(start(&config, &order, "t-own-3").await.flow.is_some());
}

#[tokio::test]
async fn with_learning_off_nothing_is_handed_over_or_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    start(&config, &order, "t-switch").await;
    config.browser.learn_from_tasks = false;
    let finished = view("t-switch", done());
    end(
        &config,
        "t-switch",
        done(),
        Some(report(
            &finished,
            flow(json!(["a"])),
            json!([hint("a", "A")]),
            json!([]),
        )),
    )
    .await;
    let mut request = super::super::browser_task::start_request(&config, &order);
    assert!(apply(&config, &order, &mut request).await.is_none());
    config.browser.learn_from_tasks = true;
    let request = start(&config, &order, "t-switch-on").await;
    assert!(request.flow.is_none());
    assert_eq!(request.memory.len(), 0);
    // A task with no site learns nothing either.
    let mut nowhere = order.clone();
    nowhere.site = None;
    let mut request = super::super::browser_task::start_request(&config, &nowhere);
    assert!(apply(&config, &nowhere, &mut request).await.is_none());
}

#[tokio::test]
async fn a_paused_task_is_followed_until_it_ends() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    start(&config, &order, "t-paused").await;
    end(
        &config,
        "t-paused",
        json!({"state": "needs_human", "reason": "sign in"}),
        None,
    )
    .await;
    let finished = view("t-paused", done());
    end(
        &config,
        "t-paused",
        done(),
        Some(report(&finished, flow(json!(["a"])), json!([]), json!([]))),
    )
    .await;
    assert!(start(&config, &order, "t-paused-next").await.flow.is_some());

    // A cancelled task, or one whose report cannot be had, teaches nothing.
    end(
        &config,
        "t-paused-next",
        json!({"state": "cancelled"}),
        None,
    )
    .await;
    let other = task("Start at https://shop.test. Order eggs");
    start(&config, &other, "t-no-report").await;
    end(&config, "t-no-report", done(), None).await;
    assert!(start(&config, &other, "t-no-report-next")
        .await
        .flow
        .is_none());
    // Ending a task nobody follows does nothing.
    end(&config, "t-unknown", done(), None).await;
}

#[test]
fn old_entries_go_and_limits_hold() {
    let order = task("Order milk");
    let following = Following {
        token: 0,
        workspace: PathBuf::from("/nowhere"),
        site: "shop.test".into(),
        goal: goal_key(&order.goal),
        facts_id: facts_id(&order.facts),
        reused_plan: false,
        facts: fact_values(order.facts.values()),
    };
    let finished = view("t-limits", done());
    let found: Vec<Value> = (0..MAX_HINTS + 5)
        .map(|index| hint(&format!("step {index}"), &format!("Button {index}")))
        .collect();
    let mut memory = SiteMemory::default();
    memory.learn(
        &following,
        &report(&finished, flow(json!(["a"])), json!(found), json!([])),
        100,
    );
    assert_eq!(memory.hints.len(), MAX_HINTS);
    assert_eq!(memory.hints[0].hint.key, "step 5", "the oldest went first");
    for index in 0..MAX_PLANS + 2 {
        let goal = Following {
            goal: format!("goal {index}"),
            ..following.clone()
        };
        let at = 200 + u64::try_from(index).unwrap();
        memory.learn(
            &goal,
            &report(&finished, flow(json!(["a"])), json!([]), json!([])),
            at,
        );
    }
    assert_eq!(memory.plans.len(), MAX_PLANS);
    assert!(memory
        .plans
        .iter()
        .all(|plan| plan.goal != "goal 0" && plan.goal != "goal 1"));
    memory.expire(100 + KEEP_SECS);
    assert_eq!(memory.hints.len(), 0, "unused for 30 days");
    assert_eq!(memory.plans.len(), MAX_PLANS);
}

#[tokio::test]
async fn an_unreadable_site_file_is_set_aside_and_files_stay_private() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let path = site_path(&config, "shop.test");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"{not json").unwrap();
    assert_eq!(load(&config, "shop.test", now()).await.plans.len(), 0);
    assert!(path.with_extension("json.corrupt").exists());

    update(&config, "shop.test", keep_one).await;
    #[cfg(unix)]
    {
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
    }
}

#[tokio::test]
async fn forgetting_removes_one_site_or_every_site() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    assert_eq!(
        forget(&config, None).await.unwrap(),
        0,
        "nothing learned yet"
    );
    for site in ["shop.test", "books.test"] {
        update(&config, site, keep_one).await;
    }
    let staged = site_path(&config, "shop.test").with_extension("json.tmp");
    std::fs::write(&staged, b"{").unwrap();
    assert_eq!(
        forget(&config, Some("https://www.shop.test/cart"))
            .await
            .unwrap(),
        1
    );
    assert!(!staged.exists(), "a write left behind goes too");
    assert_eq!(
        forget(&config, Some("shop.test")).await.unwrap(),
        0,
        "already gone"
    );
    assert_eq!(forget(&config, Some("not a host!")).await.unwrap(), 0);
    // A copy set aside as unreadable is the same site's data, counted once.
    let set_aside = |site: &str| site_path(&config, site).with_extension("json.corrupt");
    std::fs::write(set_aside("books.test"), b"{").unwrap();
    std::fs::write(set_aside("maps.test"), b"{").unwrap();
    assert_eq!(forget(&config, Some("maps.test")).await.unwrap(), 1);
    assert!(!set_aside("maps.test").exists());
    std::fs::write(set_aside("maps.test"), b"{").unwrap();
    assert_eq!(forget(&config, None).await.unwrap(), 2);
    assert!(!site_path(&config, "books.test").exists());
    assert!(!set_aside("books.test").exists());
}

#[tokio::test]
async fn an_id_the_module_gives_another_task_is_not_learned_from() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    start(&config, &order, "t-again").await;
    // Set up again, the module numbers its tasks afresh, and a task with no
    // site takes the same id.
    let mut elsewhere = order.clone();
    elsewhere.site = None;
    start(&config, &elsewhere, "t-again").await;
    end(&config, "t-again", done(), Some(finished("t-again"))).await;
    let next = start(&config, &order, "t-again-next").await;
    assert!(next.flow.is_none());
    assert_eq!(next.memory.len(), 0);
}

#[tokio::test]
async fn forgetting_holds_for_tasks_still_running() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");

    // Paused for an approval when the person forgets every site.
    start(&config, &order, "t-forgot-paused").await;
    forget(&config, None).await.unwrap();
    end(
        &config,
        "t-forgot-paused",
        done(),
        Some(finished("t-forgot-paused")),
    )
    .await;
    assert!(!site_path(&config, "shop.test").exists());

    // Forgotten while its report was being fetched.
    start(&config, &order, "t-forgot-fetching").await;
    let config_ref = &config;
    let token = token_of(&config, &TaskId::new("t-forgot-fetching"));
    learn_with(
        &config,
        &view("t-forgot-fetching", done()),
        token,
        move |_| {
            Box::pin(async move {
                forget(config_ref, Some("shop.test")).await.unwrap();
                Ok(finished("t-forgot-fetching"))
            })
        },
    )
    .await;
    assert!(!site_path(&config, "shop.test").exists());

    // Forgetting another site leaves a task here followed.
    start(&config, &order, "t-forgot-other").await;
    forget(&config, Some("books.test")).await.unwrap();
    end(
        &config,
        "t-forgot-other",
        done(),
        Some(finished("t-forgot-other")),
    )
    .await;
    assert!(start(&config, &order, "t-forgot-next").await.flow.is_some());
}

#[tokio::test]
async fn values_are_found_however_the_module_spells_them() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let mut signup = task("Start at https://shop.test. Sign up");
    signup
        .facts
        .insert("email".into(), "asha.r@example.com".into());
    signup.facts.insert("born".into(), "1990-05-12".into());
    start(&config, &signup, "t-spelled").await;
    // A value given when the paused task resumes is not kept either.
    note_inputs(
        &config,
        &TaskId::new("t-spelled"),
        &BTreeMap::from([("phone".into(), "98-7654-3210".into())]),
    );
    let found = json!([
        hint("type asha r example com into the email box", "Email"),
        {"app": "browser", "key": "pick the day", "role": "button", "name": "12", "path": ["1990 05 12"]},
        hint("enter 98 7654 3210", "Phone"),
        hint("press sign up", "Sign up"),
    ]);
    let ended = view("t-spelled", done());
    end(
        &config,
        "t-spelled",
        done(),
        Some(report(&ended, flow(json!(["a"])), found, json!([]))),
    )
    .await;
    let next = start(&config, &signup, "t-spelled-next").await;
    let keys: Vec<_> = next.memory.iter().map(|hint| hint.key.as_str()).collect();
    assert_eq!(keys, ["press sign up"]);
}

#[tokio::test]
async fn a_saved_plan_the_module_refuses_is_forgotten_and_the_task_planned_afresh() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    start(&config, &order, "t-refused-1").await;
    end(
        &config,
        "t-refused-1",
        done(),
        Some(finished("t-refused-1")),
    )
    .await;
    let refusal =
        || "browser task StartTask failed [INVALID_FLOW]: step 1 is not a step".to_owned();

    // A transport error is not a refusal: the plan stays.
    let error = super::super::browser_task::begin(&config, &order, |_| {
        Box::pin(async {
            Err::<TaskView, _>("browser task StartTask failed: bus closed".to_owned())
        })
    })
    .await
    .unwrap_err();
    assert!(error.contains("bus closed"));

    let sent = Mutex::new(Vec::new());
    let started = super::super::browser_task::begin(&config, &order, |request| {
        let carried = request.flow.is_some();
        sent.lock().unwrap().push(carried);
        Box::pin(async move {
            if carried {
                Err(refusal())
            } else {
                Ok(view("t-refused-2", json!({"state": "running"})))
            }
        })
    })
    .await
    .unwrap();
    assert_eq!(started.id, TaskId::new("t-refused-2"));
    assert_eq!(*sent.lock().unwrap(), [true, false], "sent again unplanned");
    assert!(
        start(&config, &order, "t-refused-3").await.flow.is_none(),
        "forgotten"
    );

    // A flow the caller brought, refused, is the caller's to fix.
    let mut own = order.clone();
    own.flow = Some(serde_json::from_value(flow(json!(["b"]))).unwrap());
    let calls = Mutex::new(0);
    let error = super::super::browser_task::begin(&config, &own, |_| {
        *calls.lock().unwrap() += 1;
        Box::pin(async move { Err::<TaskView, _>(refusal()) })
    })
    .await
    .unwrap_err();
    assert!(error.contains("INVALID_FLOW"));
    assert_eq!(*calls.lock().unwrap(), 1);
}

#[tokio::test]
async fn files_unchanged_for_30_days_go_and_an_emptied_site_leaves_none() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    for site in ["old.test", "new.test"] {
        update(&config, site, keep_one).await;
    }
    let old = site_path(&config, "old.test");
    let stale = [
        old.clone(),
        old.with_extension("json.corrupt"),
        old.with_extension("json.tmp"),
    ];
    let month_ago = SystemTime::now() - Duration::from_secs(KEEP_SECS + 60);
    for path in &stale {
        std::fs::write(path, b"{}").unwrap();
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(month_ago)
            .unwrap();
    }
    // Any task's start sweeps, one on no site included.
    let mut nowhere = task("Read the news");
    nowhere.site = None;
    let mut request = super::super::browser_task::start_request(&config, &nowhere);
    assert!(apply(&config, &nowhere, &mut request).await.is_none());
    assert!(stale.iter().all(|path| !path.exists()));
    assert!(site_path(&config, "new.test").exists());

    // A reused plan that fails, the site's only entry, leaves no file.
    let order = task("Start at https://shop.test. Order milk");
    start(&config, &order, "t-only").await;
    let ended = view("t-only", done());
    end(
        &config,
        "t-only",
        done(),
        Some(report(&ended, flow(json!(["a"])), json!([]), json!([]))),
    )
    .await;
    assert!(start(&config, &order, "t-only-again").await.flow.is_some());
    end(&config, "t-only-again", failed(), None).await;
    assert!(!site_path(&config, "shop.test").exists());
}

#[tokio::test]
async fn a_switch_in_settings_reaches_tools_built_before_it() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    start(&config, &order, "t-switched").await;
    note_switch(&config.workspace_dir, false);
    end(&config, "t-switched", done(), Some(finished("t-switched"))).await;
    assert!(!site_path(&config, "shop.test").exists(), "kept nothing");
    let mut request = super::super::browser_task::start_request(&config, &order);
    assert!(apply(&config, &order, &mut request).await.is_none());

    note_switch(&config.workspace_dir, true);
    assert!(apply(&config, &order, &mut request).await.is_some());
}

#[tokio::test]
async fn a_site_that_is_no_plain_host_names_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    for site in ["../outside", "a/b", "..", "", ".hidden"] {
        let mut odd = task("Start at https://shop.test. Order milk");
        odd.site = Some(site.to_owned());
        let mut request = super::super::browser_task::start_request(&config, &odd);
        assert!(apply(&config, &odd, &mut request).await.is_none(), "{site}");
    }
    // Spelled another way, a host is still its site.
    let mut spelled = task("Start at https://shop.test. Order milk");
    spelled.site = Some("WWW.Shop.Test".to_owned());
    let mut request = super::super::browser_task::start_request(&config, &spelled);
    let following = apply(&config, &spelled, &mut request).await.unwrap();
    assert_eq!(following.site, "shop.test");
}

#[tokio::test]
async fn a_site_file_stays_within_its_size_limit() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    start(&config, &order, "t-small").await;
    end(&config, "t-small", done(), Some(finished("t-small"))).await;
    let eggs = task("Start at https://shop.test. Order eggs");
    start(&config, &eggs, "t-big").await;
    let huge = flow(json!(
        ["x".repeat(usize::try_from(MAX_FILE_BYTES).unwrap())]
    ));
    let ended = view("t-big", done());
    end(
        &config,
        "t-big",
        done(),
        Some(report(&ended, huge, json!([hint("b", "B")]), json!([]))),
    )
    .await;

    let path = site_path(&config, "shop.test");
    assert!(std::fs::metadata(&path).unwrap().len() <= MAX_FILE_BYTES);
    let memory = load(&config, "shop.test", now()).await;
    assert!(
        !path.with_extension("json.corrupt").exists(),
        "read back, not set aside"
    );
    assert_eq!(memory.hints.len(), 2, "the elements are kept");
    assert_eq!(
        memory.plans.len(),
        0,
        "the oldest plans went first, then the one too large on its own"
    );
}

#[tokio::test]
async fn a_refused_plan_that_cannot_be_forgotten_still_counts_as_reused() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    start(&config, &order, "t-locked").await;
    end(&config, "t-locked", done(), Some(finished("t-locked"))).await;
    let mut request = super::super::browser_task::start_request(&config, &order);
    let mut reusing = apply(&config, &order, &mut request).await.unwrap();
    assert!(reusing.reuses_plan());

    // A directory where the site file is written through: the write fails.
    let staged = site_path(&config, "shop.test").with_extension("json.tmp");
    std::fs::create_dir(&staged).unwrap();
    refused(&config, &mut reusing).await;
    assert!(
        reusing.reuses_plan(),
        "still reused, so a failure on it forgets it"
    );
    std::fs::remove_dir(&staged).unwrap();
    refused(&config, &mut reusing).await;
    assert!(!reusing.reuses_plan());
    let mut request = super::super::browser_task::start_request(&config, &order);
    assert!(!apply(&config, &order, &mut request)
        .await
        .unwrap()
        .reuses_plan());
}

#[tokio::test]
async fn learning_switched_off_while_a_report_is_fetched_keeps_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    let order = task("Start at https://shop.test. Order milk");
    start(&config, &order, "t-switch-mid").await;
    let id = TaskId::new("t-switch-mid");
    let workspace = config.workspace_dir.clone();
    learn_with(
        &config,
        &view("t-switch-mid", done()),
        token_of(&config, &id),
        move |_| {
            Box::pin(async move {
                note_switch(&workspace, false);
                Ok(finished("t-switch-mid"))
            })
        },
    )
    .await;
    assert!(!site_path(&config, "shop.test").exists());
    assert!(token_of(&config, &id).is_none(), "no longer followed");
}

#[tokio::test]
async fn a_task_of_another_workspace_or_an_earlier_setup_is_not_taken_for_this_one() {
    let (one, two) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (first, second) = (workspace_config(&one), workspace_config(&two));
    let order = task("Start at https://shop.test. Order milk");
    let shared = TaskId::new("t-shared");
    // The same id, in two workspaces.
    start(&first, &order, "t-shared").await;
    start(&second, &order, "t-shared").await;
    end(&first, "t-shared", done(), Some(finished("t-shared"))).await;
    assert!(site_path(&first, "shop.test").exists());
    assert!(!site_path(&second, "shop.test").exists());
    assert!(token_of(&second, &shared).is_some(), "still followed there");

    // A view of an earlier task by that id, once the module, set up again,
    // gave it to a newer task.
    let earlier = token_of(&second, &shared);
    start(
        &second,
        &task("Start at https://shop.test. Order eggs"),
        "t-shared",
    )
    .await;
    learn_with(&second, &view("t-shared", done()), earlier, |_| {
        Box::pin(async { Ok::<_, String>(finished("t-shared")) })
    })
    .await;
    assert!(
        !site_path(&second, "shop.test").exists(),
        "the earlier view learns nothing"
    );
    end(&second, "t-shared", done(), Some(finished("t-shared"))).await;
    assert!(site_path(&second, "shop.test").exists());
}
