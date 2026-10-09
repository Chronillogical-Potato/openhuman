use super::*;

impl CoreContext {
    /// This context, acting for `agent`: the same configuration, workspace
    /// binding, domains and transport, with the session agent replaced.
    ///
    /// For background work that visits an agent's records when no live
    /// context of that agent is at hand (`crate::storage::agents`): the agent
    /// was registered by an earlier process, or its host dropped it. A live
    /// agent context is always preferred, since it carries the agent's own
    /// configuration.
    pub fn for_agent(self: &Arc<Self>, agent: &str) -> Arc<CoreContext> {
        let binding = self
            .workspace_binding
            .read()
            .map(|handle| Arc::clone(&*handle))
            .unwrap_or_else(|poisoned| Arc::clone(&*poisoned.into_inner()));
        Arc::new(CoreContext {
            host_kind: self.host_kind,
            workspace_binding: RwLock::new(binding),
            domains: self.domains,
            tool_groups: self.tool_groups.clone(),
            embedder_config: self.embedder_config.clone(),
            user_skill_roots: self.user_skill_roots,
            backend_transport: self.backend_transport.clone(),
            turn_origin: self.turn_origin.clone(),
            session_agent: Some(agent.to_string()),
        })
    }
}
