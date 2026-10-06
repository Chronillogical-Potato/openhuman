use super::*;

use tinymemory_integrations::documents::{DocumentConverter, Error, RawDocument};
#[cfg(feature = "documents")]
use {
    crate::memory::brain::{ingest, BrainIngestParams},
    crate::memory::test_fixtures::{bind_reference, config_in, stored},
    tinymemory_api::{ItemKind, MetaFilter},
};

/// A one-page PDF whose only text is `text` (Helvetica, no compression).
fn pdf_saying(text: &str) -> Vec<u8> {
    let stream = format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>"
            .to_string(),
        format!(
            "<< /Length {} >>\nstream\n{stream}\nendstream",
            stream.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.push_str(&format!("{} 0 obj\n{object}\nendobj\n", i + 1));
    }
    let xref = out.len();
    out.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
    for offset in offsets {
        out.push_str(&format!("{offset:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    ));
    out.into_bytes()
}

#[cfg(feature = "documents")]
#[tokio::test]
async fn a_pdf_converts_to_its_text() {
    let raw = RawDocument::new(pdf_saying("The vendor code is PV-7023")).with_filename("brief.pdf");
    let converted = converter()
        .convert(&raw)
        .await
        .expect("the office converter reads pdf");
    assert!(
        converted.markdown.contains("PV-7023"),
        "{}",
        converted.markdown
    );
}

#[tokio::test]
async fn markdown_still_goes_through_the_native_converter() {
    let raw = RawDocument::new(b"# Note\n\nPlain markdown.".to_vec()).with_filename("note.md");
    let converted = converter()
        .convert(&raw)
        .await
        .expect("native handles markdown");
    assert!(converted.markdown.contains("Plain markdown."));
}

#[tokio::test]
async fn an_unknown_binary_is_still_refused_with_a_clear_error() {
    let raw = RawDocument::new(vec![0u8, 1, 2, 3, 255]).with_filename("blob.bin");
    let error = converter().convert(&raw).await.unwrap_err();
    assert!(matches!(error, Error::UnsupportedFormat(_)), "{error}");
}

#[cfg(feature = "documents")]
#[tokio::test]
async fn brain_ingest_files_a_pdf_by_path() {
    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let engine = bind_reference(&config);
    let path = tmp.path().join("brief.pdf");
    std::fs::write(&path, pdf_saying("The vendor code is PV-7023")).unwrap();

    let view = ingest(
        &config,
        BrainIngestParams {
            path: Some(path.display().to_string()),
            text: None,
            source: None,
            title: None,
        },
    )
    .await
    .expect("a pdf is ingested, not refused");
    assert_eq!(view.source, "pdf");

    let docs = stored(&engine, MetaFilter::kinds([ItemKind::Document])).await;
    assert_eq!(docs.len(), 1);
    assert!(docs[0].text.contains("PV-7023"), "{}", docs[0].text);
}

/// Without `documents` the office parsers are not linked: a PDF is refused
/// as unsupported, never mis-read as text.
#[cfg(not(feature = "documents"))]
#[tokio::test]
async fn without_the_documents_feature_a_pdf_is_refused_cleanly() {
    let raw = RawDocument::new(pdf_saying("The vendor code is PV-7023")).with_filename("brief.pdf");
    let error = converter().convert(&raw).await.unwrap_err();
    assert!(matches!(error, Error::UnsupportedFormat(_)), "{error}");
}
