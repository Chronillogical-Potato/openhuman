//! The store itself is tested upstream (`tinyflows-sqlite`); these pin the
//! host mapping from `Config` to its options.

use super::*;
use chrono::Duration as ChronoDuration;
use tempfile::TempDir;

fn test_config(tmp: &TempDir) -> Config {
    let config = Config {
        workspace_dir: tmp.path().join("workspace"),
        action_dir: tmp.path().join("workspace"),
        config_path: tmp.path().join("config.toml"),
        ..Config::default()
    };
    std::fs::create_dir_all(&config.workspace_dir).unwrap();
    config
}

#[test]
fn database_lives_at_workspace_cron_jobs_db() {
    let tmp = TempDir::new().unwrap();
    let config = test_config(&tmp);
    add_job(&config, "*/5 * * * *", "echo ok").unwrap();
    assert!(tmp.path().join("workspace/cron/jobs.db").is_file());
}

#[test]
fn due_jobs_batch_size_follows_scheduler_max_tasks() {
    let tmp = TempDir::new().unwrap();
    let mut config = test_config(&tmp);
    config.scheduler.max_tasks = 2;
    for i in 0..3 {
        add_job(&config, "* * * * *", &format!("echo due-{i}")).unwrap();
    }
    let far_future = Utc::now() + ChronoDuration::days(365);
    assert_eq!(due_jobs(&config, far_future).unwrap().len(), 2);
}

#[test]
fn run_history_cap_follows_cron_max_run_history() {
    let tmp = TempDir::new().unwrap();
    let mut config = test_config(&tmp);
    config.cron.max_run_history = 2;
    let job = add_job(&config, "*/15 * * * *", "echo run").unwrap();
    for i in 0..5 {
        let t = Utc::now() + ChronoDuration::seconds(i);
        record_run(&config, &job.id, t, t, "ok", Some("x"), 1).unwrap();
    }
    assert_eq!(list_runs(&config, &job.id, 10).unwrap().len(), 2);
}
