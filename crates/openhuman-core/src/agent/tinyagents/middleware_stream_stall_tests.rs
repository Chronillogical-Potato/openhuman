use super::*;

#[tokio::test]
async fn repetitive_stream_is_stopped_before_it_reaches_the_ui_indefinitely() {
    let mw = StreamStallMiddleware::new();
    let mut run = ctx();
    let mut stopped = false;
    for i in 0..12 {
        let mut delta = tinyinference_llm::model::ModelDelta {
            call_id: "jev-research".to_string(),
            content: format!("Let me check another page number {i} before answering the user. "),
            reasoning: String::new(),
            tool_call: None,
        };
        if mw.on_model_delta(&mut run, &(), &mut delta).await.is_err() {
            stopped = true;
            break;
        }
    }
    assert!(stopped);
}

#[tokio::test]
async fn tool_progress_and_a_new_model_call_start_fresh_windows() {
    let mw = StreamStallMiddleware::new();
    let mut run = ctx();
    for call_id in ["first-call", "second-call"] {
        for i in 0..7 {
            let mut delta = tinyinference_llm::model::ModelDelta {
                call_id: call_id.to_string(),
                content: format!(
                    "Let me check source number {i} before answering the user's request. "
                ),
                reasoning: String::new(),
                tool_call: None,
            };
            mw.on_model_delta(&mut run, &(), &mut delta).await.unwrap();
        }
    }
    let mut tool_delta = tinyinference_llm::model::ModelDelta {
        call_id: "second-call".to_string(),
        content: String::new(),
        reasoning: String::new(),
        tool_call: Some(tinyinference_llm::tool::ToolDelta {
            call_id: "fetch-1".to_string(),
            content: String::new(),
            tool_name: Some("web_fetch".to_string()),
            content_index: None,
        }),
    };
    mw.on_model_delta(&mut run, &(), &mut tool_delta)
        .await
        .unwrap();
    for i in 0..7 {
        let mut delta = tinyinference_llm::model::ModelDelta {
            call_id: "second-call".to_string(),
            content: format!("Let me inspect result number {i} before answering the user. "),
            reasoning: String::new(),
            tool_call: None,
        };
        mw.on_model_delta(&mut run, &(), &mut delta).await.unwrap();
    }
}
