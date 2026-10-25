use super::*;

#[test]
fn unmigrated_callbacks_return_an_error_instead_of_authority() {
    assert!(
        unavailable()
            .to_string()
            .contains("security host callback unavailable")
    );
}
