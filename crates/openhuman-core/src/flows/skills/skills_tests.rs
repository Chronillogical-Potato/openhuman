use super::*;

#[test]
fn bundled_skill_matches_the_portable_resource_list() {
    let mut listed: Vec<&str> = FLOW_AUTHORING
        .files
        .iter()
        .map(|f| f.path)
        .collect();
    listed.sort();
    let mut portable: Vec<&str> = tinyflows_copilot::resources::FLOW_AUTHORING_FILES
        .iter()
        .map(|f| f.path)
        .collect();
    portable.sort();

    assert_eq!(
        listed, portable,
        "OpenHuman's bundled-skill registration and tinyflows' portable resource list diverged"
    );
}

#[test]
fn every_page_the_manifest_advertises_exists() {
    // The WORKFLOW.md table is what the model reads to choose a page. A
    // row naming a file that does not ship is a dead end the model cannot
    // diagnose — it just gets an error and gives up on the manual.
    let manifest = FLOW_AUTHORING
        .files
        .iter()
        .find(|f| f.path == "WORKFLOW.md")
        .expect("manifest")
        .contents;
    for file in FLOW_AUTHORING.files {
        if file.path == "WORKFLOW.md" {
            continue;
        }
        assert!(
            manifest.contains(file.path),
            "`{}` ships but the manifest's table never names it",
            file.path
        );
    }
    for line in manifest.lines() {
        for token in line.split('`') {
            if token.starts_with("references/") {
                assert!(
                    FLOW_AUTHORING.files.iter().any(|f| f.path == token),
                    "the manifest points at `{token}`, which does not ship"
                );
            }
        }
    }
}

#[test]
fn the_frontmatter_description_does_not_advertise_a_dropped_page() {
    // The description is what the model reads in the `## Installed Skills`
    // catalogue to decide whether to open the skill at all, and it is prose
    // rather than a path — so the `references/` token check below cannot
    // see it. It went stale the first time a page moved back into the
    // standing prompt: the description still promised graph sizing after
    // `graph-shape.md` was deleted.
    let manifest = FLOW_AUTHORING
        .files
        .iter()
        .find(|f| f.path == "WORKFLOW.md")
        .expect("manifest")
        .contents;
    let description = manifest
        .lines()
        .find(|l| l.starts_with("description:"))
        .expect("frontmatter description");
    for dropped in ["how large a graph", "graph should be", "graph-shape"] {
        assert!(
            !description.contains(dropped),
            "the description still advertises `{dropped}`, which no longer ships"
        );
    }
}
