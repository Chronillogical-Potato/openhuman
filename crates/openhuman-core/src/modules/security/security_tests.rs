use super::*;

#[tokio::test]
async fn client_fails_closed_when_module_loading_is_disabled() {
    let mut config = Config::default();
    config.modules.enabled = false;

    assert!(matches!(
        client(&config).await,
        Err(SecurityError::Unavailable(_))
    ));
}

#[tokio::test]
#[ignore = "downloads and loads the pinned native TinySecurity release"]
async fn native_fixture_loads_the_pinned_release_and_reads_policy_info() {
    let mut config = Config::default();
    config.modules.enabled = true;

    let client = client(&config)
        .await
        .expect("pinned TinySecurity module should load on supported native runners");
    let info = client
        .policy_info()
        .await
        .expect("the loaded module should answer its typed policy-info contract");

    assert!(super::policy::supports_contract(info.contract_version));

    reinitialize(&config)
        .await
        .expect("the ready module should accept the same trusted host on reinitialization");
    let reinitialized_info = client
        .policy_info()
        .await
        .expect("the reinitialized module should continue answering policy info");
    assert!(super::policy::supports_contract(
        reinitialized_info.contract_version
    ));
}

#[tokio::test]
#[ignore = "downloads and loads the pinned native TinySecurity release"]
async fn native_redactor_covers_the_shared_security_fixture() {
    let mut config = Config::default();
    config.modules.enabled = true;
    let client = client(&config)
        .await
        .expect("the pinned TinySecurity module should load");

    for corpus in [
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/security-redaction-corpus.json")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/security-redaction-extended-corpus.json")),
    ] {
        let corpus: serde_json::Value = serde_json::from_str(corpus).unwrap();
        for case in corpus["text"].as_array().unwrap() {
            let kind = case["kind"].as_str().unwrap_or(match case["case"].as_str().unwrap() {
                "anthropic_key" => "secret",
                "email" => "pii",
                _ => "path",
            });
            if !matches!(kind, "secret" | "pii") {
                continue;
            }
            let input = fixture_parts(case, "input_parts", "input");
            let needle = fixture_parts(case, "needle_parts", "needle");
            let output = client
                .redact(tinysecurity_bus::RedactMode::Text, input)
                .await
                .expect("TinySecurity Redact should answer")
                .text;
            assert!(!output.contains(&needle), "case={} output={output}", case["case"]);
        }
    }
}

#[tokio::test]
#[ignore = "downloads and loads the pinned native TinySecurity release"]
async fn native_redact_with_covers_encoded_exact_credentials_and_browser_result() {
    let mut config = Config::default();
    config.modules.enabled = true;
    let client = client(&config)
        .await
        .expect("the pinned TinySecurity module should load");
    let secret = "secret+with/slash".to_owned();
    let encoded = [
        secret.clone(),
        "secret%2Bwith%2Fslash".to_owned(),
        "c2VjcmV0K3dpdGgvc2xhc2g=".to_owned(),
        hex::encode(secret.as_bytes()),
    ];
    let rendered = format!("key={secret}; encoded=secret%2Bwith%2Fslash");
    let response = client
        .redact_with(
            tinysecurity_bus::RedactMode::CredentialsOnly,
            rendered,
            encoded,
        )
        .await
        .expect("TinySecurity RedactWith should answer");
    assert!(!response.text.contains(&secret));
    assert!(!response.text.contains("secret%2Bwith%2Fslash"));

    let confirmation = "123e4567-e89b-12d3-a456-426614174000";
    let browser = format!(
        r#"{{"status":"NeedsConfirmation","pending":{{"token":"{confirmation}"}},"page":"api_key={secret}"}}"#
    );
    let response = client
        .redact(
            tinysecurity_bus::RedactMode::BrowserResult,
            browser,
        )
        .await
        .expect("TinySecurity BrowserResult should answer");
    assert!(response.text.contains(confirmation));
    assert!(!response.text.contains(&secret));
}

fn fixture_parts(case: &serde_json::Value, parts: &str, fallback: &str) -> String {
    case[parts]
        .as_array()
        .map(|values| values.iter().map(|value| value.as_str().unwrap()).collect())
        .unwrap_or_else(|| case[fallback].as_str().unwrap().to_owned())
}
