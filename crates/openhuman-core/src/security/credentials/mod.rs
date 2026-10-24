//! Credential management for app session and provider auth profiles.

pub mod api_key;
pub mod bus;
mod core;
pub mod credential_ref;
pub mod identity;
pub mod jwt;
pub mod openai_oauth;
pub mod ops;
pub mod profiles;
pub mod provider_credential_source;
pub mod responses;
mod schemas;
pub mod session_support;
pub mod tools;

pub use core::*;
pub use credential_ref::{CredentialRef, CredentialRefError, CredentialRefScheme, ResolvedSecret};
pub use jwt::user_id_from_profile_payload;
pub use ops as rpc;
pub use ops::*;
pub use profiles::{AuthProfile, AuthProfileKind, TokenSet};
pub use provider_credential_source::{
    ProviderCredentialSource, ProviderOAuthStart, ProviderOAuthTokens,
    clear_provider_credential_source, install_provider_credential_source,
    provider_credential_source,
};
pub use schemas::{
    all_controller_schemas as all_credentials_controller_schemas,
    all_registered_controllers as all_credentials_registered_controllers,
};
