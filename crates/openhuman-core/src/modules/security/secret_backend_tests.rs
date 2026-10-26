use super::*;

fn identity() -> tinysecurity_bus::SecretBackendIdentity2 {
    tinysecurity_bus::SecretBackendIdentity2 {
        backend_id: "target-backend".into(),
        registration_id: "registration-id".into(),
        generation: 1,
        operation_id: "operation-id".into(),
        phase: SecretPhase2::TargetSet,
        slot: MasterSlotKind2::User,
    }
}

fn backend() -> SecretBackend {
    SecretBackend::new(
        "registration-id".into(),
        "user-id".into(),
        "target-backend".into(),
        "legacy-backend".into(),
        PathBuf::from("/unused/legacy-key"),
        crate::security::secret_storage::store::effective_backend_kind(),
    )
}

#[test]
fn target_mutations_reject_a_different_registration_backend_or_slot() {
    let backend = backend();
    let mut wrong_registration = identity();
    wrong_registration.registration_id.push('x');
    let mut wrong_backend = identity();
    wrong_backend.backend_id.push('x');
    let mut wrong_slot = identity();
    wrong_slot.slot = MasterSlotKind2::File;
    for identity in [wrong_registration, wrong_backend, wrong_slot] {
        let response = backend.set(SecretBackendSetRequest2 {
            identity,
            encoded: b"deadbeef".to_vec(),
        });
        assert_eq!(response.status, SecretBackendSetStatus2::Denied);
    }
}

#[test]
fn legacy_source_is_never_a_mutation_target() {
    let mut identity = identity();
    identity.backend_id = "legacy-backend".into();
    identity.slot = MasterSlotKind2::File;
    let response = backend().set(SecretBackendSetRequest2 {
        identity,
        encoded: b"deadbeef".to_vec(),
    });
    assert_eq!(response.status, SecretBackendSetStatus2::Denied);
}
