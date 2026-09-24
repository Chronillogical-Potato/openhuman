use super::*;

fn controller(name: &str) -> RegisteredController {
    all_registered_controllers()
        .into_iter()
        .find(|item| item.schema.function == name)
        .unwrap()
}

#[tokio::test]
async fn desktop_controllers_expose_expected_rpc_contracts_and_dispatch_pending() {
    let controllers = all_registered_controllers();
    assert_eq!(controllers.len(), 5);
    for item in &controllers {
        assert_eq!(item.schema.namespace, "desktop");
        assert!(item.rpc_method_name().starts_with("openhuman.desktop_"));
    }
    assert_eq!(controller("set_enabled").schema.inputs.len(), 1);
    assert_eq!(controller("confirm").schema.inputs.len(), 2);
    let value = (controller("pending").handler)(Map::new()).await.unwrap();
    assert!(value.is_array());
}

#[tokio::test]
async fn malformed_controller_inputs_fail_before_loading_config_or_acting() {
    for invalid in [
        Value::Null,
        Value::String("true".to_owned()),
        Value::Number(1.into()),
    ] {
        let mut params = Map::new();
        params.insert("enabled".to_owned(), invalid);
        assert_eq!(
            (controller("set_enabled").handler)(params)
                .await
                .unwrap_err(),
            "enabled must be a boolean"
        );
    }
    let mut params = Map::new();
    params.insert("confirmation_id".to_owned(), Value::String(String::new()));
    params.insert("approve".to_owned(), Value::Bool(true));
    assert_eq!(
        (controller("confirm").handler)(params).await.unwrap_err(),
        "confirmation_id is required"
    );
    let mut params = Map::new();
    params.insert("confirmation_id".to_owned(), Value::String("id".to_owned()));
    params.insert("approve".to_owned(), Value::String("yes".to_owned()));
    assert_eq!(
        (controller("confirm").handler)(params).await.unwrap_err(),
        "approve must be a boolean"
    );
}
