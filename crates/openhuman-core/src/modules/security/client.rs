//! Typed, attested calls to the TinySecurity policy interface.

use tinysecurity_bus::{
    BudgetDecision2, BudgetRequest2, CheckRequest, CheckResponse, CommandDecision, CommandPolicy,
    CommandPolicyId, CommandRequest, CredentialRootsRequest2, CredentialRootsResponse2, Decision,
    HostActionClassRequest2, HostActionClassResponse2, PathBatchRequest2, PathBatchResponse2,
    PathPolicy, PathPolicyId, PathValidationRequest, PathValidationResult, PolicyInfo,
    PrivacyCheckRequest2, PrivacyCheckResponse2, ProtectedFloor2, ProtectedPathRequest2,
    ProtectedPathResponse2, RedactMode, RedactRequest, RedactResponse, RedactWithRequest,
    ScanPromptScoredRequest2, ScanPromptScoredResponse2, ScanResponse, ScanToolDefinitionRequest,
    Secret,
};

use super::MODULE_ID;
use crate::config::Config;

/// Sanitized failure from the security module boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityError {
    /// The module is absent, disabled, faulted, or failed to answer.
    Unavailable(String),
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(f, "security module unavailable: {reason}"),
        }
    }
}

impl std::error::Error for SecurityError {}

/// A client whose object path comes from the compiled registry record.
#[derive(Clone)]
pub struct Client {
    object: tinybus::module::ModuleObjectClient,
}

impl Client {
    pub(super) async fn connect(config: &Config) -> Result<Self, String> {
        let client = super::super::ops::client(config.clone());
        let object = client
            .object_client(MODULE_ID, tinysecurity_bus::names::OBJECT_PATH)
            .await
            .map_err(|_| "pinned security module proxy unavailable".to_owned())?;
        Ok(Self { object })
    }

