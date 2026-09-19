//! `flows_build` must not open a new flow with another flow's builder session.
//!
//! Found by a real-token baseline run: a `flows_build` resumed the previous
//! day's builder session, because the empty-history fallback
//! (`try_load_session_transcript`) loads the newest transcript for the agent
//! *name*, and every flow's builder is named `workflow_builder`.

use super::*;
use tinyagents_session::transcript::{
    SessionTranscript, TranscriptHistory, TranscriptLocator, TranscriptMeta, TranscriptRead,
};

const FLOW_A: &str = "flow A: forward my invoices to accounting";

/// What a brand-new flow B sees on disk: flow A's builder transcript is the
/// newest for the agent name, and nothing exists for B's thread.
struct OtherFlowOnly {
    handle: Arc<FakeSessionHistory>,
}

impl TranscriptLocator for OtherFlowOnly {
    fn latest_for_agent(&self, _agent_name: &str) -> Option<Arc<dyn TranscriptRead>> {
        Some(self.handle.clone())
    }

    fn root_for_thread(&self, _thread_id: &str) -> Option<Arc<dyn TranscriptRead>> {
        None
    }

    fn open_stem(&self, _stem: &str, _seed: TranscriptMeta) -> Result<Arc<dyn TranscriptHistory>> {
        Ok(self.handle.clone())
    }
}

/// Records the text of every message sent to the model.
#[derive(Default)]
struct SeenText(Mutex<String>);

#[async_trait]
impl ChatModel<()> for SeenText {
    fn profile(&self) -> Option<&ModelProfile> {
        static PROFILE: std::sync::LazyLock<ModelProfile> =
            std::sync::LazyLock::new(ModelProfile::default);
        Some(&PROFILE)
    }

    async fn invoke(
        &self,
        _state: &(),
        request: ModelRequest,
    ) -> tinyinference::Result<ModelResponse> {
        for message in &request.messages {
            self.0.lock().push_str(&message.text());
        }
        let response = ChatResponse {
            text: Some("ok".into()),
            tool_calls: vec![],
            usage: None,
            reasoning_content: None,
        };
        Ok(crate::agent::tinyagents::model::native_model_response_for_request(&response, &request))
    }

    async fn stream(
        &self,
        state: &(),
        request: ModelRequest,
    ) -> tinyinference::Result<ModelStream> {
        let response = self.invoke(state, request).await?;
        Ok(Box::pin(futures::stream::iter(vec![
            ModelStreamItem::Started,
            ModelStreamItem::Completed(response),
        ])))
    }
}

fn builder_agent(workspace: &std::path::Path) -> (Agent, Arc<SeenText>) {
    let handle = Arc::new(FakeSessionHistory {
        path: workspace.join("session_raw").join("flow_a.jsonl"),
        canned: Some(SessionTranscript {
            meta: fake_transcript_meta("thr_flow_a"),
            messages: durable_messages(vec![
                crate::agent::messages::ChatMessage::system("builder system prompt"),
                crate::agent::messages::ChatMessage::user(FLOW_A),
                crate::agent::messages::ChatMessage::assistant("proposed flow A"),
            ]),
        }),
        appended: Mutex::new(Vec::new()),
    });
    let model = Arc::new(SeenText::default());
    let agent = Agent::builder()
        .chat_model(model.clone())
        .tools(vec![Box::new(MockTool)])
        .memory(crate::memory::test_support::noop_memory())
        .tool_dispatcher(Box::new(NativeDialect))
        .agent_definition_name("workflow_builder")
        .workspace_dir(workspace.to_path_buf())
        .with_session_history_locator(Arc::new(OtherFlowOnly { handle }))
        .build()
        .expect("agent build should succeed");
    (agent, model)
}

#[tokio::test]
async fn a_new_flow_builder_turn_does_not_resume_another_flows_session() {
    // Control: left alone, the by-name fallback splices flow A into the turn.
    // Without this the assertion below could pass on a fixture that never
    // reaches the request at all.
    let workspace = tempfile::TempDir::new().expect("temp workspace");
    let (mut agent, model) = builder_agent(workspace.path());
    agent.turn("build flow B").await.unwrap();
    assert!(
        model.0.lock().contains(FLOW_A),
        "fixture must reach the request through the by-name fallback"
    );

    let workspace = tempfile::TempDir::new().expect("temp workspace");
    let (mut agent, model) = builder_agent(workspace.path());
    crate::flows::ops::start_builder_turn_clean(&mut agent);
    agent.turn("build flow B").await.unwrap();
    let seen = model.0.lock().clone();
    assert!(seen.contains("build flow B"), "{seen}");
    assert!(
        !seen.contains(FLOW_A),
        "a new flow's builder turn carried another flow's session:\n{seen}"
    );
}
