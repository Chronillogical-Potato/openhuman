use super::*;

#[test]
fn global_returns_same_arc_across_calls() {
    let config = Config::default();
    let a = global(&config);
    let b = global(&config);
    assert!(Arc::ptr_eq(&a, &b), "global() must return a shared Arc");
}
