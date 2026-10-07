use super::*;

#[test]
fn a_job_is_running_exactly_while_a_guard_is_held() {
    let id = "in-flight-single";
    assert!(!is_running(id));
    let guard = enter(id);
    assert!(is_running(id));
    drop(guard);
    assert!(!is_running(id));
}

#[test]
fn overlapping_runs_keep_the_job_running_until_the_last_one_ends() {
    // A "Run now" may overlap a scheduled run of a default (not single-flight)
    // job; the first to finish must not mark the other as done.
    let id = "in-flight-overlap";
    let scheduled = enter(id);
    let manual = enter(id);
    drop(scheduled);
    assert!(is_running(id), "the manual run is still going");
    drop(manual);
    assert!(!is_running(id));
}

#[test]
fn jobs_are_tracked_independently() {
    let a = enter("in-flight-a");
    assert!(is_running("in-flight-a"));
    assert!(!is_running("in-flight-b"));
    drop(a);
}
