use super::*;
use serde_json::json;

fn defs() -> Vec<WorkflowInput> {
    vec![
        WorkflowInput {
            name: "repo".into(),
            description: "owner/name".into(),
            required: true,
            kind: None,
        },
        WorkflowInput {
            name: "issue".into(),
            description: "issue #".into(),
            required: true,
            kind: Some("integer".into()),
        },
        WorkflowInput {
            name: "pr_base".into(),
            description: "base branch".into(),
            required: false,
            kind: None,
        },
    ]
}

#[test]
fn missing_required_is_detected() {
    assert_eq!(
        missing_required_inputs(&defs(), &json!({"repo": "acme/web"})),
        vec!["issue".to_string()]
    );
    assert!(missing_required_inputs(&defs(), &json!({"repo": "acme/web", "issue": 42})).is_empty());
    // null counts as missing
    assert_eq!(
        missing_required_inputs(&defs(), &json!({"repo": "acme/web", "issue": null})),
        vec!["issue".to_string()]
    );
}

#[test]
fn renders_inputs_block_with_values_and_gaps() {
    let b = render_inputs_block(&defs(), &json!({"repo": "acme/web", "issue": 42}));
    assert!(b.starts_with("## Inputs"));
    assert!(b.contains("**repo**: acme/web"));
    assert!(b.contains("**issue**: 42"));
    assert!(b.contains("**pr_base**: (not provided)"));
    assert!(render_inputs_block(&[], &json!({})).is_empty());
}

#[test]
fn skill_input_parses_type_alias() {
    let i: WorkflowInput = serde_json::from_value(json!({
        "name": "issue", "description": "issue #", "required": true, "type": "integer"
    }))
    .unwrap();
    assert_eq!(i.kind.as_deref(), Some("integer"));
    assert!(i.required);
}

/// Seed a runnable WORKFLOW.md bundle under `root/slug/` with a distinct
/// body marker so the resolved definition can be traced back to its source.
fn seed_runnable(root: &std::path::Path, slug: &str, body_marker: &str) {
    seed_runnable_with_name(root, slug, slug, body_marker);
}

fn seed_runnable_with_name(root: &std::path::Path, slug: &str, name: &str, body_marker: &str) {
    let dir = root.join(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("WORKFLOW.md"),
        format!("---\nname: {name}\ndescription: {name} desc\n---\n\n{body_marker}\n"),
    )
    .unwrap();
}

#[test]
fn load_skills_reads_runtime_skill_prompt_and_inputs() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sd = tmp.path().join("skills").join("issue-crusher");
    std::fs::create_dir_all(&sd).unwrap();
    std::fs::write(
        sd.join("skill.toml"),
        "id = \"issue-crusher\"\nwhen_to_use = \"fix a github issue\"\n\
         [[inputs]]\nname = \"repo\"\ndescription = \"owner/name\"\nrequired = true\n\
         [[inputs]]\nname = \"issue\"\ndescription = \"issue #\"\nrequired = true\ntype = \"integer\"\n",
    )
    .unwrap();
    std::fs::write(sd.join("SKILL.md"), "# Issue Crusher\nFix it.").unwrap();

    let skills = load_workflows(tmp.path());
    let s = skills
        .iter()
        .find(|s| s.definition.id == "issue-crusher")
        .expect("runtime skill loaded");
    assert_eq!(s.inputs.len(), 2);
    assert_eq!(s.inputs[1].kind.as_deref(), Some("integer"));
    match &s.definition.system_prompt {
        PromptSource::Inline(p) => assert!(p.contains("Fix it.")),
        other => panic!("expected inline prompt, got {other:?}"),
    }
}

#[test]
fn skill_md_only_install_resolves_by_dir_slug_not_frontmatter_name() {
    // Regression (#3987 codex review): a SKILL.md-only install whose
    // frontmatter `name` differs from its install slug must resolve via the
    // dir slug — the id surfaced in the list summary / orchestrator prompt /
    // uninstall — not the frontmatter name. Before the fix, `definition.id`
    // was built from `wf.name` ("My Cool Workflow"), so `get_workflow`
    // (keyed on the slug) returned None → "unknown skill".
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().join("skills").join("my-cool-workflow");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        "---\nname: My Cool Workflow\ndescription: does cool things\n---\n\n# Body\n",
    )
    .unwrap();

    let resolved = get_workflow(tmp.path(), "my-cool-workflow")
        .expect("SKILL.md-only install must resolve by its dir slug");
    assert_eq!(resolved.definition.id, "my-cool-workflow");
    // And NOT by the frontmatter name.
    assert!(
        get_workflow(tmp.path(), "My Cool Workflow").is_none(),
        "frontmatter name must not be the runnable id"
    );
}

