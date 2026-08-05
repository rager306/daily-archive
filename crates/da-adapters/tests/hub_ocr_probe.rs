//! Hub PDF OCR diagnosis (M351).
//! Pages 69 and 84 are flagged suspected_garbled_text (broken ToUnicode/CID),
//! not scanned/no_text. Regression-locked so triage semantics do not silently drift.
#![cfg(feature = "pdf-classifier")]

use da_adapters::PdfInspectorClassifier;
use da_ports::PdfClassificationPort;
use std::path::PathBuf;

fn hub_pdf() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/source/2507.19457.pdf",
    )
}

#[tokio::test]
async fn hub_pdf_ocr_pages_are_suspected_garbled_text() {
    let pdf = hub_pdf();
    assert!(pdf.exists(), "hub PDF missing: {}", pdf.display());
    let bytes = std::fs::read(&pdf).expect("read pdf");
    let c = PdfInspectorClassifier::new();
    let r = c.classify(&bytes).await.expect("classify");

    assert_eq!(r.pdf_type, da_domain::PdfType::Text);
    assert_eq!(r.page_count, 96);
    assert_eq!(r.pages_needing_ocr, vec![69, 84]);
    assert!(r.has_encoding_issues);
    assert!((r.confidence - 1.0).abs() < f32::EPSILON);

    let mut reasons_69 = None;
    let mut reasons_84 = None;
    for page in &r.ocr_reasons_by_page {
        if page.page == 69 {
            reasons_69 = Some(page.reasons.clone());
        }
        if page.page == 84 {
            reasons_84 = Some(page.reasons.clone());
        }
    }
    let r69 = reasons_69.expect("page 69 must have OCR reasons");
    let r84 = reasons_84.expect("page 84 must have OCR reasons");
    assert!(
        r69.iter().any(|x| x == "suspected_garbled_text"),
        "page 69 reasons={r69:?}"
    );
    assert!(
        r84.iter().any(|x| x == "suspected_garbled_text"),
        "page 84 reasons={r84:?}"
    );
    // Not pure scanned/no_text-only for these pages
    assert!(
        !r69.iter().any(|x| x == "scanned"),
        "page 69 should not be scanned-only"
    );
    assert!(
        !r84.iter().any(|x| x == "scanned"),
        "page 84 should not be scanned-only"
    );

    // Sample raw markdown for operator docs (process_pdf_mem exposes full markdown)
    let full = pdf_inspector::process_pdf_mem(&bytes).expect("process");
    if let Some(md) = full.markdown.as_deref() {
        // crude page markers if present; else log length only
        eprintln!("HUB_MARKDOWN_CHARS={}", md.len());
        // look for garbled signals on whole doc (soft encoding)
        eprintln!(
            "HUB_HAS_FFFD={}",
            md.contains('\u{FFFD}')
        );
    }
}

#[tokio::test]
async fn hub_pdf_page_markdown_samples_for_garbled_pages() {
    // Dump first ~200 printable chars around page markers when available.
    let pdf = hub_pdf();
    let bytes = std::fs::read(&pdf).expect("read");
    let full = pdf_inspector::process_pdf_mem(&bytes).expect("process");
    // Prefer per-page markdown if process exposes layout pages — otherwise whole md.
    // PdfResult has markdown Option; also check regions via process only.
    eprintln!("HUB_OCR_PAGES={:?}", full.pages_needing_ocr);
    for r in &full.ocr_reasons_by_page {
        eprintln!("HUB_REASON page={} {:?}", r.page, r.reasons);
    }
    if let Some(md) = full.markdown.as_deref() {
        // pdf-inspector markdown often uses form-feed or page headers; sample middle/end
        let sample = |s: &str, n: usize| {
            let t: String = s.chars().take(n).collect();
            t.replace('\n', "\\n")
        };
        let mid = md.len() / 2;
        let end = md.len().saturating_sub(400);
        eprintln!("HUB_MD_HEAD={}", sample(md, 200));
        eprintln!(
            "HUB_MD_MID={}",
            sample(&md[mid..], 200)
        );
        eprintln!(
            "HUB_MD_TAIL={}",
            sample(&md[end..], 200)
        );
    }
}
