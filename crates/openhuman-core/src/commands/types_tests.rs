use super::*;

#[test]
fn command_kind_serializes_snake_case() {
    assert_eq!(serde_json::to_string(&CommandKind::Builtin).unwrap(), "\"builtin\"");
    assert_eq!(serde_json::to_string(&CommandKind::Skill).unwrap(), "\"skill\"");
    assert_eq!(serde_json::to_string(&CommandKind::Workflow).unwrap(), "\"workflow\"");
}

#[test]
fn insert_is_omitted_when_none() {
    let entry = CommandEntry {
        id: "some-skill".into(),
        label: "Some Skill".into(),
        description: String::new(),
        kind: CommandKind::Skill,
        insert: None,
    };
    let json = serde_json::to_value(&entry).unwrap();
    assert!(json.get("insert").is_none(), "{json}");
}

#[test]
fn insert_round_trips_when_present() {
    let entry = CommandEntry {
        id: "new".into(),
        label: "/new".into(),
        description: "Start a new conversation".into(),
        kind: CommandKind::Builtin,
        insert: Some("/new".into()),
    };
    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["insert"], "/new");
    assert_eq!(json["kind"], "builtin");
}
