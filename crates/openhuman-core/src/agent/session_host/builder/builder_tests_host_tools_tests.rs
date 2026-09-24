//! A host's own `dyn Tool` on a session built from config.
//!
//! The seam exists so an embedder does not have to reach its tools over MCP,
//! which costs the model a discovery call and an `arguments` object no
//! provider can validate. These assert the two halves that make a host tool a
//! real tool: it is on the belt, and it is advertised.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A tool that does nothing but be present under a name nothing else uses.
#[derive(Debug)]
struct Marker(&'static str);

#[async_trait::async_trait]
impl tinytools::Tool for Marker {
    fn name(&self) -> &str {
        self.0
    }

    fn description(&self) -> &str {
        "a test marker"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<tinytools::ToolResult> {
        Ok(tinytools::ToolResult::success("ok"))
    }
}

fn definition() -> crate::agent::harness::definition::AgentDefinition {
    crate::agent::harness::AgentDefinitionRegistry::builtins_only()
        .get("tools_agent")
        .cloned()
        .expect("tools_agent built-in definition")
}

/// The whole point: a name the host supplied is callable, and the model is
/// told about it. Advertised-but-absent is a call that fails; present-but-
/// unadvertised is a tool the model never reaches for.
#[test]
fn a_host_tool_is_on_the_belt_and_advertised() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config = test_config(&tmp);
    let host: crate::agent::HostTools = Arc::new(|| {
        crate::agent::HostTurnTools::advertised(vec![Box::new(Marker("oc_marker_tool"))])
    });

    let agent = crate::agent::OpenHumanSessionHost::from_config_with_host_tools(
        &config,
        &definition(),
        &host,
    )
    .expect("build a session with a host belt");

    assert!(
        agent
            .visible_tool_specs_arc()
            .iter()
            .any(|spec| spec.name == "oc_marker_tool"),
        "a host tool must be advertised to the provider by its own name"
    );
}

/// Without this the seam would be a belt, not a factory, and a host whose
/// tools belong to something shorter-lived than the agent -- one episode, one
/// room -- would have to register a second agent to express that. It is also
/// what makes the per-turn rebuild survivable at all: `Box<dyn Tool>` is not
/// `Clone`.
#[test]
fn the_factory_runs_once_per_session_build() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config = test_config(&tmp);
    let calls = Arc::new(AtomicUsize::new(0));
    let host: crate::agent::HostTools = {
        let calls = Arc::clone(&calls);
        Arc::new(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            crate::agent::HostTurnTools::advertised(vec![Box::new(Marker("oc_marker_tool"))])
        })
    };

    for _ in 0..2 {
        crate::agent::OpenHumanSessionHost::from_config_with_host_tools(
            &config,
            &definition(),
            &host,
        )
        .expect("build a session with a host belt");
    }

    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the belt is rebuilt per session build, so the factory is asked each time"
    );
}

/// A session built the ordinary way must be byte-identical to before this
/// seam existed -- the factory is opt-in, and a `None` host adds nothing.
#[test]
fn no_host_belt_leaves_the_advertised_set_alone() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config = test_config(&tmp);
    let agent =
        crate::agent::OpenHumanSessionHost::from_config_with_definition(&config, &definition())
            .expect("build a plain session");

    assert!(
        !agent
            .visible_tool_specs_arc()
            .iter()
            .any(|spec| spec.name == "oc_marker_tool"),
        "nothing should advertise a host tool that was never supplied"
    );
}
