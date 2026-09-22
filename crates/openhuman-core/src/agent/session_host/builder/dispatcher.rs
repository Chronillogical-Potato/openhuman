//! Which tool-call dialect a session speaks to its provider, resolved from
//! the configured `agent.tool_dispatcher` choice and the provider's native
//! tool support.

use tinytools_agent::dialect::CodeStyle;

/// Which tool-call dialect a session speaks to its provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DispatcherKind {
    /// Provider-native structured function calling (JSON tool specs on the wire).
    Native,
    /// JSON-in-tag: `<tool_call>{"name":…,"arguments":{…}}</tool_call>` in text.
    Xml,
    /// Compact positional P-Format (`tool[a|b]`) — opt-in only.
    PFormat,
    /// Code-style calls against Python or TypeScript signatures — opt-in only.
    Code(CodeStyle),
}

/// Pick the tool-call dialect from the configured `agent.tool_dispatcher`
/// choice, the provider's native-tool support, and the agent id.
///
/// `"auto"` (and any unrecognized value) resolves to native when the provider
/// supports it, otherwise JSON-in-tag — **never** P-Format or a code dialect,
/// which are opt-in (`"pformat"`, `"python"`, `"typescript"`) because their
/// compact syntaxes mis-parse on some models.
///
/// `integrations_agent` is special-cased off native: provider-side grammar
/// decoders (e.g. Fireworks) compile every JSON tool schema into a grammar
/// indexed by a `uint16_t` (max 65 535 rules), and large Composio toolkits
/// (Notion, Salesforce, Gmail) blow past that ceiling, so a native request is
/// rejected with a 400 before any generation. Falling back to JSON-in-tag puts
/// the catalogue in the prompt as prose, so no grammar is compiled.
pub(super) fn resolve_dispatcher_kind(
    dispatcher_choice: &str,
    supports_native: bool,
    agent_id: &str,
) -> DispatcherKind {
    let base = match dispatcher_choice {
        "native" => DispatcherKind::Native,
        "xml" => DispatcherKind::Xml,
        "pformat" => DispatcherKind::PFormat,
        "python" => DispatcherKind::Code(CodeStyle::Python),
        "typescript" => DispatcherKind::Code(CodeStyle::TypeScript),
        _ if supports_native => DispatcherKind::Native,
        _ => DispatcherKind::Xml,
    };
    if agent_id == "integrations_agent" && base == DispatcherKind::Native {
        DispatcherKind::Xml
    } else {
        base
    }
}
