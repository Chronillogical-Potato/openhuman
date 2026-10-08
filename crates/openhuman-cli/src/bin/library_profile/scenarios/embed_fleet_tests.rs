use super::*;

#[test]
fn agent_ids_are_distinct_and_valid() {
    let ids: std::collections::HashSet<String> = (0..500).map(agent_id).collect();
    assert_eq!(ids.len(), 500);
    assert!(ids.iter().all(|id| id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')));
}

#[test]
fn per_agent_divides_the_delta_and_skips_an_empty_fleet() {
    assert_eq!(per_agent(1_000, 3_000, 4), Some(500.0));
    assert_eq!(per_agent(3_000, 1_000, 4), Some(-500.0));
    assert_eq!(per_agent(1_000, 3_000, 0), None);
}
