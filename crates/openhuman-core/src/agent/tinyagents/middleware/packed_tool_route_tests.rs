use super::*;

fn middleware(registered: &[&str]) -> PackedToolRouteMiddleware {
    PackedToolRouteMiddleware::new(registered.iter().map(|n| n.to_string()))
}

#[test]
fn a_bare_packed_call_becomes_the_use_skill_call_that_reaches_it() {
    let mw = middleware(&["use_skill", "shell"]);
    let mut call = TaToolCall::new("c1", "skill_registry_search", json!({ "query": "x" }));

    assert!(mw.route(&mut call));
    assert_eq!(call.name, "use_skill");
    assert_eq!(
        call.arguments,
        json!({ "skill": "skills", "tool": "skill_registry_search", "args": { "query": "x" } })
    );
}

#[test]
fn calls_the_crate_should_answer_are_left_alone() {
    let unparseable = TaToolCall {
        invalid: Some("bad json".into()),
        ..TaToolCall::new("c1", "skill_registry_search", json!("{query"))
    };
    let cases = [
        (
            middleware(&["use_skill"]),
            TaToolCall::new("c1", "made_up_tool", json!({})),
            "a name no pack owns",
        ),
        (
            middleware(&["use_skill", "file_read"]),
            TaToolCall::new("c1", "file_read", json!({})),
            "a registered name",
        ),
        (
            middleware(&["shell"]),
            TaToolCall::new("c1", "skill_registry_search", json!({})),
            "a turn without use_skill",
        ),
        (
            middleware(&["use_skill"]),
            TaToolCall::new("c1", "skill_registry_search", json!(["x"])),
            "non-object arguments",
        ),
        (
            middleware(&["use_skill"]),
            unparseable,
            "unparseable arguments",
        ),
    ];
    for (mw, mut call, why) in cases {
        let before = call.clone();
        assert!(!mw.route(&mut call), "{why}: must not route");
        assert_eq!(call, before, "{why}: call must be untouched");
    }
}
