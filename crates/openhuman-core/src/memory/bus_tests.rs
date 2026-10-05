use super::*;

use tinymemory_api::{ConsolidateRequest, Namespace, Reach};
use tinymemory_tools::BackgroundJob;

use crate::memory::lifecycle::jobs;
use crate::memory::test_fixtures::{bind_reference, config_in};

#[test]
fn the_subscriber_declares_its_name_and_domain() {
    let subscriber = SystemJobsSubscriber;
    assert_eq!(subscriber.name(), "memory::system_jobs");
    assert_eq!(subscriber.domains(), Some(&["cron"][..]));
}

#[tokio::test]
async fn the_background_job_runs_the_due_queue() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(&tmp);
    config.memory.recall.build_delay_secs = 0;
    bind_reference(&config);
    jobs::enqueue(
        &config,
        &Namespace::ROOT,
        vec![BackgroundJob::BuildBeliefs {
            request: ConsolidateRequest::new(Reach::exact(Namespace::agent("a"))),
        }],
    )
    .await;

    run_system_job(&config, BACKGROUND_JOB).await;
    let queue = jobs::snapshot(&config).await;
    assert!(queue.pending.is_empty());
    assert_eq!(queue.history.len(), 1);

    run_system_job(&config, "someone_elses_job").await;
    run_system_job(&config, SOURCES_SYNC_JOB).await;
}
