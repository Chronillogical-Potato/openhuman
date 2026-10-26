//! Exact host storage callback for TinySecurity's native master resolver.

use std::path::PathBuf;

use tinysecurity_bus::{
    callback_names2, MasterSlotKind2, SecretBackendLoadRequest2, SecretBackendLoadResponse2,
    SecretBackendLoadStatus2, SecretBackendSetRequest2, SecretBackendSetResponse2,
    SecretBackendSetStatus2, SecretPhase2,
};

/// One configured backend identifier and whether it names the legacy file source.
#[derive(Clone, Copy)]
struct Binding {
    backend: crate::security::secret_storage::store::BackendKind,
    legacy_file: bool,
}

/// Store adapter admitted only on TinySecurity's exact host callback binding.
pub(crate) struct SecretBackend {
    registration_id: String,
    user_id: String,
    target_backend_id: String,
    legacy_backend_id: String,
    legacy_path: PathBuf,
    target: Binding,
    legacy: Binding,
}

impl SecretBackend {
    pub(crate) fn new(
        registration_id: String,
        user_id: String,
        target_backend_id: String,
        legacy_backend_id: String,
        legacy_path: PathBuf,
        target_backend: crate::security::secret_storage::store::BackendKind,
    ) -> Self {
        Self {
            registration_id,
            user_id,
            target_backend_id,
            legacy_backend_id,
            legacy_path,
            target: Binding {
                backend: target_backend,
                legacy_file: false,
            },
            legacy: Binding {
                backend: target_backend,
                legacy_file: true,
            },
        }
    }

    /// Serve this backend only to the admitted TinySecurity module instance.
    pub(crate) async fn serve(
        self: std::sync::Arc<Self>,
        binding: &tinybus::module::HostCallbackBinding,
    ) -> tinybus::Result<()> {
        binding
            .serve_at(callback_names2::OBJECT_PATH.parse()?, self)
            .await
    }

    fn binding(&self, identity: &tinysecurity_bus::SecretBackendIdentity2) -> Option<Binding> {
        if identity.registration_id != self.registration_id {
            return None;
        }
        if identity.backend_id == self.target_backend_id && identity.slot == MasterSlotKind2::User {
            Some(self.target)
        } else if identity.backend_id == self.legacy_backend_id
            && identity.slot == MasterSlotKind2::File
        {
            Some(self.legacy)
        } else {
            None
        }
    }

    fn load(&self, request: SecretBackendLoadRequest2) -> SecretBackendLoadResponse2 {
        let identity = request.identity;
        let status = match self.binding(&identity) {
            None => SecretBackendLoadStatus2::Denied,
            Some(binding) if binding.legacy_file && identity.phase == SecretPhase2::LegacyRead => {
                match read_legacy_file(&self.legacy_path) {
                    Ok(Some(encoded)) => SecretBackendLoadStatus2::Present { encoded },
                    Ok(None) => SecretBackendLoadStatus2::Absent,
                    Err(()) => SecretBackendLoadStatus2::Unavailable,
                }
            }
            Some(binding)
                if !binding.legacy_file
                    && matches!(
                        identity.phase,
                        SecretPhase2::TargetRead | SecretPhase2::TargetReadback
                    ) =>
            {
                match load_backend(binding.backend, &self.user_id) {
                    Ok(Some(encoded)) => SecretBackendLoadStatus2::Present { encoded },
                    Ok(None) => SecretBackendLoadStatus2::Absent,
                    Err(()) => SecretBackendLoadStatus2::Unavailable,
                }
            }
            Some(_) => SecretBackendLoadStatus2::Denied,
        };
        SecretBackendLoadResponse2 { identity, status }
    }

    fn set(&self, request: SecretBackendSetRequest2) -> SecretBackendSetResponse2 {
        let identity = request.identity;
        let status = match self.binding(&identity) {
            Some(binding)
                if !binding.legacy_file
                    && identity.phase == SecretPhase2::TargetSet
                    && request.encoded.len() <= 4096 =>
            {
                match std::str::from_utf8(&request.encoded)
                    .ok()
                    .and_then(|encoded| save_backend(binding.backend, &self.user_id, encoded).ok())
                {
                    Some(()) => SecretBackendSetStatus2::Stored,
                    None => SecretBackendSetStatus2::Unavailable,
                }
            }
            _ => SecretBackendSetStatus2::Denied,
        };
        SecretBackendSetResponse2 { identity, status }
    }
}

#[cfg(test)]
#[path = "secret_backend_tests.rs"]
mod tests;

fn load_backend(
    backend: crate::security::secret_storage::store::BackendKind,
    user_id: &str,
) -> Result<Option<Vec<u8>>, ()> {
    if crate::security::secret_storage::store::effective_backend_kind() != backend {
        return Err(());
    }
    crate::security::secret_storage::ops::process_get(user_id, "secretstore.master_key")
        .map(|value| value.map(String::into_bytes))
        .map_err(|_| ())
}

fn save_backend(
    backend: crate::security::secret_storage::store::BackendKind,
    user_id: &str,
    encoded: &str,
) -> Result<(), ()> {
    if crate::security::secret_storage::store::effective_backend_kind() != backend {
        return Err(());
    }
    crate::security::secret_storage::ops::process_set(user_id, "secretstore.master_key", encoded)
        .map_err(|_| ())
}

fn read_legacy_file(path: &std::path::Path) -> Result<Option<Vec<u8>>, ()> {
    match std::fs::read(path) {
        Ok(contents) if contents.len() <= 4096 => Ok(Some(contents)),
        Ok(_) => Err(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}

#[tinybus::interface(name = "ai.tinyhumans.tinysecurity.SecretBackend2")]
impl SecretBackend {
    #[tinybus(confidential)]
    async fn load(
        &self,
        request: SecretBackendLoadRequest2,
    ) -> tinybus::Result<SecretBackendLoadResponse2> {
        Ok(SecretBackend::load(self, request))
    }

    #[tinybus(confidential)]
    async fn set(
        &self,
        request: SecretBackendSetRequest2,
    ) -> tinybus::Result<SecretBackendSetResponse2> {
        Ok(SecretBackend::set(self, request))
    }
}
