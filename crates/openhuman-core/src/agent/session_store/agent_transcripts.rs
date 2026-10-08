//! Transcript files of one embedded agent.
//!
//! An agent derived on a shared workspace keeps its transcripts under
//! `<workspace>/agents/<id>/session_raw/`. Conversations it wrote before that
//! layout, in `<workspace>/session_raw/`, stay readable: a lookup that misses
//! the agent's own directory falls back to the shared one, and the first time
//! such a session is opened for writing its file is copied into the agent's
//! directory. The shared file is never modified.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tinyagents_session::transcript::{
    find_root_transcript_for_thread_scoped, resolve_keyed_transcript_path, session_stem,
    FileTranscriptLocator, SessionAdoption, SessionRef, TranscriptHistory, TranscriptLocator,
    TranscriptMessage, TranscriptMeta, TranscriptPartial, TranscriptRead,
};

/// The workspace root an agent's transcripts are written under.
#[must_use]
pub fn agent_transcript_root(workspace_dir: &Path, agent_id: &str) -> PathBuf {
    workspace_dir.join("agents").join(agent_id)
}

/// The agent's own transcript files, with a read-only fallback to the shared
/// workspace's.
pub struct AgentTranscriptFiles {
    own: FileTranscriptLocator,
    own_root: PathBuf,
    legacy: FileTranscriptLocator,
    legacy_root: PathBuf,
}

impl AgentTranscriptFiles {
    /// `agent_id`'s transcripts on `workspace_dir`.
    #[must_use]
    pub fn new(workspace_dir: &Path, agent_id: &str) -> Self {
        let own_root = agent_transcript_root(workspace_dir, agent_id);
        Self {
            own: FileTranscriptLocator::new(own_root.clone()),
            own_root,
            legacy: FileTranscriptLocator::new(workspace_dir.to_path_buf()),
            legacy_root: workspace_dir.to_path_buf(),
        }
    }

    /// Copies `stem` from the shared directory into the agent's when only
    /// the shared one has it.
    fn adopt_stem(&self, stem: &str) {
        let Ok(own) = resolve_keyed_transcript_path(&self.own_root, stem) else {
            return;
        };
        if own.exists() {
            return;
        }
        let legacy = self
            .legacy_root
            .join("session_raw")
            .join(own.file_name().unwrap_or_default());
        if legacy.is_file() {
            self.copy_in(&legacy, &own);
        }
    }

    /// Copies the shared root transcript of `thread_id` into the agent's
    /// directory when the agent has none of its own.
    fn adopt_thread_root(&self, thread_id: &str, agent_id: Option<&str>) {
        if find_root_transcript_for_thread_scoped(&self.own_root, thread_id, agent_id).is_some() {
            return;
        }
        let Some(legacy) =
            find_root_transcript_for_thread_scoped(&self.legacy_root, thread_id, agent_id)
        else {
            return;
        };
        let Some(name) = legacy.file_name() else {
            return;
        };
        let own_dir = self.own_root.join("session_raw");
        if std::fs::create_dir_all(&own_dir).is_ok() {
            self.copy_in(&legacy, &own_dir.join(name));
        }
    }

    fn copy_in(&self, legacy: &Path, own: &Path) {
        match std::fs::copy(legacy, own) {
            Ok(_) => log::debug!(
                "[session_store] adopted shared transcript into the agent's directory file={}",
                own.file_name().unwrap_or_default().to_string_lossy()
            ),
            Err(error) => log::warn!(
                "[session_store] could not adopt shared transcript file={} error={error}",
                own.file_name().unwrap_or_default().to_string_lossy()
            ),
        }
    }
}

impl TranscriptLocator for AgentTranscriptFiles {
    fn destination_key(&self) -> Option<String> {
        self.own.destination_key()
    }

    fn latest_for_agent(&self, agent_name: &str) -> Option<Arc<dyn TranscriptRead>> {
        self.own
            .latest_for_agent(agent_name)
            .or_else(|| self.legacy.latest_for_agent(agent_name))
    }

    fn root_for_thread(&self, thread_id: &str) -> Option<Arc<dyn TranscriptRead>> {
        self.own
            .root_for_thread(thread_id)
            .or_else(|| self.legacy.root_for_thread(thread_id))
    }

    fn root_for_thread_scoped(
        &self,
        thread_id: &str,
        agent_id: Option<&str>,
    ) -> Option<Arc<dyn TranscriptRead>> {
        self.own
            .root_for_thread_scoped(thread_id, agent_id)
            .or_else(|| self.legacy.root_for_thread_scoped(thread_id, agent_id))
    }

    fn open_stem(
        &self,
        stem: &str,
        seed: TranscriptMeta,
    ) -> anyhow::Result<Arc<dyn TranscriptHistory>> {
        self.adopt_stem(stem);
        self.own.open_stem(stem, seed)
    }

    fn session_exists(&self, session: &SessionRef) -> bool {
        self.adopt_stem(&session_stem(session));
        self.own.session_exists(session)
    }

    fn read_session_transcript(&self, session: &SessionRef) -> Option<Arc<dyn TranscriptRead>> {
        self.adopt_stem(&session_stem(session));
        self.own.read_session_transcript(session)
    }

    fn adopt_legacy(
        &self,
        session: &SessionRef,
        thread_id: &str,
        seed: &TranscriptMeta,
    ) -> anyhow::Result<Option<SessionAdoption>> {
        self.own.adopt_legacy(session, thread_id, seed)
    }

    fn append_interrupted_partial(
        &self,
        thread_id: &str,
        agent_id: Option<&str>,
        partial: &TranscriptPartial,
        request_id: Option<&str>,
    ) -> anyhow::Result<bool> {
        self.adopt_thread_root(thread_id, agent_id);
        self.own
            .append_interrupted_partial(thread_id, agent_id, partial, request_id)
    }

    fn begin_generation(
        &self,
        session: &SessionRef,
        seed: TranscriptMeta,
    ) -> anyhow::Result<(SessionRef, Arc<dyn TranscriptHistory>)> {
        self.adopt_stem(&session_stem(session));
        self.own.begin_generation(session, seed)
    }

    fn begin_generation_from_baseline(
        &self,
        session: &SessionRef,
        seed: TranscriptMeta,
        baseline: &[TranscriptMessage],
    ) -> anyhow::Result<(SessionRef, Arc<dyn TranscriptHistory>)> {
        self.adopt_stem(&session_stem(session));
        self.own
            .begin_generation_from_baseline(session, seed, baseline)
    }
}
