use async_trait::async_trait;
use serde_json::{json, Value};
use tinytools::{PermissionLevel, Tool, ToolCategory, ToolContent, ToolResult};

use super::MediaArtifactTool;
use crate::agent::artifacts::ArtifactKind;

/// A minimal stand-in for `GenerateImageTool` / `GenerateVideoTool`: writes
/// a fixed number of files under `<action_dir>/generated-media/` and reports
/// them in the same `artifacts` array shape the real vendor tools use.
struct StubMediaTool {
    file_count: usize,
    fail: bool,
}

#[async_trait]
impl Tool for StubMediaTool {
    fn name(&self) -> &str {
        "stub_generate"
    }

    fn description(&self) -> &str {
        "stub"
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object" })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        if self.fail {
            return Ok(ToolResult::error("billed and failed"));
        }
        let dir = args
            .get("__dir")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
            .expect("stub requires __dir");
        std::fs::create_dir_all(&dir).unwrap();
        let mut artifacts = Vec::new();
        for i in 0..self.file_count {
            let path = dir.join(format!("stub-{i}.png"));
            std::fs::write(&path, format!("bytes-{i}")).unwrap();
            artifacts.push(json!({
                "type": "image",
                "path": path.display().to_string(),
                "media_type": "image/png",
                "bytes": 8,
            }));
        }
        Ok(ToolResult::success_with_markdown(
            json!({ "model": "stub/model", "artifacts": artifacts }),
            "generated stub media",
        ))
    }
}

fn json_payload(result: &ToolResult) -> Value {
    result
        .content
        .iter()
        .find_map(|block| match block {
            ToolContent::Json { data } => Some(data.clone()),
            _ => None,
        })
        .expect("json content block")
}

#[tokio::test]
async fn files_a_single_generated_artifact() {
    let root = tempfile::tempdir().unwrap();
    let staging = root.path().join("staging");
    let workspace = root.path().join("workspace");

    let wrapped = MediaArtifactTool::new(
        StubMediaTool {
            file_count: 1,
            fail: false,
        },
        ArtifactKind::Image,
        workspace.clone(),
    );

    let result = wrapped
        .execute(json!({ "prompt": "a cat", "__dir": staging.display().to_string() }))
        .await
        .unwrap();
    assert!(!result.is_error, "{result:?}");

    let payload = json_payload(&result);
    let artifacts = payload["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 1);
    let artifact_id = artifacts[0]["artifact_id"].as_str().unwrap();
    assert!(artifacts[0].get("artifact_error").is_none());

    // Original staged file is gone (moved).
    assert!(!staging.join("stub-0.png").exists());
    // Artifact metadata + file both landed under the artifacts root.
    let artifact_dir = workspace.join("artifacts").join(artifact_id);
    assert!(artifact_dir.join("meta.json").exists());
    let meta_raw = std::fs::read_to_string(artifact_dir.join("meta.json")).unwrap();
    assert!(meta_raw.contains("\"image\""));
}

#[tokio::test]
async fn files_every_artifact_when_n_greater_than_one() {
    let root = tempfile::tempdir().unwrap();
    let staging = root.path().join("staging");
    let workspace = root.path().join("workspace");

    let wrapped = MediaArtifactTool::new(
        StubMediaTool {
            file_count: 3,
            fail: false,
        },
        ArtifactKind::Image,
        workspace.clone(),
    );

    let result = wrapped
        .execute(json!({ "prompt": "three cats", "__dir": staging.display().to_string() }))
        .await
        .unwrap();
    let payload = json_payload(&result);
    let artifacts = payload["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 3);
    let mut ids = std::collections::HashSet::new();
    for entry in artifacts {
        let id = entry["artifact_id"].as_str().expect("artifact_id set");
        assert!(ids.insert(id.to_string()), "artifact ids must be unique");
        assert!(workspace.join("artifacts").join(id).join("meta.json").exists());
    }
}

#[tokio::test]
async fn leaves_an_errored_tool_result_untouched() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");

    let wrapped = MediaArtifactTool::new(
        StubMediaTool {
            file_count: 1,
            fail: true,
        },
        ArtifactKind::Image,
        workspace.clone(),
    );

    let result = wrapped.execute(json!({ "prompt": "x" })).await.unwrap();
    assert!(result.is_error);
    // No artifacts root should even be created.
    assert!(!workspace.join("artifacts").exists());
}

#[tokio::test]
async fn host_metadata_forwards_to_the_inner_tool() {
    let workspace_dir = tempfile::tempdir().unwrap();
    let workspace = workspace_dir.path().to_path_buf();
    let wrapped = MediaArtifactTool::new(
        StubMediaTool {
            file_count: 0,
            fail: false,
        },
        ArtifactKind::Video,
        workspace,
    );
    assert_eq!(wrapped.name(), "stub_generate");
    assert_eq!(wrapped.permission_level(), PermissionLevel::ReadOnly);
    assert_eq!(wrapped.category(), ToolCategory::System);
}