#[test]
fn prune_removes_legacy_bundled_only() {
    let tmp = tempfile::TempDir::new().unwrap();
    let skills = tmp.path().join("skills");
    // A legacy bundled id + a user-authored workflow that must survive.
    for id in ["github-issue-crusher", "my-workflow"] {
        std::fs::create_dir_all(skills.join(id)).unwrap();
        std::fs::write(skills.join(id).join("SKILL.md"), "# x").unwrap();
    }
    prune_legacy_default_workflows(tmp.path());
    assert!(
        !skills.join("github-issue-crusher").exists(),
        "legacy bundled id should be pruned"
    );
    assert!(
        skills.join("my-workflow").exists(),
        "user-authored workflow must be left untouched"
    );
}

#[test]
fn skill_github_config_defaults_when_absent() {
    // No [github] block in skill.toml → `github` deserialises to None,
    // which the preflight reads as "gate disabled, skip silently".
    let toml = "id = \"x\"\nwhen_to_use = \"y\"\n";
    let parsed: WorkflowDefinition = toml::from_str(toml).expect("parse");
    assert!(parsed.github.is_none(), "no [github] block ⇒ None");
}

#[test]
fn skill_github_config_parses_full_block() {
    let toml = "id = \"x\"\nwhen_to_use = \"y\"\n\
                [github]\nrequired = true\nidentity_match = \"strict\"\n";
    let parsed: WorkflowDefinition = toml::from_str(toml).expect("parse");
    let gh = parsed.github.expect("github block present");
    assert!(gh.required);
    assert_eq!(gh.identity_match, IdentityMatch::Strict);
}

#[test]
fn skill_github_config_required_defaults_to_false() {
    // Block present but required not set ⇒ required = false (default).
    let toml = "id = \"x\"\nwhen_to_use = \"y\"\n\
                [github]\nidentity_match = \"any\"\n";
    let parsed: WorkflowDefinition = toml::from_str(toml).expect("parse");
    let gh = parsed.github.expect("github block present");
    assert!(!gh.required, "required defaults to false");
    assert_eq!(gh.identity_match, IdentityMatch::Any);
}

#[test]
fn skill_github_config_identity_match_defaults_to_strict() {
    let toml = "id = \"x\"\nwhen_to_use = \"y\"\n\
                [github]\nrequired = true\n";
    let parsed: WorkflowDefinition = toml::from_str(toml).expect("parse");
    let gh = parsed.github.expect("github block present");
    assert_eq!(
        gh.identity_match,
        IdentityMatch::Strict,
        "default is Strict"
    );
}

#[test]
fn skill_github_config_accepts_all_identity_match_variants() {
    for (variant, expected) in [
        ("strict", IdentityMatch::Strict),
        ("any", IdentityMatch::Any),
        ("none", IdentityMatch::None),
    ] {
        let toml = format!(
            "id = \"x\"\nwhen_to_use = \"y\"\n\
             [github]\nrequired = true\nidentity_match = \"{variant}\"\n"
        );
        let parsed: WorkflowDefinition = toml::from_str(&toml).expect("parse");
        assert_eq!(
            parsed.github.expect("github block present").identity_match,
            expected,
            "variant {variant} → {expected:?}",
        );
    }
}

#[test]
fn skill_github_config_serializes_lowercase() {
    let gh = WorkflowGithubConfig {
        required: true,
        identity_match: IdentityMatch::Strict,
    };
    let s = toml::to_string(&gh).expect("serialize");
    assert!(s.contains("required = true"));
    assert!(
        s.contains("identity_match = \"strict\""),
        "lowercase serialization: got {s}"
    );
}

/// One workflow lookup must run at most one full on-disk discovery pass.
#[test]
fn workflow_lookup_discovers_once() {
    use super::super::ops_discover::DISCOVERY_CALLS;

    let ws = tempfile::TempDir::new().unwrap();
    seed_runnable(&ws.path().join("skills"), "zzcount6166", "COUNT_BODY");

    DISCOVERY_CALLS.with(|c| c.set(0));
    let resolved = get_workflow(ws.path(), "zzcount6166").expect("workflow resolves");
    assert_eq!(resolved.definition.id, "zzcount6166");
    assert_eq!(
        DISCOVERY_CALLS.with(|c| c.get()),
        1,
        "lookup must discover the skills tree once"
    );

    // A miss walks the same fallback and must not discover twice either.
    DISCOVERY_CALLS.with(|c| c.set(0));
    assert!(get_workflow(ws.path(), "zznosuch6166").is_none());
    assert_eq!(
        DISCOVERY_CALLS.with(|c| c.get()),
        1,
        "an unresolvable id must discover the skills tree once, not twice"
    );
}
