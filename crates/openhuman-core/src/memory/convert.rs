//! The document converter memory files with: PDF, DOCX, PPTX and XLSX through
//! TinyMemory's `OfficeConverter` (with the `documents` feature, which the
//! shipped product enables), then its `NativeConverter` for text, markdown,
//! HTML and code. Without `documents` the chain is native only, and an office
//! file is refused as before; the parsers stay off the always-on path.
//!
//! Brain ingest (`memory_brain_ingest` with a `path`) and file-backed sources
//! (folder, file) both convert through [`converter`], so a PDF filed by hand
//! and a PDF in a synced folder read the same. Without the office converter
//! both refused office formats with "the native converter does not handle
//! pdf" (#6718).
//!
//! Office parsing is CPU-bound and synchronous, so it runs on Tokio's
//! blocking pool rather than on a runtime worker (`OfficeConverter`'s own
//! async `convert` would parse inline). Size is capped upstream by
//! `MAX_DOCUMENT_BYTES`.

use std::sync::LazyLock;

use tinymemory_integrations::documents::ConverterChain;
#[cfg(feature = "documents")]
use {
    async_trait::async_trait,
    tinymemory_integrations::documents::{
        ConvertedDocument, DocumentConverter, DocumentFormat, Error, OfficeConverter, RawDocument,
        Result,
    },
};

/// [`OfficeConverter`] on the blocking pool.
#[cfg(feature = "documents")]
struct BlockingOffice;

#[cfg(feature = "documents")]
#[async_trait]
impl DocumentConverter for BlockingOffice {
    fn name(&self) -> &str {
        OfficeConverter.name()
    }

    fn supports(&self, format: DocumentFormat) -> bool {
        OfficeConverter.supports(format)
    }

    async fn convert(&self, document: &RawDocument) -> Result<ConvertedDocument> {
        let document = document.clone();
        crate::core::runtime::spawn_blocking_scoped(move || {
            OfficeConverter.convert_blocking(&document)
        })
        .await
        .map_err(task_failed)?
    }
}

/// The error for a conversion task that never finished (it panicked or was
/// cancelled): a converter failure, never a crash of the caller.
#[cfg(feature = "documents")]
fn task_failed(error: impl std::fmt::Display) -> Error {
    Error::Converter {
        converter: "office".to_string(),
        message: format!("the conversion task did not finish: {error}"),
    }
}

static CHAIN: LazyLock<ConverterChain> = LazyLock::new(|| {
    let chain = ConverterChain::default();
    #[cfg(feature = "documents")]
    let chain = chain.prepend(Box::new(BlockingOffice));
    chain
});

/// The converter every memory write of a file goes through.
pub(crate) fn converter() -> &'static ConverterChain {
    &CHAIN
}

#[cfg(test)]
#[path = "convert_tests.rs"]
mod tests;
