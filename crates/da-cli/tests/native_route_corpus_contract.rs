//! Contract test for the S03 (M516-th0oby) native PDF-to-TEI route corpus.
//!
//! `data/parse/layout_bakeoff.yaml#native_route_corpus` is the route's own
//! expanded corpus: the three S02 corridor documents plus nine documents drawn
//! from the frozen holdout measurable set. `java_free_vertical_slice` reads it
//! by pointer (`native_route_corpus.documents`), so every declared identity must
//! be real, byte-pinned and page-pinned before any measurement may trust it.
//!
//! This file proves, against the live DATA and the live PDFs:
//!
//! * the block resolves through `load_declaration` and the slice binds exactly
//!   its pointer (the layout donor corpus is not the route's corpus);
//! * `corpus_n` / `documents.len()` agree and the declared `page_denominator`
//!   equals the sum of the pinned page counts;
//! * every `source_pdf` exists, hashes to its pinned bare sha256, and carries
//!   exactly the declared number of pages under pdf-inspector (the parser the
//!   route runner itself uses);
//! * the named mutations are refused: document-count drift, page-denominator
//!   drift, a slice binding that disagrees with the block, sha drift, a PDF
//!   page-count mismatch, and a pointer that escapes the declaration manifest.
//!
//! Unlike the boundary contract, this test necessarily reads the gitignored
//! `data/sources/**` PDFs: a pin that is never checked against the bytes is
//! prose, not evidence. The corpus is present locally (the S03 plan records all
//! 610 manifest PDFs present), and the test is feature-gated so it only runs
//! with the parser features that make page counting possible.

#![cfg(all(feature = "pdf-layout", feature = "pdf-classifier"))]

use da_adapters::pdf_classifier::classify_sync_with_options;
use da_adapters::pdf_inspector::PdfOptions;
use da_cli::java_free_slice::{load_declaration, sha256_hex};
use serde_yaml::Value;
use std::fs;
use std::path::{Path, PathBuf};

const LAYOUT: &str = "data/parse/layout_bakeoff.yaml";
const COMPATIBILITY: &str = "data/parse/grobid_compatibility.yaml";
const BLOCK: &str = "native_route_corpus";
const POINTER: &str = "native_route_corpus.documents";
const CORRIDOR: [&str; 3] = ["hub-2507.19457", "rlm-2512.24601", "reca-2605.26525v1"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_text(rel: &str) -> String {
    fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|error| panic!("{rel}: {error}"))
}

fn layout() -> Value {
    serde_yaml::from_str(&read_text(LAYOUT)).expect("layout_bakeoff.yaml is valid YAML")
}

fn compatibility() -> String {
    read_text(COMPATIBILITY)
}

/// Pure shape / cross-reference errors; no PDF bytes are read. A drifted count,
/// denominator, or an escaping pointer is refused here by name.
fn shape_errs(layout: &Value, compatibility: &str) -> Vec<String> {
    let mut errs = Vec::new();
    let block = &layout[BLOCK];
    if block.is_null() {
        errs.push(format!("[native-route-corpus] `{BLOCK}` block is absent"));
        return errs;
    }
    if block["id"].as_str().map(str::trim).unwrap_or("").is_empty() {
        errs.push("[native-route-corpus] id must be a non-empty string".to_owned());
    }
    match block["documents"].as_sequence() {
        None => errs.push("[native-route-corpus] documents must be a sequence".to_owned()),
        Some(documents) => {
            if documents.is_empty() {
                errs.push("[native-route-corpus] documents must not be empty".to_owned());
            }
            if block["corpus_n"].as_u64() != Some(documents.len() as u64) {
                errs.push("[native-route-corpus] corpus_n must equal documents.len()".to_owned());
            }
            let page_sum: i64 = documents
                .iter()
                .filter_map(|document| document["pages"].as_i64())
                .sum();
            if block["page_denominator"].as_u64() != Some(page_sum.max(0) as u64) {
                errs.push(
                    "[native-route-corpus] page_denominator must equal the sum of pages".to_owned(),
                );
            }
        }
    }

    // The slice must resolve the declared pointer; this is where a mutated
    // binding count or a manifest-escaping pointer is refused.
    let text = serde_yaml::to_string(layout).expect("layout serializes");
    match load_declaration(&text, compatibility) {
        Err(error) => errs.push(format!(
            "[native-route-corpus] load_declaration refused the declaration: {error}"
        )),
        Ok(declaration) => {
            if declaration.corpus_pointer != POINTER {
                errs.push(format!(
                    "[native-route-corpus] the slice must bind `{POINTER}`, found `{}`",
                    declaration.corpus_pointer
                ));
            }
        }
    }
    errs
}