    /// Read effective settings and advertised capabilities from the module.
    pub async fn policy_info(&self) -> Result<PolicyInfo, SecurityError> {
        self.object
            .call_confidential(tinysecurity_bus::names::methods::POLICY_INFO, ())
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Redact text through TinySecurity's shared policy engine.
    pub async fn redact(
        &self,
        mode: RedactMode,
        text: impl Into<String>,
    ) -> Result<RedactResponse, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::REDACT,
                (RedactRequest {
                    text: text.into(),
                    mode,
                },),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Redact text using exact credentials that apply to this call only.
    pub async fn redact_with(
        &self,
        mode: RedactMode,
        text: impl Into<String>,
        secrets: impl IntoIterator<Item = String>,
    ) -> Result<RedactResponse, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::REDACT_WITH,
                (RedactWithRequest {
                    text: text.into(),
                    mode,
                    secrets: secrets.into_iter().map(Secret).collect(),
                },),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Score prompt text through TinySecurity. The result is context, never authorization.
    pub async fn scan_prompt2(
        &self,
        text: impl Into<String>,
    ) -> Result<ScanPromptScoredResponse2, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::SCAN_PROMPT2,
                (ScanPromptScoredRequest2 { text: text.into() },),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Scan a complete tool definition encoded as a JSON object.
    pub async fn scan_tool_definition(
        &self,
        name: impl Into<String>,
        definition: &serde_json::Value,
    ) -> Result<ScanResponse, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::SCAN_TOOL_DEFINITION,
                (ScanToolDefinitionRequest {
                    name: name.into(),
                    definition_json: definition.to_string(),
                },),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Ask the module for a policy decision. A missing or malformed response
    /// remains an error for the caller to deny.
    pub async fn check(&self, request: &CheckRequest) -> Result<CheckResponse, SecurityError> {
        self.object
            .call_confidential(tinysecurity_bus::names::methods::CHECK, (request,))
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Register an immutable host-derived path scope.
    pub async fn register_path_policy(
        &self,
        policy: PathPolicy,
    ) -> Result<PathPolicyId, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::REGISTER_PATH_POLICY,
                (policy,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Check a batch of paths against a registered path scope.
    pub async fn check_paths2(
        &self,
        request: PathBatchRequest2,
    ) -> Result<PathBatchResponse2, SecurityError> {
        self.object
            .call_confidential(tinysecurity_bus::names::methods::CHECK_PATHS2, (request,))
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Resolve and authorize an existing path under a registered path scope.
    pub async fn validate_path(
        &self,
        request: PathValidationRequest,
    ) -> Result<PathValidationResult, SecurityError> {
        self.object
            .call_confidential(tinysecurity_bus::names::methods::VALIDATE_PATH, (request,))
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Resolve and authorize the existing parent of a destination path.
    pub async fn validate_parent(
        &self,
        request: PathValidationRequest,
    ) -> Result<PathValidationResult, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::VALIDATE_PARENT,
                (request,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Query the unconditional path floor without a registered scope.
    pub async fn protected_path2(
        &self,
        request: ProtectedPathRequest2,
    ) -> Result<ProtectedPathResponse2, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::PROTECTED_PATH2,
                (request,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Read TinySecurity's protected-path vocabulary.
    pub async fn protected_floor2(&self) -> Result<ProtectedFloor2, SecurityError> {
        self.object
            .call_confidential(tinysecurity_bus::names::methods::PROTECTED_FLOOR2, ())
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Resolve TinySecurity's credential-store roots for a trusted home path.
    pub async fn credential_roots2(
        &self,
        request: CredentialRootsRequest2,
    ) -> Result<CredentialRootsResponse2, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::CREDENTIAL_ROOTS2,
                (request,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Classify a host tool from its trusted permission and effect declarations.
    pub async fn classify_host_action2(
        &self,
        request: HostActionClassRequest2,
    ) -> Result<HostActionClassResponse2, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::CLASSIFY_HOST_ACTION2,
                (request,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Ask TinySecurity whether one descriptor-level egress transfer is allowed.
    pub async fn privacy_check2(
        &self,
        request: PrivacyCheckRequest2,
    ) -> Result<Decision, SecurityError> {
        let response: PrivacyCheckResponse2 = self
            .object
            .call_confidential(tinysecurity_bus::names::methods::PRIVACY_CHECK2, (request,))
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))?;
        Ok(response.decision)
    }

    /// Authorize one browser socket dial through TinySecurity.
    pub(crate) async fn authorize_browser_dial(
        &self,
        request: tinysecurity_bus::BrowserDialRequest,
    ) -> Result<tinysecurity_bus::BrowserDialResponse, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::AUTHORIZE_BROWSER_DIAL,
                (request,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    pub(crate) async fn register_egress_policy(
        &self,
        policy: tinysecurity_bus::EgressPolicy,
    ) -> Result<tinysecurity_bus::EgressPolicyId, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::REGISTER_EGRESS_POLICY,
                (policy,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Consume the exact one-use browser grant before the selected socket opens.
    pub(crate) async fn consume_browser_dial(
        &self,
        request: tinysecurity_bus::ConsumeBrowserDialRequest,
    ) -> Result<bool, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::CONSUME_BROWSER_DIAL,
                (request,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Validate a live browser tunnel lease.
    pub(crate) async fn validate_browser_dial(
        &self,
        request: tinysecurity_bus::ValidateBrowserDialRequest,
    ) -> Result<bool, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::VALIDATE_BROWSER_DIAL,
                (request,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Register an immutable host-derived command scope.
    pub async fn register_command_policy(
        &self,
        policy: CommandPolicy,
    ) -> Result<CommandPolicyId, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::REGISTER_COMMAND_POLICY,
                (policy,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Classify an actual command without spending its action budget.
    pub async fn classify_command(
        &self,
        request: CommandRequest,
    ) -> Result<CommandDecision, SecurityError> {
        self.object
            .call_confidential(
                tinysecurity_bus::names::methods::CLASSIFY_COMMAND,
                (request,),
            )
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Check an actual command and reserve budget when TinySecurity allows it.
    pub async fn check_command(
        &self,
        request: CommandRequest,
    ) -> Result<CommandDecision, SecurityError> {
        self.object
            .call_confidential(tinysecurity_bus::names::methods::CHECK_COMMAND, (request,))
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Spend one action from a registered command scope.
    pub async fn spend_action2(
        &self,
        request: BudgetRequest2,
    ) -> Result<BudgetDecision2, SecurityError> {
        self.object
            .call_confidential(tinysecurity_bus::names::methods::SPEND_ACTION2, (request,))
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }

    /// Read a command scope's budget without spending an action.
    pub async fn peek_budget2(
        &self,
        request: BudgetRequest2,
    ) -> Result<BudgetDecision2, SecurityError> {
        self.object
            .call_confidential(tinysecurity_bus::names::methods::PEEK_BUDGET2, (request,))
            .await
            .map_err(|error| SecurityError::Unavailable(error.to_string()))
    }
}
