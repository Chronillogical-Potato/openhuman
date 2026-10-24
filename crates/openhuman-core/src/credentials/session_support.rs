//! Compatibility-free grouping of the session state and backend credential APIs.

pub use super::backend_credential::{
    ambient_config_has_api_key, backend_bearer_secret, credential_fingerprint,
    credential_fingerprint_matches, direct_backend_credential, has_backend_credential,
    publish_local_session_expiry, resolve_backend_credential, BackendCredential,
    LOCAL_SESSION_BACKEND_UNAVAILABLE, LOCAL_SESSION_MANAGED_INFERENCE_UNAVAILABLE,
};
pub use super::session_state::{
    build_session_state, check_session_backend, classify_session_token, current_session_is_local,
    get_session_token, is_local_session_token, load_app_session_profile,
    load_app_session_profile_read_only, local_session_user_id, normalize_session_backend,
    parse_fields_value, profile_name_or_default, require_live_session_token,
    session_state_from_profile, session_token_for_config, session_token_from_profile,
    summarize_auth_profile, user_id_from_jwt_claims, CredentialKind, SessionTokenCheck,
    LOCAL_SESSION_USER_ID, SESSION_EXPIRES_AT_META, SESSION_ISSUING_BACKEND_META,
};

#[cfg(test)]
#[path = "session_support_tests.rs"]
mod tests;
