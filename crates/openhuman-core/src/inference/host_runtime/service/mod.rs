//! OpenHuman-owned speech bindings around TinyInference's local runtime.

mod speech;
pub(crate) mod transcription;

pub use speech::{transcribe, transcribe_with_prompt, tts};
pub use tinyinference_local::service::LocalAiService;
