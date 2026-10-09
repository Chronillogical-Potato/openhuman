//! `user_agents.*` controllers: the SaaS operator plane.
//!
//! Tagged [`DomainGroup::Operator`](crate::core::all::DomainGroup::Operator),
//! which only `DomainSet::saas()` enables, so a single-user core never serves
//! them.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::core::all::{ControllerFuture, RegisteredController};
use crate::core::Outcome;
use crate::core::{ControllerSchema, FieldSchema, TypeSchema};

#[derive(Debug, Deserialize)]
struct UserParams {
    user_id: String,
}

#[derive(Debug, Deserialize)]
struct AgentParams {
    agent_id: String,
}

#[derive(Deserialize)]
struct CredentialParams {
    agent_id: String,
    kind: super::credentials::UserCredentialKind,
    token: String,
    #[serde(default)]
    expires_at: Option<String>,
}

const FUNCTIONS: [&str; 6] = [
    "provision",
    "deprovision",
    "list",
    "status",
    "set_credential",
    "clear_credential",
];

pub fn all_user_agents_controller_schemas() -> Vec<ControllerSchema> {
    FUNCTIONS.iter().map(|f| user_agents_schemas(f)).collect()
}

pub fn all_user_agents_registered_controllers() -> Vec<RegisteredController> {
    vec![
        RegisteredController {
            schema: user_agents_schemas("provision"),
            handler: handle_provision,
        },
        RegisteredController {
            schema: user_agents_schemas("deprovision"),
            handler: handle_deprovision,
        },
        RegisteredController {
            schema: user_agents_schemas("list"),
            handler: handle_list,
        },
        RegisteredController {
            schema: user_agents_schemas("status"),
            handler: handle_status,
        },
        RegisteredController {
            schema: user_agents_schemas("set_credential"),
            handler: handle_set_credential,
        },
        RegisteredController {
            schema: user_agents_schemas("clear_credential"),
            handler: handle_clear_credential,
        },
    ]
}

pub fn user_agents_schemas(function: &str) -> ControllerSchema {
    match function {
        "provision" => ControllerSchema {
            namespace: "user_agents",
            function: "provision",
            description: "Create the agent that serves a gateway user, if it does not exist.",
            inputs: vec![string_field("user_id", "The gateway's id for the user.")],
            outputs: vec![
                string_field("agent_id", "The user's agent id, derived from the user id."),
                bool_field("created", "False when the agent already existed."),
            ],
        },
        "deprovision" => ControllerSchema {
            namespace: "user_agents",
            function: "deprovision",
            description: "Close a user agent and archive its state. Nothing is deleted.",
            inputs: vec![string_field("agent_id", "The agent to deprovision.")],
            outputs: vec![
                string_field("agent_id", "The agent."),
                bool_field("removed", "False when there was no such agent."),
            ],
        },
        "list" => ControllerSchema {
            namespace: "user_agents",
            function: "list",
            description: "List every provisioned user agent.",
            inputs: vec![],
            outputs: vec![FieldSchema {
                name: "agents",
                ty: TypeSchema::Array(Box::new(TypeSchema::Json)),
                comment: "agent_id, created_at and whether it is open.",
                required: true,
            }],
        },
        "status" => ControllerSchema {
            namespace: "user_agents",
            function: "status",
            description: "Report one provisioned user agent.",
            inputs: vec![string_field("agent_id", "The agent to report.")],
            outputs: vec![
                string_field("agent_id", "The agent."),
                FieldSchema {
                    name: "created_at",
                    ty: TypeSchema::U64,
                    comment: "Unix seconds.",
                    required: true,
                },
                bool_field("open", "Whether it is loaded right now."),
                bool_field("has_credential", "Whether a backend credential is installed."),
            ],
        },
        "set_credential" => ControllerSchema {
            namespace: "user_agents",
            function: "set_credential",
            description: "Install the TinyHumans credential the gateway holds for a user agent. \
                          The core stores it beside the agent's state and never validates or echoes it.",
            inputs: vec![
                string_field("agent_id", "The agent the credential belongs to."),
                FieldSchema {
                    name: "kind",
                    ty: TypeSchema::Enum {
                        variants: vec!["session", "api_key"],
                    },
                    comment: "A session JWT or an API key.",
                    required: true,
                },
                string_field("token", "The credential."),
                FieldSchema {
                    name: "expires_at",
                    ty: TypeSchema::Option(Box::new(TypeSchema::String)),
                    comment: "RFC 3339 expiry of a session, so an expired one is refused locally.",
                    required: false,
                },
            ],
            outputs: vec![
                string_field("agent_id", "The agent."),
                bool_field("has_credential", "Always true on success."),
            ],
        },
        "clear_credential" => ControllerSchema {
            namespace: "user_agents",
            function: "clear_credential",
            description: "Remove every credential a user agent holds.",
            inputs: vec![string_field("agent_id", "The agent.")],
            outputs: vec![
                string_field("agent_id", "The agent."),
                bool_field("has_credential", "Always false on success."),
            ],
        },
        _ => ControllerSchema {
            namespace: "user_agents",
            function: "unknown",
            description: "Unknown user_agents controller.",
            inputs: vec![],
            outputs: vec![string_field("error", "Lookup error details.")],
        },
    }
}

fn handle_provision(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let payload = deserialize_params::<UserParams>(params)?;
        to_json(super::ops::provision(&payload.user_id)?)
    })
}

fn handle_deprovision(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let payload = deserialize_params::<AgentParams>(params)?;
        to_json(super::ops::deprovision(&payload.agent_id)?)
    })
}

fn handle_list(_params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move { to_json(super::ops::list()?) })
}

fn handle_status(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let payload = deserialize_params::<AgentParams>(params)?;
        to_json(super::ops::status(&payload.agent_id)?)
    })
}

fn handle_set_credential(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let payload = deserialize_params::<CredentialParams>(params)?;
        to_json(super::ops::set_credential(
            &payload.agent_id,
            payload.kind,
            &payload.token,
            payload.expires_at.as_deref(),
        )?)
    })
}

fn handle_clear_credential(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let payload = deserialize_params::<AgentParams>(params)?;
        to_json(super::ops::clear_credential(&payload.agent_id)?)
    })
}

fn deserialize_params<T: DeserializeOwned>(params: Map<String, Value>) -> Result<T, String> {
    serde_json::from_value(Value::Object(params)).map_err(|e| format!("invalid params: {e}"))
}

fn to_json<T: serde::Serialize>(outcome: Outcome<T>) -> Result<Value, String> {
    outcome.into_cli_compatible_json()
}

fn string_field(name: &'static str, comment: &'static str) -> FieldSchema {
    FieldSchema {
        name,
        ty: TypeSchema::String,
        comment,
        required: true,
    }
}

fn bool_field(name: &'static str, comment: &'static str) -> FieldSchema {
    FieldSchema {
        name,
        ty: TypeSchema::Bool,
        comment,
        required: true,
    }
}

#[cfg(test)]
#[path = "schemas_tests.rs"]
mod tests;
