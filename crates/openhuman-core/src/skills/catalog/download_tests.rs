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
        find_skill_md_in_tree(&tree, "my-skill").as_deref(),
        Some("plugins/x/skills/my-skill/SKILL.md")
    );
    let root = json!({ "tree": [{ "path": "my-skill/SKILL.md", "type": "blob" }] });
    assert_eq!(
        find_skill_md_in_tree(&root, "my-skill").as_deref(),
        Some("my-skill/SKILL.md")
    );
    assert_eq!(find_skill_md_in_tree(&tree, "other-skill"), None);
}
