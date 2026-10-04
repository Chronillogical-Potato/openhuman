use super::*;
use crate::memory::ops::learn;
use crate::memory::test_fixtures::{bind_reference, config_in};
use crate::memory::types::LearnParams;
use tinymemory::LearningKind;

fn params(text: &str, kind: LearningKind) -> LearnParams {
    LearnParams {
        text: text.to_string(),
        kind: Some(kind),
        confidence: Some(0.9),
        meta: None,
    }
}

#[test]
fn view_of_a_fresh_workspace_is_empty_with_the_configured_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let view = view(&config);
    assert!(view.markdown.is_empty());
    assert_eq!(view.tokens, 0);
    assert!(view.generated_at.is_none());
    assert_eq!(view.interval_mins, config.memory.context.interval_mins);
    assert_eq!(view.budget_tokens, config.memory.context.budget_tokens);
    assert!(view.enabled);
}

#[tokio::test]
async fn refresh_without_an_engine_reports_memory_off() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let error = refresh(&config).await.unwrap_err();
    assert_eq!(error.code(), crate::memory::error::MEMORY_OFF);
    assert!(!context_path(&config.workspace_dir).exists());
}

#[tokio::test]
async fn refresh_writes_context_md_and_state() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    bind_reference(&config);
    learn(
        &config,
        params("The user prefers terse answers", LearningKind::Preference),
        None,
    )
    .await
    .unwrap();

    let view = refresh(&config).await.unwrap();
    let path = config.workspace_dir.join("memory").join("context.md");
    assert_eq!(context_path(&config.workspace_dir), path);
    let on_disk = std::fs::read_to_string(&path).expect("context.md written");
    assert_eq!(view.markdown, on_disk);
    assert!(view.generated_at.is_some());
    assert!(view.tokens > 0);
    assert!(state_path(&config.workspace_dir, &Namespace::ROOT).exists());
}

#[test]
fn injection_block_wraps_the_body_and_strips_frontmatter() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    crate::memory::test_fixtures::bind_reference(&config);
    std::fs::create_dir_all(config.workspace_dir.join("memory")).unwrap();
    std::fs::write(
        context_path(&config.workspace_dir),
        "---\ngenerated: now\n---\n\n# Brief\n\nLikes tea.\n",
    )
    .unwrap();
    let block = injection_block(&config).expect("a block");
    assert!(block.starts_with(OPEN_TAG));
    assert!(block.ends_with(CLOSE_TAG));
    assert!(block.contains("Likes tea."));
    assert!(!block.contains("generated: now"));
    assert!(!block.contains("---"));

    let message = prepend_to_first_message(&config, "hello");
    assert!(message.starts_with(OPEN_TAG));
    assert!(message.ends_with("\n\nhello"));
}

#[test]
fn injection_block_is_none_when_disabled_off_or_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(&tmp);

    // Memory off, even with a document on disk.
    std::fs::create_dir_all(config.workspace_dir.join("memory")).unwrap();
    std::fs::write(context_path(&config.workspace_dir), "Likes tea.").unwrap();
    assert!(injection_block(&config).is_none());
    assert_eq!(prepend_to_first_message(&config, "hello"), "hello");

    // On, but nothing compiled yet.
    let tmp2 = tempfile::tempdir().unwrap();
    let config2 = config_in(&tmp2);
    bind_reference(&config2);
    assert!(injection_block(&config2).is_none());

    // On, but only frontmatter and whitespace.
    std::fs::create_dir_all(config2.workspace_dir.join("memory")).unwrap();
    std::fs::write(context_path(&config2.workspace_dir), "---\na: b\n---\n  \n").unwrap();
    assert!(injection_block(&config2).is_none());

    // On with content, but disabled by config.
    bind_reference(&config);
    assert!(injection_block(&config).is_some());
    config.memory.context.enabled = false;
    assert!(injection_block(&config).is_none());
}

#[test]
fn an_unterminated_frontmatter_is_left_alone() {
    assert_eq!(strip_frontmatter("---\nno end"), "---\nno end");
    assert_eq!(strip_frontmatter("plain"), "plain");
    assert_eq!(strip_frontmatter("---\na: 1\n---\nbody"), "body");
}

