use super::*;

#[tokio::test]
async fn prompt_cache_segments_fingerprint_full_tool_schema() {
    let mw = PromptCacheSegmentMiddleware;
    let mut first =
        ModelRequest::new(vec![TaMessage::system("sys")]).with_tools(vec![ToolSchema::new(
            "lookup",
            "lookup a user",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                },
            }),
        )]);
    mw.before_model(&mut ctx(), &(), &mut first).await.unwrap();

    let mut second =
        ModelRequest::new(vec![TaMessage::system("sys")]).with_tools(vec![ToolSchema::new(
            "lookup",
            "lookup a user",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "integer" }
                },
            }),
        )]);
    mw.before_model(&mut ctx(), &(), &mut second).await.unwrap();

    let first_tool_segment = first
        .cache_segments
        .iter()
        .find(|segment| segment.role == SegmentRole::Tools)
        .expect("tool segment");
    let second_tool_segment = second
        .cache_segments
        .iter()
        .find(|segment| segment.role == SegmentRole::Tools)
        .expect("tool segment");

    // Segment ids are the harness-layout constants, never content-suffixed:
    // `refresh_prompt_cache_fingerprint` only recognises exactly `system` /
    // `tools`, and any other id is fingerprinted over the whole request, which
    // re-rolls the provider `prompt_cache_key` (OpenRouter's sticky-routing
    // key) on every call.
    assert_eq!(first_tool_segment.id, "tools");
    assert_eq!(second_tool_segment.id, "tools");
    assert!(first.cache_segments.iter().all(|s| s.cacheable));
    assert_eq!(
        first
            .cache_segments
            .iter()
            .find(|s| s.role == SegmentRole::System)
            .expect("system segment")
            .id,
        "system"
    );
    // The content difference is carried by the request fingerprint instead.
    assert_ne!(
        first.prompt_fingerprint, second.prompt_fingerprint,
        "same-name tools with different schemas must bust the stable prefix"
    );
    assert_eq!(
        first.prompt_fingerprint.as_deref().unwrap().len(),
        64,
        "request prompt fingerprints use TinyAgents' SHA-256 shape"
    );
}

#[tokio::test]
async fn prompt_cache_segments_are_stable_across_a_threads_turns() {
    // The whole point: two calls of one thread — same system prompt, same
    // tools, longer conversation — must declare identical segments and an
    // identical request fingerprint, so the provider routing key derived from
    // them (`tap-<fingerprint>`) does not change turn to turn.
    let mw = PromptCacheSegmentMiddleware;
    let tools = vec![ToolSchema::new(
        "lookup",
        "lookup a user",
        json!({ "type": "object", "properties": { "id": { "type": "string" } } }),
    )];
    let mut turn_one = ModelRequest::new(vec![TaMessage::system("sys"), TaMessage::user("hi")])
        .with_tools(tools.clone());
    let mut turn_two = ModelRequest::new(vec![
        TaMessage::system("sys"),
        TaMessage::user("hi"),
        TaMessage::assistant("hello"),
        TaMessage::user("and again, later"),
    ])
    .with_tools(tools);
    mw.before_model(&mut ctx(), &(), &mut turn_one)
        .await
        .unwrap();
    mw.before_model(&mut ctx(), &(), &mut turn_two)
        .await
        .unwrap();

    let ids = |r: &ModelRequest| {
        r.cache_segments
            .iter()
            .map(|s| (s.id.clone(), s.role, s.cacheable))
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&turn_one), ids(&turn_two));
    assert_eq!(
        ids(&turn_one),
        vec![
            ("system".to_string(), SegmentRole::System, true),
            ("tools".to_string(), SegmentRole::Tools, true),
        ]
    );
    assert_eq!(turn_one.prompt_fingerprint, turn_two.prompt_fingerprint);
    assert!(turn_one.prompt_fingerprint.is_some());
}

