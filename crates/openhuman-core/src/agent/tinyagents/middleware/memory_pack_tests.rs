use super::*;

use std::sync::Arc;

use tinyagents_harness::context::RunConfig;
use tinyinference_llm::message::Message;

use crate::memory::lifecycle::hooks::{MemoryTurn, TurnPack, OPEN_TAG};
use crate::memory::scope::MemoryIdentity;

fn context(pack: Option<TurnPack>) -> RunContext<OpenHumanRunContext> {
    let config = crate::config::Config::default();
    let mut data = OpenHumanRunContext::new();
    data.memory_turn = Some(Arc::new(MemoryTurn {
        identity: MemoryIdentity::agent("a").resolve(&config),
        config: Arc::new(config),
        thread_id: "t".into(),
        pack,
    }));
    RunContext::new(RunConfig::new("memory-pack-test"), data)
}

fn pack() -> TurnPack {
    TurnPack {
        markdown: "# Memory\n\n## Learnings\n- The user prefers metric units".into(),
        tokens: 14,
        refs: vec!["id-1".into()],
        engine: "reference".into(),
        citations: Vec::new(),
        refusal: None,
    }
}

#[tokio::test]
async fn the_pack_rides_every_request_of_the_turn() {
    let mut ctx = context(Some(pack()));
    let history = vec![
        Message::system("You are helpful."),
        Message::user("How far is it?"),
    ];

    for _ in 0..2 {
        let mut request = ModelRequest::new(history.clone());
        MemoryPackMiddleware
            .before_model(&mut ctx, &(), &mut request)
            .await
            .unwrap();
        assert_eq!(request.messages.len(), 3, "added after the transcript");
        let tail = request.messages.last().unwrap().text();
        assert!(tail.starts_with(OPEN_TAG), "{tail}");
        assert!(tail.contains("metric units"));
        assert_eq!(
            &request.messages[..2],
            &history[..],
            "the transcript is untouched"
        );
    }
}

#[tokio::test]
async fn a_turn_without_a_pack_is_left_alone() {
    let mut ctx = context(None);
    let mut request = ModelRequest::new(vec![Message::user("hi")]);
    MemoryPackMiddleware
        .before_model(&mut ctx, &(), &mut request)
        .await
        .unwrap();
    assert_eq!(request.messages.len(), 1);
    assert!(ctx.data.child().memory_turn.is_none());
}
