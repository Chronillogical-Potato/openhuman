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
    let host: crate::agent::HostTools = Arc::new(|_| {
        crate::agent::HostTurnTools::advertised(vec![Box::new(Marker("oc_marker_tool"))])
    });

    let agent = crate::agent::OpenHumanSessionHost::from_config_with_host_tools(
        &config,
        &definition(),
        &host,
        None,
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
        Arc::new(move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            crate::agent::HostTurnTools::advertised(vec![Box::new(Marker("oc_marker_tool"))])
        })
    };

    for _ in 0..2 {
        crate::agent::OpenHumanSessionHost::from_config_with_host_tools(
            &config,
            &definition(),
            &host,
            None,
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

/// The factory is told which conversation it is building for.
///
/// Without this a host keying its belt on the chat has to read the occasion
/// out of state it closed over, which is correct only while the agent serves
/// one conversation at a time. Two at once and that state is a race, so the
/// occasion has to arrive as an argument.
#[test]
fn the_factory_is_told_the_session_it_is_building_for() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config = test_config(&tmp);
    let seen = Arc::new(std::sync::Mutex::new(Vec::<(String, Option<String>)>::new()));

    let host: crate::agent::HostTools = {
        let seen = Arc::clone(&seen);
        Arc::new(move |turn: crate::agent::TurnContext<'_>| {
            seen.lock().unwrap().push((
                turn.agent_id().to_owned(),
                turn.session_id().map(str::to_owned),
            ));
            // The belt itself varies with the occasion, which is the point.
            let name = match turn.session_id() {
                Some("desk:eng") => "oc_desk_tool",
                _ => "oc_plain_tool",
            };
            crate::agent::HostTurnTools::advertised(vec![Box::new(Marker(name))])
        })
    };

    let desked = crate::agent::OpenHumanSessionHost::from_config_with_host_tools(
        &config,
        &definition(),
        &host,
        Some("desk:eng"),
    )
    .expect("build a session for a named conversation");

    assert!(
        desked
            .visible_tool_specs_arc()
            .iter()
            .any(|spec| spec.name == "oc_desk_tool"),
        "the belt must be able to differ per conversation"
    );

    let unnamed = crate::agent::OpenHumanSessionHost::from_config_with_host_tools(
        &config,
        &definition(),
        &host,
        None,
    )
    .expect("build a session with no conversation named");

    assert!(
        unnamed
            .visible_tool_specs_arc()
            .iter()
            .any(|spec| spec.name == "oc_plain_tool"),
        "an unnamed turn is a case the host decides, not one it cannot observe"
    );

    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.iter()
            .map(|(agent, session)| (agent.as_str(), session.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (definition().id.as_str(), Some("desk:eng")),
            (definition().id.as_str(), None),
        ],
        "the factory is told the agent every time, and the session when one was named"
    );
}

/// A host tool wins a collision with a config-derived tool of the same name.
///
/// The rule only holds because the host belt is spliced in *first* and
/// `dedup_visible_tool_specs` keeps the first occurrence. Append instead and
/// the config spec is advertised while the host believes it replaced it --
/// the model told about one tool and a different one answering, which is the
/// version of this bug that is hardest to see from the outside.
#[test]
fn a_host_tool_overrides_a_config_tool_of_the_same_name() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config = test_config(&tmp);

    // A name the config-derived belt already carries, so the two collide.
    let contested =
        crate::agent::OpenHumanSessionHost::from_config_with_definition(&config, &definition())
            .expect("build a session without a host belt")
            .visible_tool_specs_arc()
            .first()
            .map(|spec| spec.name.clone())
            .expect("the config-derived belt advertises at least one tool");

    let host: crate::agent::HostTools = {
        let contested = contested.clone();
        Arc::new(move |_| {
            crate::agent::HostTurnTools::advertised(vec![Box::new(Marker(Box::leak(
                contested.clone().into_boxed_str(),
            )))])
        })
    };

    let agent = crate::agent::OpenHumanSessionHost::from_config_with_host_tools(
        &config,
        &definition(),
        &host,
        None,
    )
    .expect("build a session whose host belt collides");

    let advertised: Vec<_> = agent
        .visible_tool_specs_arc()
        .iter()
        .filter(|spec| spec.name == contested)
        .map(|spec| spec.description.clone())
        .collect();

    assert_eq!(
        advertised.len(),
        1,
        "a collision must leave exactly one advertised spec for {contested}"
    );
    assert_eq!(
        advertised[0], "a test marker",
        "the surviving spec must be the host's, not the config-derived one"
    );

    // Advertising the host's spec while executing the config's would be the
    // same bug wearing a disguise, so check the belt the driver resolves
    // against, not only what the provider was told.
    let executable = agent
        .tools()
        .iter()
        .find(|tool| tool.name() == contested)
        .map(|tool| tool.description().to_owned())
        .expect("the contested name is callable");
    assert_eq!(
        executable, "a test marker",
        "the tool that runs must be the host's, matching the spec advertised for it"
    );
}
