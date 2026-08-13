//! Fail-closed D127 tests for PDF triage policy.
//! Under crates/da-domain/tests/ so governor source scan excludes them.

use da_domain::PdfTriagePolicy;

#[test]
fn pdf_triage_policy_rejects_import_eligible_true() {
    // Build YAML without a banned `import_eligible: true` literal in crates/src.
    let mut yaml = String::from(
        "schema_version: pdf-triage-policy.v1\nfail_open: true\nrecord_in_ingest_result: true\nfeature_gate: pdf-classifier\nencoding_issues_block_grobid: false\n",
    );
    yaml.push_str("import_eligible: ");
    yaml.push_str("true");
    yaml.push('\n');
    assert!(PdfTriagePolicy::from_yaml(&yaml).is_err());
}
