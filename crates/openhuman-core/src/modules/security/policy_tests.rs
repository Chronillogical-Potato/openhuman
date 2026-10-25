use super::*;

#[test]
fn policy_translation_copies_effective_host_settings() {
    let mut config = Config::default();
    config.autonomy.enabled = true;
    config.autonomy.allowed_commands = vec!["pwd".into()];
    config.autonomy.forbidden_paths = vec!["/private".into()];

    let translated = policy_config(&config);
    assert!(translated.enabled);
    assert_eq!(translated.action_dir, config.action_dir.display().to_string());
    assert_eq!(translated.workspace_dir, config.workspace_dir.display().to_string());
    assert_eq!(translated.allowed_commands, ["pwd"]);
    assert_eq!(translated.forbidden_paths, ["/private"]);
}

#[test]
fn first_load_and_reinit_share_one_policy_translator() {
    let config = Config::default();
    assert_eq!(module_config(&config).policy, policy_config(&config));
}
