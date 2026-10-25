//! Fail-closed TinySecurity `Callbacks2` server skeleton.
//!
//! The object is served before module load, but callback authority stays
//! unavailable until its owning caller migration lands. The wrapper admits
//! requests only from the loaded TinySecurity bus owner.

use async_trait::async_trait;
use serde_json::Value;
use tinybus::{BusName, CallContext, Connection, Interface, InterfaceName, MemberName, ObjectPath};
use tinysecurity_bus::{callback_names2, names};

#[derive(Clone)]
struct ApprovalStore;
#[tinybus::interface(name = "ai.tinyhumans.tinysecurity.ApprovalStore2")]
impl ApprovalStore {
    async fn load(&self, _request: tinysecurity_bus::ApprovalLoadRequest2) -> tinybus::Result<tinysecurity_bus::ApprovalLoadResponse2> {
        Err(unavailable())
    }
    async fn cas(&self, _request: tinysecurity_bus::ApprovalCasRequest2) -> tinybus::Result<tinysecurity_bus::ApprovalCasResponse2> {
        Err(unavailable())
    }
}

#[derive(Clone)]
struct AuditSink;
#[tinybus::interface(name = "ai.tinyhumans.tinysecurity.AuditSink2")]
impl AuditSink {
    async fn append(&self, _request: tinysecurity_bus::AuditSinkRequest2) -> tinybus::Result<tinysecurity_bus::AuditSinkResponse2> {
        Err(unavailable())
    }
}

#[derive(Clone)]
struct ApprovalPrompt;
#[tinybus::interface(name = "ai.tinyhumans.tinysecurity.ApprovalPrompt2")]
impl ApprovalPrompt {
    async fn prompt(&self, _request: tinysecurity_bus::ApprovalDeliveryRequest2) -> tinybus::Result<tinysecurity_bus::ApprovalDeliveryResponse2> {
        Err(unavailable())
    }
    async fn verify_human(&self, _request: tinysecurity_bus::VerifyHumanRequest2) -> tinybus::Result<tinysecurity_bus::HumanDecisionReceipt2> {
        Err(unavailable())
    }
}

#[derive(Clone)]
struct KeyringConsent;
#[tinybus::interface(name = "ai.tinyhumans.tinysecurity.KeyringConsent2")]
impl KeyringConsent {
    async fn consent(&self, _request: tinysecurity_bus::KeyringConsentRequest2) -> tinybus::Result<tinysecurity_bus::KeyringConsentResponse2> {
        Err(unavailable())
    }
}

/// Wrap one callback interface with module-owner authentication.
struct ModuleOnly<T> {
    inner: T,
    connection: Connection,
}

#[async_trait]
impl<T: Interface> Interface for ModuleOnly<T> {
    fn name(&self) -> InterfaceName { self.inner.name() }
    fn members(&self) -> Vec<MemberName> { self.inner.members() }
    fn requires_confidential(&self, member: &MemberName) -> bool { self.inner.requires_confidential(member) }
    async fn call(&self, _member: &MemberName, _args: Value) -> tinybus::Result<Value> { Err(unavailable()) }
    async fn call_with_context(&self, member: &MemberName, args: Value, context: &CallContext) -> tinybus::Result<Value> {
        let owner: Option<BusName> = self.connection.name_owner(names::INTERFACE).await?;
        if owner.is_none() || context.authenticated_sender() != owner.as_ref() { return Err(unavailable()); }
        self.inner.call(member, args).await
    }
}

/// Serve callbacks before loading TinySecurity; all not-yet-migrated operations deny.
pub(in crate::modules) async fn install(connection: &Connection) -> tinybus::Result<()> {
    let path = ObjectPath::new(callback_names2::OBJECT_PATH)?;
    connection.serve_at(path.clone(), ModuleOnly { inner: ApprovalStore, connection: connection.clone() }).await?;
    connection.serve_at(path.clone(), ModuleOnly { inner: AuditSink, connection: connection.clone() }).await?;
    connection.serve_at(path, ModuleOnly { inner: ApprovalPrompt, connection: connection.clone() }).await?;
    connection.serve_at(ObjectPath::new(callback_names2::OBJECT_PATH)?, ModuleOnly { inner: KeyringConsent, connection: connection.clone() }).await?;
    Ok(())
}

/// Fail closed for callback operations not migrated to the bus yet.
fn unavailable() -> tinybus::Error {
    tinybus::Error::failed("security host callback unavailable")
}
