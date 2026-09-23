use super::describe_signature_divergence;

/// #6419: the parity warning has to name *what* diverged.
///
/// These assert the rendered description, not that the two vectors differ —
/// "they differ" is what the old warning already told us, every turn, for long
/// enough that a true signal got filed as noise.
#[test]
fn divergence_description_names_the_spans_present_live_and_absent_in_the_journal() {
    let live = vec![
        "Turn|agent.turn:orchestrator|Ok|attrs:[a]".to_string(),
        "SubAgent|subagent:researcher|Ok|attrs:[b]".to_string(),
        "Tool|tool:file_read|Ok|attrs:[c]".to_string(),
    ];
    let projected = vec![
        "Turn|agent.turn:orchestrator|Ok|attrs:[a]".to_string(),
        "Tool|tool:file_read|Ok|attrs:[c]".to_string(),
    ];

    let described = describe_signature_divergence(&live, &projected);
    assert!(
        described.contains("live_only"),
        "a span present live and absent in the journal must be named: {described}"
    );
    assert!(
        described.contains("subagent:researcher"),
        "the missing span's identity is the whole point: {described}"
    );
    // The spans both sides agree on must NOT be listed — a description that
    // repeats the whole signature set is the blob this replaces.
    assert!(
        !described.contains("file_read"),
        "agreed spans must not be reported as divergent: {described}"
    );
    assert!(
        !described.contains("journal_only"),
        "nothing was projected that the live path lacked: {described}"
    );
}

#[test]
fn divergence_description_counts_repeats_as_a_multiset() {
    // Three iterations live, one projected. Matching as sets would call these
    // "identical" and report nothing; the gap is two occurrences.
    let sig = "Iteration|agent.iteration|Ok|attrs:[i]".to_string();
    let live = vec![sig.clone(), sig.clone(), sig.clone()];
    let projected = vec![sig.clone()];

    let described = describe_signature_divergence(&live, &projected);
    assert!(
        described.contains("2x"),
        "two missing occurrences, not a set difference of zero: {described}"
    );
}

#[test]
fn divergence_description_reports_journal_only_spans_too() {
    // The projection producing something the live path did not is a different
    // bug from the projection missing something, and must read differently.
    let live: Vec<String> = Vec::new();
    let projected = vec!["Tool|tool:ghost|Ok|attrs:[]".to_string()];

    let described = describe_signature_divergence(&live, &projected);
    assert!(described.contains("journal_only"), "{described}");
    assert!(!described.contains("live_only"), "{described}");
}

#[test]
fn divergence_description_is_empty_when_the_two_agree() {
    let sig = vec!["Turn|agent.turn:orchestrator|Ok|attrs:[a]".to_string()];
    assert_eq!(describe_signature_divergence(&sig, &sig), "");
    assert_eq!(describe_signature_divergence(&[], &[]), "");
}
