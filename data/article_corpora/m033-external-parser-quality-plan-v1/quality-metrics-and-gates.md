# M033 S06 T02 Quality Metrics and Acceptance Gates

- status: `complete`
- candidate_only: `true`
- graph_import_allowed=false
- ladybugdb_written=false
- production_import_attempted=false
- import_eligible=false

## Metric categories

Covered components: GROBID, OpenDataLoader, Adaptix, quant-mind-inspired schemas, and daily-archive review/import boundaries.

### grobid_tei_bibliography_citation_quality

- metrics:
  - header/title/author/abstract exact-or-reviewed match
  - reference count recall against golden references
  - citation marker/ref-link coverage
  - TEI parse success and well-formedness
  - section hierarchy coverage
- acceptance gate: Pass only if TEI sidecar produces valid candidate scholarly structure and bibliography/citation quality meets documented golden thresholds or emits typed blockers.

### opendataloader_layout_ocr_table_coordinate_quality

- metrics:
  - page count match
  - element bounding-box coverage
  - reading order reviewer score
  - table count/cell-structure precision-recall against golden tables
  - OCR text adequacy for scanned controls
  - caption/figure linkage candidates
- acceptance gate: Pass only if layout/table/OCR candidates meet thresholds per corpus class; scanned failures must be explicit typed blockers, not silent success.

### adaptix_adapter_contract_coverage

- metrics:
  - fixed JSON schema-shape coverage
  - typed model load success rate
  - candidate_only flag preservation
  - diagnostic count and zero unhandled exceptions
  - unknown/extra type handling
- acceptance gate: Pass only if all parser outputs map to typed candidate contracts or typed blockers without semantic promotion.

### tree_pageindex_card_provenance_schema_fit

- metrics:
  - PageIndex node coverage by section/page
  - PaperKnowledgeCard required-field completeness
  - SourceRef/EvidencePath hash/path linkage
  - citation/extraction provenance references
  - tree/card consistency checks
- acceptance gate: Pass only if quant-mind-inspired patterns are reimplemented as daily-archive schemas and validate without quant-mind runtime dependency.

### source_span_anchoring_and_staleness

- metrics:
  - stable element-id or JSON-pointer path coverage
  - PDF/source sha256 binding
  - coordinate normalization validity
  - invalid span refusal coverage
  - stale source hash refusal coverage
- acceptance gate: Pass only if candidate evidence anchors can be traced to stable local source/hash/span coordinates and stale/invalid anchors are rejected.

### low_quality_and_refusal_preservation

- metrics:
  - low_quality_source detection on controls
  - no_substantive_body detection
  - zero-chunk refusal preservation
  - metadata-only control blocked
  - unsafe/missing source typed blocker
- acceptance gate: Pass only if external parser success cannot bypass current daily-archive refusal diagnostics.

### review_packet_and_graph_readiness_boundary

- metrics:
  - review packet completeness
  - output_contract_completed=true only after explicit review
  - graph-readiness review post-check pass
  - accepted_count/import_eligible_count remain zero in no-write rehearsal
  - LadybugDB write attempts zero
- acceptance gate: Pass only if review artifact post-check succeeds before manifest synthesis and all graph/import/write flags remain false unless separately authorized outside M033.

## Global acceptance rules

- parser output is candidate evidence only
- no production integration from quality plan
- no graph import or LadybugDB write
- all diagnostic failures must be typed
- no secrets or raw article bodies in logs
- graph-readiness review post-check required before any future manifest synthesis

## Review post-check

`uv run python -m arxiv_archive.graph_readiness_review --review-dir <review-dir> --events <events.jsonl> --validate-only --require-completed-review`
