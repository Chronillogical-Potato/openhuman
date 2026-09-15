use super::*;
use serde_json::json;

#[test]
fn clawhub_download_url_uses_the_file_api_and_rejects_unsafe_slugs() {
    assert_eq!(
        clawhub_download_url("apple-design").as_deref(),
        Some("https://clawhub.ai/api/v1/skills/apple-design/file?path=SKILL.md")
    );
    for slug in ["", ".", "..", "a/b", "a?b", "a b"] {
        assert_eq!(clawhub_download_url(slug), None, "slug {slug:?}");
    }
}

#[test]
fn skills_sh_ref_parses_listing_urls_only() {
    let skill = SkillsShRef::parse("https://skills.sh/getagentseal/founder-playbook/100m-leads")
        .expect("skills.sh listing");
    assert_eq!(
        (skill.owner, skill.repo, skill.skill),
        ("getagentseal", "founder-playbook", "100m-leads")
    );
    for url in [
        "https://skills.sh/owner/repo",
        "https://skills.sh/o/r/s/extra",
        "https://skills.sh/o/../s",
        "https://example.com/o/r/s",
        "https://lobehub.com/agent/x",
    ] {
        assert!(SkillsShRef::parse(url).is_none(), "not a listing: {url}");
    }
}

#[test]
fn skills_sh_candidates_cover_the_conventional_skill_dirs() {
    let skill = SkillsShRef::parse("https://skills.sh/o/r/my-skill").unwrap();
    assert_eq!(
        skill.candidate_urls(),
        vec![
            "https://raw.githubusercontent.com/o/r/HEAD/my-skill/SKILL.md",
            "https://raw.githubusercontent.com/o/r/HEAD/skills/my-skill/SKILL.md",
            "https://raw.githubusercontent.com/o/r/HEAD/.agents/skills/my-skill/SKILL.md",
            "https://raw.githubusercontent.com/o/r/HEAD/.claude/skills/my-skill/SKILL.md",
        ]
    );
}

#[test]
fn find_skill_md_in_tree_matches_the_skill_directory_at_any_depth() {
    let tree = json!({ "tree": [
        { "path": "plugins/x/skills/my-skill", "type": "tree" },
        { "path": "plugins/x/skills/not-my-skill/SKILL.md", "type": "blob" },
        { "path": "plugins/x/skills/my-skill/SKILL.md", "type": "blob" },
    ]});
    assert_eq!(
        find_skill_md_in_tree(&tree, "my-skill"),
        Ok("plugins/x/skills/my-skill/SKILL.md".to_string())
    );
    let root = json!({ "tree": [{ "path": "my-skill/SKILL.md", "type": "blob" }] });
    assert_eq!(
        find_skill_md_in_tree(&root, "my-skill"),
        Ok("my-skill/SKILL.md".to_string())
    );
    assert_eq!(
        find_skill_md_in_tree(&tree, "other-skill"),
        Err(TreeMiss::Absent)
    );
}

#[test]
fn find_skill_md_in_tree_refuses_to_guess_between_same_named_directories() {
    let tree = json!({ "tree": [
        { "path": "plugins/a/my-skill/SKILL.md", "type": "blob" },
        { "path": "plugins/b/my-skill/SKILL.md", "type": "blob" },
    ]});
    assert_eq!(
        find_skill_md_in_tree(&tree, "my-skill"),
        Err(TreeMiss::Ambiguous(vec![
            "plugins/a/my-skill/SKILL.md".to_string(),
            "plugins/b/my-skill/SKILL.md".to_string(),
        ]))
    );
}

#[test]
fn find_skill_md_in_tree_does_not_call_a_truncated_listing_proof_of_absence() {
    let truncated = json!({
        "truncated": true,
        "tree": [{ "path": "other/SKILL.md", "type": "blob" }]
    });
    assert_eq!(
        find_skill_md_in_tree(&truncated, "my-skill"),
        Err(TreeMiss::Truncated)
    );
    // A match that did make it into a truncated listing is still usable.
    let found = json!({
        "truncated": true,
        "tree": [{ "path": "deep/my-skill/SKILL.md", "type": "blob" }]
    });
    assert_eq!(
        find_skill_md_in_tree(&found, "my-skill"),
        Ok("deep/my-skill/SKILL.md".to_string())
    );
}
