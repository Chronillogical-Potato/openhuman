use super::credentials::scrub_credentials;
use super::parse_tests::tools_to_openai_format;
use crate::tools;
use std::sync::Arc;
use tinytools_agent::{
    extract_json_values, parse_arguments_value, parse_glm_style_tool_calls, parse_tool_call_value,
    parse_tool_calls, parse_tool_calls_from_json_value,
};

fn build_tool_instructions(tools: &[Box<dyn tinytools::Tool>]) -> String {
    let specs = tools.iter().map(|tool| tool.spec()).collect::<Vec<_>>();
    tinytools_agent::dialect::XmlDialect::instructions(&specs)
}

#[path = "harness_tool_call_parsing_edge_case_tests.rs"]
mod harness_tool_call_parsing_edge_case_tests;
#[path = "harness_tool_call_parsing_tests.rs"]
mod harness_tool_call_parsing_tests;