#[test]
fn apply_set_validates_bounds_and_applies_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(&tmp);

    for bad in [
        ContextSetParams {
            interval_mins: Some(MIN_INTERVAL_MINS - 1),
            ..ContextSetParams::default()
        },
        ContextSetParams {
            budget_tokens: Some(MIN_BUDGET_TOKENS - 1),
            ..ContextSetParams::default()
        },
        ContextSetParams {
            budget_tokens: Some(MAX_BUDGET_TOKENS + 1),
            ..ContextSetParams::default()
        },
    ] {
        let error = apply_set(&mut config, &bad).unwrap_err();
        assert_eq!(error.code(), crate::memory::error::INVALID_REQUEST);
    }

    apply_set(
        &mut config,
        &ContextSetParams {
            enabled: Some(false),
            interval_mins: Some(MIN_INTERVAL_MINS),
            budget_tokens: Some(MAX_BUDGET_TOKENS),
        },
    )
    .unwrap();
    let settings = &config.memory.context;
    assert!(!settings.enabled);
    assert_eq!(settings.interval_mins, MIN_INTERVAL_MINS);
    assert_eq!(settings.budget_tokens, MAX_BUDGET_TOKENS);
}

#[test]
fn spec_for_never_goes_below_the_minimum_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(&tmp);
    config.memory.context.budget_tokens = 1;
    assert_eq!(spec_for(&config).budget_tokens, MIN_BUDGET_TOKENS as usize);
}

fn as_agent(agent: &str) -> tinymemory::MemoryMeta {
    tinymemory::MemoryMeta {
        namespace: Namespace::agent(agent),
        agent_id: Some(agent.to_string()),
        ..tinymemory::MemoryMeta::default()
    }
}

#[tokio::test]
async fn each_agent_gets_its_own_context_md_from_its_reach() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(&tmp);
    config.memory.agents.insert(
        "planner".into(),
        crate::config::schema::MemoryAgentConfig::default(),
    );
    bind_reference(&config);
    let researcher = Namespace::agent("researcher");
    for (text, meta) in [
        ("The user is called Sam", None),
        ("Prefer arxiv for papers", Some(as_agent("researcher"))),
        ("Draft in British English", Some(as_agent("writer"))),
    ] {
        learn(&config, params(text, LearningKind::Preference), meta)
            .await
            .unwrap();
    }

    let view = refresh_for(&config, &researcher).await.unwrap();
    assert_eq!(view.namespace, "agent:researcher");
    assert!(view.markdown.contains("arxiv"), "{}", view.markdown);
    assert!(view.markdown.contains("Sam"), "the root is inherited");
    assert!(
        !view.markdown.contains("British"),
        "a sibling is out of reach"
    );
    let path = context_path_for(&config.workspace_dir, &researcher);
    assert!(path.ends_with("memory/context/agent-researcher/context.md"));
    assert!(path.is_file());
    assert_eq!(
        compiled_nodes(&config.workspace_dir),
        vec![Namespace::ROOT, researcher.clone()]
    );

    // The cron job compiles the root, every compiled node and every
    // configured agent.
    assert_eq!(refresh_all(&config).await, 3);
    assert!(context_path_for(&config.workspace_dir, &Namespace::agent("planner")).is_file());

    let injected = crate::memory::scope::within_agent("researcher", async {
        injection_block(&config).unwrap()
    })
    .await;
    assert!(injected.contains("arxiv"));
    let main = injection_block(&config).unwrap();
    assert!(!main.contains("arxiv"), "the root's document is the root's");
}

#[tokio::test]
async fn an_agent_without_a_document_falls_back_to_its_ancestor_s() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = config_in(&tmp);
    bind_reference(&config);
    learn(
        &config,
        params("The user is called Sam", LearningKind::Fact),
        None,
    )
    .await
    .unwrap();
    refresh(&config).await.unwrap();
    let injected =
        crate::memory::scope::within_agent("scout", async { injection_block(&config) }).await;
    assert!(injected.unwrap().contains("Sam"));

    config.memory.agents.insert(
        "quiet".into(),
        crate::config::schema::MemoryAgentConfig {
            context: Some(false),
            ..Default::default()
        },
    );
    let none =
        crate::memory::scope::within_agent("quiet", async { injection_block(&config) }).await;
    assert!(none.is_none(), "an agent can switch its context off");
}