#[tokio::test]
async fn prompt_cache_segments_ignore_system_nudges_after_the_user_turn() {
    let mw = PromptCacheSegmentMiddleware;
    let tools = vec![ToolSchema::new(
        "shell",
        "run a command",
        json!({"type": "object"}),
    )];
    let prefix = vec![
        TaMessage::system("stable"),
        TaMessage::system("context"),
        TaMessage::user("work on this"),
    ];
    let mut before = ModelRequest::new(prefix.clone()).with_tools(tools.clone());
    let mut after = ModelRequest::new(
        prefix
            .into_iter()
            .chain([TaMessage::system(
                "[no progress since step 4] change strategy",
            )])
            .collect(),
    )
    .with_tools(tools);

    mw.before_model(&mut ctx(), &(), &mut before).await.unwrap();
    mw.before_model(&mut ctx(), &(), &mut after).await.unwrap();

    assert_eq!(before.cache_segments, after.cache_segments);
    assert_eq!(before.prompt_fingerprint, after.prompt_fingerprint);
}

#[tokio::test]
async fn prompt_cache_segments_do_not_promote_a_compaction_summary() {
    let mw = PromptCacheSegmentMiddleware;
    let mut run = ctx();
    run.data.cacheable_system_prefix_len = Some(2);
    let mut before = ModelRequest::new(vec![
        TaMessage::system("stable"),
        TaMessage::system("context"),
        TaMessage::user("first"),
    ]);
    let mut after = ModelRequest::new(vec![
        TaMessage::system("stable"),
        TaMessage::system("context"),
        TaMessage::system("changing history summary"),
        TaMessage::user("later"),
    ]);

    mw.before_model(&mut run, &(), &mut before).await.unwrap();
    mw.before_model(&mut run, &(), &mut after).await.unwrap();

    assert_eq!(before.cache_segments, after.cache_segments);
    assert_eq!(before.prompt_fingerprint, after.prompt_fingerprint);
}

#[tokio::test]
async fn prompt_cache_segments_keep_an_unrecoverable_prefix_uncacheable() {
    let mw = PromptCacheSegmentMiddleware;
    let mut run = ctx();
    run.data.cacheable_system_prefix_len = Some(0);
    let mut request = ModelRequest::new(vec![
        TaMessage::system("history summary"),
        TaMessage::user("continue"),
    ]);

    mw.before_model(&mut run, &(), &mut request).await.unwrap();

    assert_eq!(request.cache_segments.len(), 1);
    assert_eq!(request.cache_segments[0].role, SegmentRole::Volatile);
    assert!(!request.cache_segments[0].cacheable);
}

#[tokio::test]
async fn prompt_cache_segments_name_each_system_tier_and_skip_tools_under_a_text_dialect() {
    // Two leading system messages (stable+context, then volatile) are two
    // segments named the way the harness's `refresh_prompt_cache_fingerprint`
    // expects (`system`, `system.1`). Under a text dialect the harness folds
    // the catalogue into the prompt and clears `tools` after this hook, so
    // no `tools` segment is declared: declaring one would not match the
    // rebuilt layout and would demote the request to a per-call digest.
    let mw = PromptCacheSegmentMiddleware;
    let tools = vec![ToolSchema::new(
        "lookup",
        "lookup a user",
        json!({ "type": "object", "properties": { "id": { "type": "string" } } }),
    )];
    let messages = vec![
        TaMessage::system("stable"),
        TaMessage::system("volatile"),
        TaMessage::user("hi"),
    ];
    let ids = |r: &ModelRequest| {
        r.cache_segments
            .iter()
            .map(|s| (s.id.clone(), s.role))
            .collect::<Vec<_>>()
    };

    let mut native = ModelRequest::new(messages.clone()).with_tools(tools.clone());
    mw.before_model(&mut ctx(), &(), &mut native).await.unwrap();
    assert_eq!(
        ids(&native),
        vec![
            ("system".to_string(), SegmentRole::System),
            ("system.1".to_string(), SegmentRole::System),
            ("tools".to_string(), SegmentRole::Tools),
        ]
    );

    let mut python_ctx = ctx();
    python_ctx.data = python_ctx
        .data
        .clone()
        .with_tool_dialect(tinyagents_harness::config::ToolDispatcher::Python);
    let mut python = ModelRequest::new(messages).with_tools(tools);
    mw.before_model(&mut python_ctx, &(), &mut python)
        .await
        .unwrap();
    assert_eq!(
        ids(&python),
        vec![
            ("system".to_string(), SegmentRole::System),
            ("system.1".to_string(), SegmentRole::System),
        ]
    );
    assert!(python.prompt_fingerprint.is_some());
}
