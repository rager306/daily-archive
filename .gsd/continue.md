# Continue — M420 DATA triage admission shipped (pending GSD seal)

## Last action

M420-okuzyt S01+S02 implemented (code green; GSD skip-delivered closeout next):

- S01 domain: `PdfTriagePolicy.max_garbled_page_percent` (u8 0..=100, default
  100) + typed `PdfTriageDecision::{Block{ratio}, Continue}`. Bundled YAML
  stays permissive. `encoding_issues_block_grobid` kept as silent compat
  (always false). Zero-page → Continue, never divide.
- S02 ingest: after classify, `decide()` before GROBID. Block +
  `fail_open=false` bails with typed error; Block + `fail_open=true` warns
  `triage_decision=block_continue`. Tests inject policy via
  `IngestUseCase::with_pdf_triage_policy`. Governor pins
  `triage_decision_site` + `pdf_triage_admission_threshold`.

TSR as Wave 2-a head was rejected: `extract_tables_with_structure_*` needs
independent cell bboxes; feeding GROBID TEI into TSR is tautological.

## Next action (priority)

1. **Wave 2-b pdf-inspector** (remaining followUps M419):
   a) ToUnicode/markdown repair for garbled pages → typed page-level
      missingness in `ParsedArticle` (changes parse contract/cache — careful);
   b) page-scope / detect-only options for canaries.
2. Либо P1 remote GraphStore (`DA_GRAPH_MODE=remote` → NotConfigured).

## Do not

Unlock D127. Commit `.env`. Retry TSR-as-second-structural-source without
an independent layout-model cell detector. Soak-wakeup loops. Feature-gated
code must pass default-build clippy. `#[deprecated]` on
`encoding_issues_block_grobid` fails `-D warnings` even in-crate.
