use serde::Deserialize;
use serde_yaml::Value;
use std::collections::{HashMap, HashSet};

const FIXTURE: &str = include_str!("fixtures/fulltext_block_semantics_oracle.yaml");
const DOCS: [&str; 3] = ["hub-2507.19457", "rlm-2512.24601", "reca-2605.26525v1"];
const LANES: [&str; 3] = [
    "pdfalto",
    "pdf_inspector_positioned",
    "pdf_inspector_positioned_xy_cut",
];
const FAMILIES: [&str; 6] = [
    "paragraph",
    "section",
    "list_item",
    "note",
    "annex",
    "marker",
];
#[derive(Debug, Deserialize)]
struct Oracle {
    schema_version: String,
    independent: bool,
    source_bound: bool,
    forbidden_fields: Vec<String>,
    source_contract: Contract,
    documents: Vec<Document>,
    lanes: Vec<String>,
    families: Vec<String>,
    upstream_outcomes: Vec<Outcome>,
    page_receipts: PageReceipts,
    annotations: Vec<Annotation>,
    cell_evidence: CellEvidence,
}
#[derive(Debug, Deserialize)]
struct Contract {
    page_count: u32,
    lane_page_count: u32,
    unpooled_cell_count: u32,
}
#[derive(Debug, Deserialize)]
struct Document {
    id: String,
    source_sha256: String,
    pages: u32,
}
#[derive(Debug, Deserialize, Clone)]
struct Outcome {
    document_id: String,
    lane: String,
    status: String,
    failure_code: Option<String>,
}
#[derive(Debug, Deserialize)]
struct PageReceipts {
    entries: Vec<PageReceipt>,
}
#[derive(Debug, Deserialize, Clone)]
struct PageReceipt {
    document_id: String,
    lane: String,
    start_page: u32,
    end_page: u32,
    receipt_digest: String,
    status: String,
}
#[derive(Debug, Deserialize, Clone)]
struct Span {
    page: u32,
    block: u32,
    line: u32,
    token_start: u32,
    token_end: u32,
}
#[derive(Debug, Deserialize, Clone)]
struct Annotation {
    id: String,
    document_id: String,
    lane: String,
    region: String,
    family: String,
    span: Option<Span>,
    digest: Option<String>,
    provenance: String,
    parent_id: Option<String>,
    subtype: Option<String>,
    disposition: Option<String>,
}
#[derive(Debug, Deserialize, Clone)]
struct Cell {
    document_id: String,
    lane: String,
    family: String,
    status: String,
}
#[derive(Debug, Deserialize)]
struct CellEvidence {
    cells: Vec<Cell>,
}

