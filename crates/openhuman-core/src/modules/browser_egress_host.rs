//! Authenticated TinyComputer callbacks for TinySecurity browser egress grants.

use async_trait::async_trait;
use serde_json::Value;
use tinybus::{BusName, CallContext, Connection, Interface, InterfaceName, MemberName, ObjectPath};
use tinycomputer_bus::browser_egress::{
    self as contract, BrowserEgressConsumeRequest, BrowserEgressDecision, BrowserEgressGrant,
    BrowserEgressTarget, BrowserEgressValidateRequest,
};

#[derive(Clone)]
struct BrowserEgressHost;

#[tinybus::interface(name = "ai.tinyhumans.tinycomputer.BrowserEgressHost")]
impl BrowserEgressHost {
    async fn authorize(&self, target: BrowserEgressTarget) -> tinybus::Result<BrowserEgressGrant> {
        let config = crate::core::runtime::DispatchCtx::ambient()
            .load_config()
            .await
            .map_err(|_| refused())?;
        let client = super::security::client(&config)
            .await
            .map_err(|_| refused())?;
        let response = client
            .authorize_browser_dial(tinysecurity_bus::BrowserDialRequest {
                policy_id: tinysecurity_bus::EgressPolicyId(target.policy_id),
                expected_generation: target.expected_generation,
                session_id: target.session_id,
                scheme: target.scheme,
                hostname: target.hostname,
                port: target.port,
            })
            .await
            .map_err(|_| refused())?;
        let authorization = response.authorization.ok_or_else(refused)?;
        if response.generation != authorization.generation {
            return Err(refused());
        }
        let addresses = authorization
            .addresses
            .iter()
            .map(ToString::to_string)
            .collect();
        let authorization = serde_json::to_string(&authorization).map_err(|_| refused())?;
        Ok(BrowserEgressGrant {
            generation: response.generation,
            authorization,
            addresses,
        })
    }

    async fn consume(
        &self,
        request: BrowserEgressConsumeRequest,
    ) -> tinybus::Result<BrowserEgressDecision> {
        let client = security_client().await?;
        let authorization = decode_browser_authorization(&request.authorization)?;
        let address = request.address.parse().map_err(|_| refused())?;
        let allowed = client
            .consume_browser_dial(tinysecurity_bus::ConsumeBrowserDialRequest {
                request: browser_request(request.target),
                address,
                authorization,
            })
            .await
            .unwrap_or(false);
        Ok(BrowserEgressDecision { allowed })
    }

    async fn validate(
        &self,
        request: BrowserEgressValidateRequest,
    ) -> tinybus::Result<BrowserEgressDecision> {
        let client = security_client().await?;
        let authorization = decode_browser_authorization(&request.authorization)?;
        let allowed = client
            .validate_browser_dial(tinysecurity_bus::ValidateBrowserDialRequest { authorization })
            .await
            .unwrap_or(false);
        Ok(BrowserEgressDecision { allowed })
    }
}

async fn security_client() -> tinybus::Result<super::security::Client> {
    let config = crate::core::runtime::DispatchCtx::ambient()
        .load_config()
        .await
        .map_err(|_| refused())?;
    super::security::client(&config)
        .await
        .map_err(|_| refused())
}

fn browser_request(target: BrowserEgressTarget) -> tinysecurity_bus::BrowserDialRequest {
    tinysecurity_bus::BrowserDialRequest {
        policy_id: tinysecurity_bus::EgressPolicyId(target.policy_id),
        expected_generation: target.expected_generation,
        session_id: target.session_id,
        scheme: target.scheme,
        hostname: target.hostname,
        port: target.port,
    }
}

fn decode_browser_authorization(
    value: &str,
) -> tinybus::Result<tinysecurity_bus::BrowserDialAuthorization> {
    serde_json::from_str(value).map_err(|_| refused())
}

fn refused() -> tinybus::Error {
    tinybus::Error::failed("TinySecurity browser egress authorization unavailable")
}

struct ComputerOnly<T> {
    inner: T,
    connection: Connection,
}

#[async_trait]
impl<T: Interface> Interface for ComputerOnly<T> {
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
        Err(refused())
    }
    async fn call_with_context(
        &self,
        member: &MemberName,
        args: Value,
        context: &CallContext,
    ) -> tinybus::Result<Value> {
        let owner: Option<BusName> = self
            .connection
            .name_owner(tinycomputer_bus::names::INTERFACE)
            .await?;
        if owner.is_none() || context.authenticated_sender() != owner.as_ref() {
            return Err(refused());
        }
        self.inner.call(member, args).await
    }
}

pub(super) async fn install(connection: &Connection) -> tinybus::Result<()> {
    connection
        .serve_at(
            ObjectPath::new(contract::OBJECT_PATH)?,
            ComputerOnly {
                inner: BrowserEgressHost,
                connection: connection.clone(),
            },
        )
        .await?;
    connection.request_name(contract::INTERFACE).await
}
