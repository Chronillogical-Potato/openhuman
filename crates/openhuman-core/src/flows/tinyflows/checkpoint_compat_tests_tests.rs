use serde_json::json;
use tempfile::TempDir;
use tinyflows::graph::ids::NodeId;
use tinyflows::graph::Checkpointer;
use tinyflows_sqlite::checkpoint::SqliteCheckpointer;

/// The reason this file exists. The port is a retarget of the backend
/// `tinyagents` shipped, and an existing `<workspace>/flows/checkpoints.db`
/// written by that backend is read and written by this code after the
/// upgrade; a divergence would surface as an interrupted flow that cannot
/// resume — at the worst possible moment, on a user's machine, with the run
/// already half-done.
///
/// The two schemas were byte-identical at the port. `tinyagents-graph` has
/// since grown its own copy (a `format_version` / `created_at` column pair and
/// a `thread_leases` table for its execution leases), so byte-equality is no
/// longer the contract. What still must hold is that every table and index
/// the ported schema creates is also created, with the same name, by the
/// backend it replaced — the ported reader only ever touches those.
#[test]
fn ported_schema_objects_all_exist_in_the_backend_it_replaced() {
    let ported = SqliteCheckpointer::<serde_json::Value>::schema_sql();
    let backend = tinyagents_graph::SqliteCheckpointer::<serde_json::Value>::schema_sql();
    let objects = |ddl: &str| -> Vec<String> {
        ddl.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("CREATE "))
            .map(|line| {
                line.split_whitespace()
                    .skip_while(|word| *word != "EXISTS")
                    .nth(1)
                    .unwrap_or_default()
                    .to_string()
            })
            .collect()
    };
    let ported_objects = objects(&ported);
    let backend_objects = objects(&backend);
    assert!(
        !ported_objects.is_empty(),
        "no CREATE statements parsed from:\n{ported}"
    );
    for object in &ported_objects {
        assert!(
            backend_objects.contains(object),
            "the ported schema creates `{object}`, which the tinyagents backend that \
             wrote every checkpoints.db in the field does not — an existing database \
             would stop resuming"
        );
    }
}

/// A database written by the backend this replaced must be readable here, and
/// the schema check above is a statement about DDL rather than about
/// behaviour. This writes through the old type and reads back through the new
/// one, against one file.
#[tokio::test]
async fn reads_a_database_written_by_the_previous_backend() {
    let tmp = TempDir::new().unwrap();
    let db = tmp.path().join("checkpoints.db");

    // Fully qualified: the two `Checkpointer` traits share a method set, so
    // importing both here would make every call in this file ambiguous.
    use tinyagents_graph::Checkpointer as LegacyCheckpointer;

    let old = tinyagents_graph::SqliteCheckpointer::<serde_json::Value>::open(&db).unwrap();
    // Built through the constructor rather than a struct literal: the
    // upstream record keeps growing bookkeeping fields (channel versions,
    // barrier arrivals, ...) and this test only cares about the columns the
    // ported reader must still decode.
    let mut written = tinyagents_graph::Checkpoint::new(json!({ "counter": 7 }), Vec::new())
        .with_thread_id("flow:f1:run-a")
        .with_checkpoint_id("cp-1");
    written.run_id = Some("run-1".to_string());
    written.next_nodes = vec![tinyagents_harness::ids::NodeId::new("next")];
    written.completed_tasks = vec![tinyagents_harness::ids::NodeId::new("done")];
    written.metadata = json!({ "source": "loop", "step": 3 });
    LegacyCheckpointer::put(&old, written).await.unwrap();
    drop(old);

    let new = SqliteCheckpointer::<serde_json::Value>::open(&db).unwrap();
    let read = new
        .get("flow:f1:run-a", None)
        .await
        .unwrap()
        .expect("the pre-upgrade checkpoint must still load");
    assert_eq!(read.checkpoint_id, "cp-1");
    assert_eq!(read.state, json!({ "counter": 7 }));
    assert_eq!(read.next_nodes, vec![NodeId::new("next")]);
    assert_eq!(read.metadata["step"], 3);
}