/// Byte-level errors: every declared PDF must exist, hash to its pin, and carry
/// exactly the declared page count. `fail_fast` stops after the first finding so
/// a mutation test pays for one document, not all twelve.
fn byte_errs(layout: &Value, fail_fast: bool) -> Vec<String> {
    let mut errs = Vec::new();
    let documents = layout[BLOCK]["documents"]
        .as_sequence()
        .cloned()
        .unwrap_or_default();
    for document in &documents {
        let id = document["id"].as_str().unwrap_or("<unnamed>");
        let rel = document["source_pdf"].as_str().unwrap_or("");
        let declared_sha = document["sha256"].as_str().unwrap_or("");
        let declared_pages = document["pages"].as_u64();
        let path = repo_root().join(rel);
        if !path.is_file() {
            errs.push(format!(
                "[native-route-corpus] `{id}` source_pdf `{rel}` is absent"
            ));
            if fail_fast {
                return errs;
            }
            continue;
        }
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{rel}: {error}"));
        if sha256_hex(&bytes) != declared_sha {
            errs.push(format!(
                "[native-route-corpus] `{id}` sha256 does not match the bytes at `{rel}`"
            ));
            if fail_fast {
                return errs;
            }
            continue;
        }
        match classify_sync_with_options(&bytes, PdfOptions::new()) {
            Ok(classification) if Some(classification.page_count as u64) == declared_pages => {}
            Ok(classification) => {
                errs.push(format!(
                    "[native-route-corpus] `{id}` declares {declared_pages:?} pages but the PDF has {}",
                    classification.page_count
                ));
                if fail_fast {
                    return errs;
                }
            }
            Err(error) => {
                errs.push(format!(
                    "[native-route-corpus] `{id}` page count unavailable: {error:?}"
                ));
                if fail_fast {
                    return errs;
                }
            }
        }
    }
    errs
}

#[test]
fn the_native_route_corpus_is_pinned_to_real_pdfs() {
    let layout = layout();
    let compatibility = compatibility();

    // The declaration resolves, and the route binds the new block, not the
    // layout donor corpus.
    let declaration = load_declaration(&read_text(LAYOUT), &compatibility)
        .expect("the slice declaration must load");
    assert_eq!(declaration.corpus_pointer, POINTER);
    assert_eq!(declaration.documents.len(), 12);
    assert_eq!(declaration.page_denominator, 381);

    let block = &layout[BLOCK];
    assert_eq!(block["id"].as_str(), Some("native-pdf-to-tei-route-corpus"));
    assert_eq!(block["corpus_n"].as_u64(), Some(12));
    assert_eq!(block["page_denominator"].as_u64(), Some(381));

    // The three S02 corridor identities are carried verbatim, so the expanded
    // corpus strictly extends the corridor rather than replacing it.
    let ids: Vec<&str> = declaration
        .documents
        .iter()
        .map(|document| document.id.as_str())
        .collect();
    for corridor in CORRIDOR {
        assert!(
            ids.contains(&corridor),
            "the corridor identity `{corridor}` must be retained"
        );
    }

    assert_eq!(
        shape_errs(&layout, &compatibility),
        Vec::<String>::new(),
        "the live corpus block must be internally consistent"
    );
    assert_eq!(
        byte_errs(&layout, false),
        Vec::<String>::new(),
        "every declared PDF must match its sha256 and page pin"
    );
}

#[test]
fn native_route_corpus_mutations_fail_visible() {
    // 1. Document-count drift inside the corpus block.
    let mut mutated = layout();
    mutated[BLOCK]["corpus_n"] = Value::Number(13.into());
    assert!(
        !shape_errs(&mutated, &compatibility()).is_empty(),
        "a drifted corpus_n must be refused"
    );

    // 2. Page-denominator drift inside the corpus block.
    let mut mutated = layout();
    let denominator = mutated[BLOCK]["page_denominator"].as_u64().unwrap();
    mutated[BLOCK]["page_denominator"] = Value::Number((denominator + 1).into());
    assert!(
        !shape_errs(&mutated, &compatibility()).is_empty(),
        "a drifted page_denominator must be refused"
    );

    // 3. Slice binding count that disagrees with the resolved block.
    let mut mutated = layout();
    mutated["java_free_vertical_slice"]["corpus_binding"]["document_count"] =
        Value::Number(3.into());
    assert!(
        !shape_errs(&mutated, &compatibility()).is_empty(),
        "a slice binding count that disagrees with the block must be refused"
    );

    // 4. A well-formed but wrong sha256 pin (byte-level refusal).
    let mut mutated = layout();
    mutated[BLOCK]["documents"].as_sequence_mut().unwrap()[0]["sha256"] =
        Value::String("0".repeat(64));
    assert!(
        !byte_errs(&mutated, true).is_empty(),
        "a drifted sha256 pin must be refused against the bytes"
    );

    // 5. A page pin that is internally consistent but wrong (byte-level refusal).
    let mut mutated = layout();
    {
        let documents = mutated[BLOCK]["documents"].as_sequence_mut().unwrap();
        let first_pages = documents[0]["pages"].as_u64().unwrap();
        documents[0]["pages"] = Value::Number((first_pages + 1).into());
        let denominator = mutated[BLOCK]["page_denominator"].as_u64().unwrap() + 1;
        mutated[BLOCK]["page_denominator"] = Value::Number(denominator.into());
    }
    assert_eq!(
        shape_errs(&mutated, &compatibility()),
        Vec::<String>::new(),
        "the page mutation must stay internally consistent"
    );
    assert!(
        !byte_errs(&mutated, true).is_empty(),
        "a page pin that disagrees with the PDF page count must be refused"
    );

    // 6. Pointers that escape the declaration manifest.
    for escape in [
        "data/parse/layout_bakeoff.yaml#corpus.documents",
        "../corpus.documents",
        "corpus..documents",
        "corpus",
        "corpus.documents.json",
    ] {
        let mut mutated = layout();
        mutated["java_free_vertical_slice"]["corpus_binding"]["pointer"] =
            Value::String(escape.to_owned());
        assert!(
            !shape_errs(&mutated, &compatibility()).is_empty(),
            "an escaping pointer `{escape}` must be refused"
        );
    }
}
