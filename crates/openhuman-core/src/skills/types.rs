//! Shared tool result types used by the tool and node runtime surfaces.
//!
//! The definitions live in [`tinytools`]; this module is the stable host import
//! path for the ~14 call sites that already name it. The MCP-result conversion
//! is `tinymcp::tools::tool_result`.

pub use tinytools::{ToolContent, ToolResult};
