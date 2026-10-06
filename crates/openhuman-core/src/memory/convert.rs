//! The document converter memory files with: PDF, DOCX, PPTX and XLSX through
//! TinyMemory's `OfficeConverter`, then its `NativeConverter` for text,
//! markdown, HTML and code.
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

use async_trait::async_trait;
use tinymemory_integrations::documents::{
    ConvertedDocument, ConverterChain, DocumentConverter, DocumentFormat, Error, OfficeConverter,
    RawDocument, Result,
};

/// [`OfficeConverter`] on the blocking pool.
struct BlockingOffice;

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
        tokio::task::spawn_blocking(move || OfficeConverter.convert_blocking(&document))
            .await
            .map_err(|error| Error::Converter {
                converter: "office".to_string(),
                message: format!("the conversion task did not finish: {error}"),
            })?
    }
}

static CHAIN: LazyLock<ConverterChain> =
    LazyLock::new(|| ConverterChain::default().prepend(Box::new(BlockingOffice)));

/// The converter every memory write of a file goes through.
pub(crate) fn converter() -> &'static ConverterChain {
    &CHAIN
}

#[cfg(test)]
#[path = "convert_tests.rs"]
mod tests;
