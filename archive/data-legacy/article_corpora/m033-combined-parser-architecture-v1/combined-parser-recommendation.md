# M033 S05 T02 Combined Parser Architecture Recommendation

- verdict: `recommended-bounded-combined-sidecar-architecture`
- candidate_only: `true`
- production_adoption_authorized: `false`
- runtime_dependency_adoption_authorized: `false`
- graph_import_allowed=false
- ladybugdb_written=false
- production_import_attempted=false
- import_eligible=false

## Recommendation

Use a bounded combined sidecar architecture, not a parser replacement or production import path. Parser outputs are candidate evidence only.

## Recommended flow

1. source acquisition proves local PDF/source path, hash, size, and catalog identity
2. GROBID produces scholarly TEI sidecar candidates
3. OpenDataLoader-style backend produces layout/OCR/table/page-coordinate sidecar candidates
4. Adaptix maps fixed OpenDataLoader JSON into typed daily-archive candidate summaries
5. quant-mind-inspired schemas guide PageIndex/tree/card/provenance design under daily-archive ownership
6. daily-archive validators normalize diagnostics and reject low-quality/unsafe/stale/source-span failures
7. completed review packet may be prepared for graph-readiness review; parser output alone is never import eligibility

## Component responsibilities

### GROBID

- role: `scholarly_tei_sidecar`
- adapter boundary: TEI-to-daily-archive candidate adapter with source/ref anchors and bibliography/citation review gates
- owns:
  - header/title/authors/abstract candidates
  - section and TEI hierarchy candidates
  - bibliography entries
  - citation/reference markers
- must not own:
  - table fidelity proof
  - OCR replacement
  - graph-readiness decision
  - LadybugDB import trigger

### OpenDataLoader-style extraction

- role: `layout_ocr_table_coordinate_sidecar`
- adapter boundary: fixed JSON/Markdown/HTML/Text outputs become candidate sidecar artifacts only
- owns:
  - page/layout blocks
  - bounding boxes and page coordinates
  - reading-order candidates
  - table/figure/layout candidates
  - backend diagnostics and model-cache state
- must not own:
  - bibliography authority
  - semantic fact promotion
  - graph-ready chunks without daily-archive validation
  - production import eligibility

### Adaptix

- role: `typed_adapter_layer`
- adapter boundary: post-processing layer after parser output, before daily-archive validators/review packets
- owns:
  - structural mapping from fixed OpenDataLoader JSON into typed candidate summaries
  - shape/type validation for adapter outputs
  - explicit candidate_only flags
- must not own:
  - semantic validation
  - reading-order correctness
  - table fidelity
  - source-span truth
  - graph readiness

### quant-mind patterns

- role: `architecture_pattern_source`
- adapter boundary: ideas reimplemented under daily-archive-owned schemas, never imported as runtime dependency in this track
- owns:
  - TreeKnowledge/PageIndex inspiration
  - PaperKnowledgeCard summary-card split
  - SourceRef/Citation/ExtractionRef provenance ideas
  - fetch-format-flow separation
  - bounded batch concurrency
  - typed resolver guardrails
- must not own:
  - runtime dependency
  - OpenAI/API extraction proof
  - GraphKnowledge/storage/retrieval/RAG platform
  - semantic KG implementation

### daily-archive

- role: `contract_validator_review_owner`
- adapter boundary: all parser sidecars terminate in candidate artifacts until validators and independent review complete
- owns:
  - SourceRef/EvidencePath/PageIndex/SemanticChunk contracts
  - diagnostic taxonomy
  - review packet completion
  - graph-readiness review
  - no-write import rehearsal
  - final adoption decision
- must not own:
  - blind trust in parser sidecar output
  - positive import eligibility from parser success alone

## Rejected alternatives

- **GROBID-only parser replacement**: GROBID is strong for scholarly TEI/bibliography/citations but does not prove table fidelity, OCR replacement, or layout-coordinate completeness.
- **OpenDataLoader-only parser replacement**: OpenDataLoader provides layout/OCR/table/page candidates but not scholarly bibliography authority or graph-ready semantic contracts.
- **Adaptix as semantic validator**: Adaptix validates structure and mapping shape; it cannot validate reading order, table fidelity, source spans, or factual correctness.
- **quant-mind runtime adoption**: quant-mind requires OpenAI/API/network-coupled flows for its live pipeline and has placeholder/missing GraphKnowledge/storage/retrieval/RAG layers.
- **direct parser-to-LadybugDB import**: M033 is research only; graph-readiness review and no-write import rehearsal remain independent blockers.
