//! Site files: entries that expire and the limits a site's memory keeps,
//! an unreadable file set aside, forgetting one site or every site, the
//! 30-day sweep, and the size a file may grow to.

use super::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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
    let mut request = crate::modules::browser_task::start_request(&config, &nowhere);
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
async fn a_site_that_is_no_plain_host_names_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let config = workspace_config(&dir);
    for site in ["../outside", "a/b", "..", "", ".hidden"] {
        let mut odd = task("Start at https://shop.test. Order milk");
        odd.site = Some(site.to_owned());
        let mut request = crate::modules::browser_task::start_request(&config, &odd);
        assert!(apply(&config, &odd, &mut request).await.is_none(), "{site}");
    }
    // Spelled another way, a host is still its site.
    let mut spelled = task("Start at https://shop.test. Order milk");
    spelled.site = Some("WWW.Shop.Test".to_owned());
    let mut request = crate::modules::browser_task::start_request(&config, &spelled);
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
