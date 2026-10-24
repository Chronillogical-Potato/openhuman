use super::{AutonomyLevel, CommandClass, SecurityPolicy};
use serde::Deserialize;

const COMMANDS: &str = include_str!("fixtures/security_command_corpus.json");

#[derive(Debug, Deserialize)]
struct Case {
    policy: String,
    op: String,
    command: String,
    expect: serde_json::Value,
}

fn policy(name: &str) -> SecurityPolicy {
    let mut policy = SecurityPolicy::default();
    match name {
        "default" => {}
        "readonly" => policy.autonomy = AutonomyLevel::ReadOnly,
        "full" => policy.autonomy = AutonomyLevel::Full,
        other => panic!("unknown command-corpus policy {other}"),
    }
    policy
}

fn class(name: &str) -> CommandClass {
    match name {
        "Read" => CommandClass::Read,
        "Write" => CommandClass::Write,
        "Network" => CommandClass::Network,
        "Install" => CommandClass::Install,
        "Destructive" => CommandClass::Destructive,
        other => panic!("unknown command-corpus class {other}"),
    }
}

#[test]
fn security_command_corpus_pins_host_policy_behavior() {
    let cases: Vec<Case> = serde_json::from_str(COMMANDS).expect("valid command corpus");
    assert_eq!(cases.len(), 118, "the characterized corpus is complete");

    for case in cases {
        let policy = policy(&case.policy);
        match case.op.as_str() {
            "allowed" => assert_eq!(
                policy.is_command_allowed(&case.command),
                case.expect.as_bool().expect("boolean expectation"),
                "policy={} command={:?}",
                case.policy,
                case.command,
            ),
            "class" => assert_eq!(
                policy.classify_command(&case.command),
                class(case.expect.as_str().expect("class expectation")),
                "policy={} command={:?}",
                case.policy,
                case.command,
            ),
            other => panic!("unknown command-corpus operation {other}"),
        }
    }
}
