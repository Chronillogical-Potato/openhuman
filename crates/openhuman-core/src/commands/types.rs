use serde::{Deserialize, Serialize};

/// Where a `commands.list` entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    /// A fixed slash command the core itself understands (`/new`, `/plan`, …).
    Builtin,
    /// A `SKILL.md`/legacy skill from `skills.list`.
    Skill,
    /// A saved `tinyflows` automation from `flows.list`.
    Workflow,
}

/// One entry in the command palette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntry {
    /// Stable identifier: the bare command name for a built-in (`"new"`,
    /// not `"/new"`), the skill/workflow id otherwise.
    pub id: String,
    /// What to show in the menu.
    pub label: String,
    /// One-line description, when known. Empty string when the source has
    /// none (e.g. a workflow with no description field) — never omitted,
    /// so the frontend need not special-case a missing key.
    pub description: String,
    pub kind: CommandKind,
    /// The literal text a built-in inserts into the composer (`"/new"`).
    /// `None` for a skill/workflow entry — the frontend dispatches those
    /// through `skills.run` / `flows.run`, not by inserting text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert: Option<String>,
}

/// Wire shape for `commands.list`'s result: `{"commands": [...]}`, matching
/// `skills.list`'s `{"skills": [...]}` / `flows.list`'s `{"flows": [...]}`
/// convention rather than a bare array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandsListResponse {
    pub commands: Vec<CommandEntry>,
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
