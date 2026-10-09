use super::*;

#[test]
fn saas_sandbox_is_not_resolved_outside_saas() {
    let resolved = saas_sandbox_with(false, || panic!("must not resolve a policy"));
    assert!(resolved.is_none());
}

#[test]
fn saas_sandbox_refuses_instead_of_falling_back_to_the_host() {
    let resolved = saas_sandbox_with(true, || Err("no such sandbox".to_string()));
    let why = match resolved {
        Some(Err(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    };
    let (allowed, result) = saas_sandbox_refusal(&why);
    assert!(!allowed);
    assert!(result.is_error);
    assert!(result
        .output()
        .contains("Sandbox unavailable: no such sandbox"));
}

#[test]
fn runtime_path_is_withheld_only_in_saas() {
    assert!(!passes_runtime_path_with(true));
    assert!(passes_runtime_path_with(false));
}
