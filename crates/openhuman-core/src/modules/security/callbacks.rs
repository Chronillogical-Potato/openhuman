//! TinySecurity `Callbacks2` server. Approval state and prompts are served by
//! the approval host adapter; the wrapper admits requests only from the loaded
//! TinySecurity bus owner.

use async_trait::async_trait;
use serde_json::Value;
use tinybus::{BusName, CallContext, Connection, Interface, InterfaceName, MemberName, ObjectPath};
use tinysecurity_bus::{callback_names2, names};

#[derive(Clone)]
struct ApprovalStore;
#[tinybus::interface(name = "ai.tinyhumans.tinysecurity.ApprovalStore2")]
impl ApprovalStore {
    async fn load(
        &self,
        request: tinysecurity_bus::ApprovalLoadRequest2,
    ) -> tinybus::Result<tinysecurity_bus::ApprovalLoadResponse2> {
        crate::security::approval::native::callbacks::load(request)
            .await
            .map_err(tinybus::Error::failed)
    }
    async fn cas(
        &self,
        request: tinysecurity_bus::ApprovalCasRequest2,
    ) -> tinybus::Result<tinysecurity_bus::ApprovalCasResponse2> {
        crate::security::approval::native::callbacks::cas(request)
            .await
            .map_err(tinybus::Error::failed)
    }
}

#[derive(Clone)]
struct AuditSink;
#[tinybus::interface(name = "ai.tinyhumans.tinysecurity.AuditSink2")]
impl AuditSink {
    async fn append(
        &self,
        request: tinysecurity_bus::AuditSinkRequest2,
    ) -> tinybus::Result<tinysecurity_bus::AuditSinkResponse2> {
        crate::security::approval::native::callbacks::append(request)
            .await
            .map_err(tinybus::Error::failed)
    }
}

#[derive(Clone)]
struct ApprovalPrompt;
#[tinybus::interface(name = "ai.tinyhumans.tinysecurity.ApprovalPrompt2")]
impl ApprovalPrompt {
    async fn prompt(
        &self,
        request: tinysecurity_bus::ApprovalDeliveryRequest2,
    ) -> tinybus::Result<tinysecurity_bus::ApprovalDeliveryResponse2> {
        crate::security::approval::native::callbacks::prompt(request)
            .await
            .map_err(tinybus::Error::failed)
    }
    async fn verify_human(
        &self,
        request: tinysecurity_bus::VerifyHumanRequest2,
    ) -> tinybus::Result<tinysecurity_bus::HumanDecisionReceipt2> {
        crate::security::approval::native::callbacks::verify_human(request)
            .await
            .map_err(tinybus::Error::failed)
    }
}

#[derive(Clone)]
struct KeyringConsent;
#[tinybus::interface(name = "ai.tinyhumans.tinysecurity.KeyringConsent2")]
impl KeyringConsent {
    async fn consent(
        &self,
        request: tinysecurity_bus::KeyringConsentRequest2,
    ) -> tinybus::Result<tinysecurity_bus::KeyringConsentResponse2> {
        use tinysecurity_bus::KeyringConsentResponse2;

        let approved =
            crate::security::keyring_consent::operation_approval::request_approval(request).await;
        Ok(KeyringConsentResponse2 {
            outcome: keyring_consent_outcome(approved),
        })
    }
}

fn keyring_consent_outcome(approved: Option<bool>) -> tinysecurity_bus::KeyringConsentOutcome2 {
    match approved {
        Some(true) => tinysecurity_bus::KeyringConsentOutcome2::Allowed,
        Some(false) => tinysecurity_bus::KeyringConsentOutcome2::ConsentDenied,
        None => tinysecurity_bus::KeyringConsentOutcome2::Unavailable,
    }
}

/// Wrap one callback interface with module-owner authentication.
struct ModuleOnly<T> {
    inner: T,
    connection: Connection,
}

#[async_trait]
impl<T: Interface> Interface for ModuleOnly<T> {
    fn name(&self) -> InterfaceName {
        self.inner.name()
    }
    fn members(&self) -> Vec<MemberName> {
        self.inner.members()
    }
    fn requires_confidential(&self, member: &MemberName) -> bool {
        self.inner.requires_confidential(member)
    }
    async fn call(&self, _member: &MemberName, _args: Value) -> tinybus::Result<Value> {
        Err(unavailable())
    }
    async fn call_with_context(
        &self,
        member: &MemberName,
        args: Value,
        context: &CallContext,
    ) -> tinybus::Result<Value> {
        let owner: Option<BusName> = self.connection.name_owner(names::INTERFACE).await?;
        if owner.is_none() || context.authenticated_sender() != owner.as_ref() {
            return Err(unavailable());
        }
        self.inner.call(member, args).await
    }
}

/// Serve callbacks before loading TinySecurity; authority is restricted to its
/// authenticated bus owner.
pub(in crate::modules) async fn install(connection: &Connection) -> tinybus::Result<()> {
    let path = ObjectPath::new(callback_names2::OBJECT_PATH)?;
    connection
        .serve_at(
            path.clone(),
            ModuleOnly {
                inner: ApprovalStore,
                connection: connection.clone(),
            },
        )
        .await?;
    connection
        .serve_at(
            path.clone(),
            ModuleOnly {
                inner: AuditSink,
                connection: connection.clone(),
            },
        )
        .await?;
    connection
        .serve_at(
            path,
            ModuleOnly {
                inner: ApprovalPrompt,
                connection: connection.clone(),
            },
        )
        .await?;
    connection
        .serve_at(
            ObjectPath::new(callback_names2::OBJECT_PATH)?,
            ModuleOnly {
                inner: KeyringConsent,
                connection: connection.clone(),
            },
        )
        .await?;
    Ok(())
}

/// Fail closed for callback operations not migrated to the bus yet.
fn unavailable() -> tinybus::Error {
    tinybus::Error::failed("security host callback unavailable")
}

#[cfg(test)]
#[path = "callbacks_tests.rs"]
mod tests;
