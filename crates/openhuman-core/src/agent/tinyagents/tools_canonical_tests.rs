use super::*;

use std::sync::atomic::{AtomicBool, Ordering};

use tinytools::{ToolContent, ToolPolicy};

struct RecordingTool {
    saw_context: AtomicBool,
}

#[async_trait]
impl Tool for RecordingTool {
    fn name(&self) -> &str {
        "recording"
    }

    fn description(&self) -> &str {
        "Preserves the canonical TinyTools contract."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {"fail": {"type": "boolean"}}})
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy::read_only().requiring_approval()
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        self.execute_with_context(args, ToolCallOptions::default(), None)
            .await
    }

    async fn execute_with_context(
        &self,
        args: serde_json::Value,
        options: ToolCallOptions,
        context: Option<&dyn ToolRunContext>,
    ) -> anyhow::Result<ToolResult> {
        self.saw_context.store(
            context.is_some() && options.prefer_markdown,
            Ordering::SeqCst,
        );
        Ok(ToolResult {
            content: vec![
                ToolContent::Text {
                    text: "plain content".to_string(),
                },
                ToolContent::Json {
                    data: serde_json::json!({"preserved": true}),
                },
            ],
            is_error: args["fail"].as_bool().unwrap_or(false),
            markdown_formatted: Some("markdown content".to_string()),
        })
    }
}

struct TestToolRunContext;
impl ToolRunContext for TestToolRunContext {}

#[tokio::test]
async fn canonical_adapter_preserves_spec_policy_context_and_result() {
    let seen_context = Arc::new(AtomicBool::new(false));
    struct SharedRecordingTool(Arc<AtomicBool>);
    #[async_trait]
    impl Tool for SharedRecordingTool {
        fn name(&self) -> &str {
            "recording"
        }
        fn description(&self) -> &str {
            "Preserves the canonical TinyTools contract."
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        fn policy(&self) -> ToolPolicy {
            ToolPolicy::read_only().requiring_approval()
        }
        async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
            self.execute_with_context(args, ToolCallOptions::default(), None)
                .await
        }
        async fn execute_with_context(
            &self,
            args: serde_json::Value,
            options: ToolCallOptions,
            context: Option<&dyn ToolRunContext>,
        ) -> anyhow::Result<ToolResult> {
            self.0.store(
                context.is_some() && options.prefer_markdown,
                Ordering::SeqCst,
            );
            Ok(ToolResult {
                content: vec![
                    ToolContent::Text {
                        text: "plain content".into(),
                    },
                    ToolContent::Json {
                        data: serde_json::json!({"preserved": true}),
                    },
                ],
                is_error: args["fail"].as_bool().unwrap_or(false),
                markdown_formatted: Some("markdown content".into()),
            })
        }
    }
    let sets: Vec<Arc<Vec<Box<dyn Tool>>>> = vec![Arc::new(vec![Box::new(SharedRecordingTool(
        seen_context.clone(),
    ))])];
    let adapter = CanonicalSharedToolAdapter::for_name(sets, "recording").expect("registered tool");

    assert_eq!(adapter.name(), "recording");
    assert_eq!(
        adapter.description(),
        "Preserves the canonical TinyTools contract."
    );
    assert_eq!(
        adapter.parameters_schema(),
        serde_json::json!({"type": "object"})
    );
    assert_eq!(
        adapter.policy(),
        ToolPolicy::read_only().requiring_approval()
    );

    let result = adapter
        .execute_with_context(
            serde_json::json!({"fail": true}),
            ToolCallOptions::prefer_markdown(),
            Some(&TestToolRunContext),
        )
        .await
        .expect("canonical result");
    assert!(seen_context.load(Ordering::SeqCst));
    assert!(result.is_error);
    assert_eq!(
        result.markdown_formatted.as_deref(),
        Some("markdown content")
    );
    assert_eq!(result.content.len(), 2);
    assert!(matches!(result.content[0], ToolContent::Text { ref text } if text == "plain content"));
    assert!(
        matches!(result.content[1], ToolContent::Json { ref data } if data == &serde_json::json!({"preserved": true}))
    );
}

#[tokio::test]
async fn canonical_adapter_fails_closed_when_the_registered_tool_is_gone() {
    let adapter = CanonicalSharedToolAdapter {
        sets: Vec::new(),
        name: "missing".to_string(),
        description: "missing".to_string(),
        parameters_schema: serde_json::json!({"type": "object"}),
        early_exit: None,
    };

    let result = adapter
        .execute(serde_json::json!({}))
        .await
        .expect("reported failure");
    assert!(result.is_error);
    assert_eq!(result.output(), "unknown tool 'missing'");
}

#[tokio::test]
async fn early_exit_only_fires_after_a_successful_canonical_result() {
    let sets: Vec<Arc<Vec<Box<dyn Tool>>>> = vec![Arc::new(vec![Box::new(RecordingTool {
        saw_context: AtomicBool::new(false),
    })])];
    let hook = EarlyExitHook::new(SteeringHandle::allow_all());
    let adapter = CanonicalSharedToolAdapter::for_name(sets, "recording")
        .expect("registered tool")
        .with_early_exit(hook.clone());

    let failed = adapter
        .execute(serde_json::json!({"fail": true}))
        .await
        .expect("reported failure");
    assert!(failed.is_error);
    assert!(
        hook.take().is_none(),
        "reported tool errors must not pause the run"
    );

    let successful = adapter
        .execute(serde_json::json!({"fail": false}))
        .await
        .expect("success");
    assert!(!successful.is_error);
    let early_exit = hook
        .take()
        .expect("successful early-exit tool pauses the run");
    assert_eq!(early_exit.tool, "recording");
    assert_eq!(early_exit.question, "markdown content");
}
