use super::*;

#[test]
fn openrouter_routes_are_detected() {
    let mut config = Config::default();
    assert!(!routes_to_openrouter(&config));
    config.inference_url = Some("https://openrouter.ai/api/v1".into());
    assert!(routes_to_openrouter(&config));
    config.inference_url = Some("https://OpenRouter.AI/api/v1".into());
    assert!(routes_to_openrouter(&config));
    config.inference_url = Some("https://api.openai.com/v1".into());
    assert!(!routes_to_openrouter(&config));
    config.memory_provider = Some("openrouter:deepseek/deepseek-v4-flash".into());
    assert!(routes_to_openrouter(&config));
    config.memory_provider = Some("ollama:llama3.1:8b".into());
    assert!(!routes_to_openrouter(&config));
}