fn reject_forbidden_keys(value: &Value, forbidden: &[&str]) -> Result<(), String> {
    match value {
        Value::Mapping(map) => {
            for (key, child) in map {
                if let Some(name) = key.as_str()
                    && (forbidden.contains(&name) || name == "target" || name == "resolution")
                {
                    return Err(format!("forbidden field: {name}"));
                }
                reject_forbidden_keys(child, forbidden)?;
            }
        }
        Value::Sequence(items) => {
            for child in items {
                reject_forbidden_keys(child, forbidden)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate(raw: &str) -> Result<(), String> {
    let value: Value = serde_yaml::from_str(raw).map_err(|e| format!("yaml: {e}"))?;
    reject_forbidden_keys(
        &value,
        &[
            "raw_text",
            "text",
            "tei",
            "tei_xml",
            "pdf",
            "pdf_bytes",
            "image",
            "page_image",
            "path",
            "physical_path",
            "host",
            "hostname",
            "executable",
            "vendor_path",
        ],
    )?;
    let oracle: Oracle =
        serde_yaml::from_value(value.clone()).map_err(|e| format!("shape: {e}"))?;
    if oracle.schema_version != "fulltext-block-semantics-oracle.v1"
        || !oracle.independent
        || !oracle.source_bound
        || oracle.forbidden_fields.len() < 10
    {
        return Err("source-bound independent schema required".into());
    }
    if oracle.documents.len() != 3 || oracle.lanes != LANES || oracle.families != FAMILIES {
        return Err("exact corpus, lanes, and six-family vocabulary required".into());
    }
    if oracle.source_contract.page_count != 202
        || oracle.source_contract.lane_page_count != 606
        || oracle.source_contract.unpooled_cell_count != 54
    {
        return Err("denominator drift".into());
    }
    let doc_map: HashMap<_, _> = oracle
        .documents
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();
    let mut seen_docs = HashSet::new();
    for d in &oracle.documents {
        if !seen_docs.insert(&d.id)
            || !DOCS.contains(&d.id.as_str())
            || d.pages == 0
            || d.source_sha256.len() != 64
        {
            return Err("invalid or duplicate document identity".into());
        }
    }
    if oracle.upstream_outcomes.len() != 9 {
        return Err("all nine upstream outcomes required".into());
    }
    let mut outcomes = HashMap::new();
    for o in &oracle.upstream_outcomes {
        if !doc_map.contains_key(o.document_id.as_str())
            || !LANES.contains(&o.lane.as_str())
            || o.status == "pass" && o.failure_code.is_some()
        {
            return Err("invalid upstream outcome".into());
        }
        if o.status != "pass" && o.failure_code.is_none() {
            return Err("failure needs typed failure_code".into());
        }
        if outcomes
            .insert((o.document_id.as_str(), o.lane.as_str()), o)
            .is_some()
        {
            return Err("duplicate upstream outcome".into());
        }
    }
    let mut pages = HashSet::new();
    for p in &oracle.page_receipts.entries {
        let d = doc_map
            .get(p.document_id.as_str())
            .ok_or("page receipt unknown document")?;
        if !LANES.contains(&p.lane.as_str())
            || p.start_page != 1
            || p.end_page != d.pages
            || p.receipt_digest.len() != 64
        {
            return Err("page-complete receipt mismatch".into());
        }
        let outcome = outcomes
            .get(&(p.document_id.as_str(), p.lane.as_str()))
            .ok_or("page receipt without outcome")?;
        let expected = if outcome.status == "pass" {
            "reviewed"
        } else {
            "propagated_upstream_invalid_evidence"
        };
        let expected = if outcome.status == "terminal_failure" {
            "propagated_upstream_terminal_failure"
        } else {
            expected
        };
        if p.status != expected {
            return Err("upstream disposition was erased or changed".into());
        }
        if !pages.insert((p.document_id.as_str(), p.lane.as_str())) {
            return Err("duplicate page receipt".into());
        }
    }
    if pages.len() != 9 {
        return Err("missing page receipt".into());
    }
    let mut cells = HashSet::new();
    for c in &oracle.cell_evidence.cells {
        if !doc_map.contains_key(c.document_id.as_str())
            || !LANES.contains(&c.lane.as_str())
            || !FAMILIES.contains(&c.family.as_str())
            || !cells.insert((c.document_id.as_str(), c.lane.as_str(), c.family.as_str()))
        {
            return Err("cell denominator is pooled, duplicated, or vocabulary-drifted".into());
        }
        let upstream = outcomes
            .get(&(c.document_id.as_str(), c.lane.as_str()))
            .unwrap();
        if upstream.status != "pass" {
            let expected = if upstream.status == "terminal_failure" {
                "propagated_upstream_terminal_failure"
            } else {
                "propagated_upstream_invalid_evidence"
            };
            if c.status != expected {
                return Err("non-pass upstream cell became pass/zero".into());
            }
        } else if c.status != "reviewed"
            && c.status != "source_not_established"
            && c.status != "reviewed_absent"
            && c.status != "no_positive_denominator"
        {
            return Err("reviewed cell lacks reviewed or typed absence evidence".into());
        }
    }
    if cells.len() != 54 {
        return Err("exactly 54 unpooled cells required".into());
    }

    let mut ids = HashSet::new();
    let mut spans = Vec::new();
    for a in &oracle.annotations {
        if !ids.insert(&a.id)
            || !doc_map.contains_key(a.document_id.as_str())
            || !LANES.contains(&a.lane.as_str())
            || !FAMILIES.contains(&a.family.as_str())
            || a.provenance != "independently_reviewed"
        {
            return Err("annotation lacks independent provenance or closed vocabulary".into());
        }
        if a.family == "note" {
            if a.disposition.as_deref() != Some("source_not_established")
                || a.span.is_some()
                || a.digest.is_some()
            {
                return Err("note positive invention or footnote conflation".into());
            }
        } else {
            let span = a.span.as_ref().ok_or("positive annotation missing span")?;
            let digest = a
                .digest
                .as_ref()
                .ok_or("positive annotation missing digest")?;
            if digest.len() != 64
                || digest
                    .chars()
                    .all(|c| c == digest.chars().next().unwrap_or('\0'))
                || span.token_start >= span.token_end
            {
                return Err("invalid annotation span/digest".into());
            }
            if a.family == "marker"
                && !matches!(
                    a.subtype.as_deref(),
                    Some("citation" | "figure" | "table" | "equation")
                )
            {
                return Err("marker subtype is not closed".into());
            }
            if a.family == "annex" && a.region != "annex" {
                return Err("annex parent outside annex region".into());
            }
            if a.family != "annex" && a.region == "annex" && a.parent_id.is_none() {
                return Err("annex leaf missing parent".into());
            }
            if a.parent_id.is_some() && (a.region != "annex" || a.family == "annex") {
                return Err("invalid annex parent relation".into());
            }
            spans.push((
                a.region.as_str(),
                a.span.as_ref().unwrap().clone(),
                a.family.as_str(),
                a.parent_id.as_deref(),
            ));
        }
    }
    for (i, (region, a, family, parent)) in spans.iter().enumerate() {
        for (other_region, b, other_family, _) in spans.iter().skip(i + 1) {
            if region == other_region
                && family != &"annex"
                && other_family != &"annex"
                && a.page == b.page
                && a.block == b.block
                && a.line == b.line
                && a.token_start < b.token_end
                && b.token_start < a.token_end
            {
                return Err("sibling span overlap".into());
            }
        }
        if family != &"annex" && region == &"annex" && parent.is_none() {
            return Err("body-annex crossing".into());
        }
    }
    Ok(())
}

#[test]
fn reviewed_oracle_is_complete_independent_and_digest_only() {
    validate(FIXTURE).unwrap();
    let value: Value = serde_yaml::from_str(FIXTURE).unwrap();
    let encoded = serde_json::to_string(&value).unwrap();
    for forbidden in [
        "raw_text",
        "tei_xml",
        "pdf_bytes",
        "page_image",
        "physical_path",
        "hostname",
        "executable",
    ] {
        assert!(
            !encoded.contains(&format!("\"{forbidden}\":")),
            "forbidden payload key: {forbidden}"
        );
    }
}

#[test]
fn adversarial_mutations_fail_closed() {
    let mutations = [
        ("unpooled_cell_count: 54", "unpooled_cell_count: 53"),
        (
            "page_receipts:\n  mode:",
            "page_receipts:\n  entries: []\n  mode:",
        ),
        (
            "family: marker, status: reviewed",
            "family: marker, status: pass",
        ),
        (
            "family: note, disposition: source_not_established",
            "family: note, disposition: reviewed, span: {page: 94, block: 0, line: 2, token_start: 0, token_end: 1}, digest: \"5e1a9c7d2b4f6083e9c1a7d5b2f40618e3c9a7d1b5f20864c3e9a7d1b5f2086\"",
        ),
        (
            "family: marker, subtype: citation",
            "family: marker, subtype: citation, target: x",
        ),
        (
            "region: annex, family: paragraph",
            "region: body, family: paragraph",
        ),
        (
            "    - {document_id: hub-2507.19457, lane: pdf_inspector_positioned, family: paragraph, status: reviewed}\n",
            "    - {document_id: hub-2507.19457, lane: pdf_inspector_positioned, family: paragraph, status: reviewed}\n    - {document_id: hub-2507.19457, lane: pdf_inspector_positioned, family: paragraph, status: reviewed}\n",
        ),
    ];
    for (needle, replacement) in mutations {
        let mutated = FIXTURE.replacen(needle, replacement, 1);
        assert!(
            validate(&mutated).is_err(),
            "mutation unexpectedly accepted: {needle}"
        );
    }
}

#[test]
fn forbidden_payload_and_placeholder_digest_are_rejected() {
    let raw = FIXTURE.replacen("digest: \"0f4a9c2d71b8e603c5a17e94f2d8a6319b0c7e45a2d6f8c1039e5b7a4c1d2e6f\"", "digest: \"0f4a9c2d71b8e603c5a17e94f2d8a6319b0c7e45a2d6f8c1039e5b7a4c1d2e6f\"\n    raw_text: leaked", 1);
    assert!(validate(&raw).is_err());
    let raw = FIXTURE.replacen(
        "digest: \"1a7c4e902bd653f8e0c1a9d47b3265f081e2c6a93d7b4f5018c9e2a6d3b7f4c5\"",
        "digest: \"0000000000000000000000000000000000000000000000000000000000000000\"",
        1,
    );
    assert!(validate(&raw).is_err());
}
