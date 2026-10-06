use super::*;
use serde_json::json;
use tinytools::Tool;

#[tokio::test]
async fn missing_hash_is_error() {
    let tool = retrieve_tool_output_tool();
    let res2 = tool.execute(json!({})).await.unwrap();
    assert!(res2.is_error);
}

#[tokio::test]
async fn an_unreachable_store_is_reported_as_a_tool_error() {
    let tool = retrieve_tool_output_tool();
    let res = tool
        .execute(json!({ "hash": "deadbeefcafe" }))
        .await
        .unwrap();
    assert!(res.is_error);
}
