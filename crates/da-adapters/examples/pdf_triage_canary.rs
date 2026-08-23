//! pdf_triage_canary — measure pdf-inspector Full vs DetectOnly (M423).
//!
//! The product classifier always runs `ProcessMode::Full` (markdown included).
//! Before anyone flips it to `PdfOptions::detect_only()`, this canary runs
//! BOTH modes on a real PDF and reports a typed per-field agreement
//! (`TriageModeDiff`). Switching modes becomes a measured decision, not a
//! hope — the M349 lesson was exactly that `classify_pdf_mem` (a detect-only
//! shortcut) disagreed with `process_pdf_mem` and hid OCR pages.
//!
//! Page scope (`--pages`) is CANARY-ONLY: `PdfOptions::pages(..)` filters the
//! OCR detection to a subset while `page_count` stays document-wide, so a
//! page-scoped list must never feed `PdfTriagePolicy::garbled_ratio`.
//!
//! Build/run:
//!   cargo run -p da-adapters --features pdf-classifier --example pdf_triage_canary -- \
//!       --pdf data/sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf \
//!       [--pages 69,84]
//!
//! Exit codes: 0 = modes agree, 2 = divergence (see `diff`), 1 = error.

use std::time::Instant;

use da_adapters::pdf_classifier::classify_sync_with_options;
use da_domain::{PdfClassification, TriageModeDiff};

fn usage() -> ! {
    eprintln!(
        "usage: pdf_triage_canary --pdf PATH [--pages 1,2,3]\n\
         \x20 exit 0 = Full and DetectOnly agree; 2 = divergence; 1 = error"
    );
    std::process::exit(1);
}

fn classification_json(label: &str, c: &PdfClassification, ms: u128) -> serde_json::Value {
    serde_json::json!({
        "mode": label,
        "elapsed_ms": ms,
        "pdf_type": format!("{:?}", c.pdf_type),
        "page_count": c.page_count,
        "pages_needing_ocr": c.pages_needing_ocr,
        "ocr_reasons_by_page": c.ocr_reasons_by_page
            .iter()
            .map(|r| serde_json::json!({"page": r.page, "reasons": r.reasons}))
            .collect::<Vec<_>>(),
        "has_encoding_issues": c.has_encoding_issues,
        "confidence": c.confidence,
    })
}

fn main() {
    let mut pdf: Option<String> = None;
    let mut pages: Option<Vec<u32>> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pdf" => {
                pdf = args.next().or_else(|| {
                    eprintln!("--pdf requires a path");
                    usage()
                })
            }
            "--pages" => {
                let raw = args.next().unwrap_or_else(|| {
                    eprintln!("--pages requires a comma list, e.g. 69,84");
                    usage()
                });
                pages = Some(
                    raw.split(',')
                        .map(|s| {
                            s.trim().parse::<u32>().unwrap_or_else(|_| {
                                eprintln!("invalid page number {s:?} in --pages {raw:?}");
                                usage()
                            })
                        })
                        .collect(),
                );
            }
            other => {
                eprintln!("unknown argument {other:?}");
                usage();
            }
        }
    }
    let pdf_path = pdf.unwrap_or_else(|| {
        eprintln!("--pdf is required");
        usage()
    });

    let bytes = match std::fs::read(&pdf_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("cannot read {pdf_path}: {error}");
            std::process::exit(1);
        }
    };

    let start = Instant::now();
    let full = match classify_sync_with_options(&bytes, pdf_inspector::PdfOptions::new()) {
        Ok(classification) => classification,
        Err(error) => {
            eprintln!("Full mode failed: {error}");
            std::process::exit(1);
        }
    };
    let full_ms = start.elapsed().as_millis();

    let start = Instant::now();
    let detect_only =
        match classify_sync_with_options(&bytes, pdf_inspector::PdfOptions::detect_only()) {
            Ok(classification) => classification,
            Err(error) => {
                eprintln!("DetectOnly mode failed: {error}");
                std::process::exit(1);
            }
        };
    let detect_only_ms = start.elapsed().as_millis();

    let diff = TriageModeDiff::compare(&full, &detect_only);
    let agree = diff.agree();

    let mut report = serde_json::json!({
        "pdf": pdf_path,
        "full": classification_json("full", &full, full_ms),
        "detect_only": classification_json("detect_only", &detect_only, detect_only_ms),
        "diff": diff,
        "agree": agree,
    });

    if let Some(pages) = pages {
        let start = Instant::now();
        let scoped = classify_sync_with_options(
            &bytes,
            pdf_inspector::PdfOptions::detect_only().pages(pages.iter().copied()),
        );
        let scoped_ms = start.elapsed().as_millis();
        match scoped {
            Ok(scoped) => {
                report["page_scoped"] = serde_json::json!({
                    "mode": "detect_only+pages",
                    "pages": pages,
                    "elapsed_ms": scoped_ms,
                    "page_count": scoped.page_count,
                    "pages_needing_ocr": scoped.pages_needing_ocr,
                    "page_scope_canary_only": true,
                    "warning": "page_count stays document-wide; a filtered OCR list must never feed PdfTriagePolicy::garbled_ratio",
                });
            }
            Err(error) => {
                report["page_scoped"] = serde_json::json!({
                    "error": error.to_string(),
                    "page_scope_canary_only": true,
                });
            }
        }
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("serialize report")
    );
    if !agree {
        // Divergence is the finding, not a crash — exit 2 so scripts can see it.
        std::process::exit(2);
    }
}
