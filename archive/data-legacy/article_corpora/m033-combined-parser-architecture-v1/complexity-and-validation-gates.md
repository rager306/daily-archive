# M033 S05 T03 Complexity and Validation Gates

- status: `complete`
- recommendation_verdict: `recommended-bounded-combined-sidecar-architecture`
- graph_import_allowed=false
- ladybugdb_written=false
- production_import_attempted=false
- import_eligible=false

## Risk categories

### grobid_runtime_and_accuracy

- risk: Native GROBID source build requires JDK 21 while local Java is 17; Docker CRF image worked, but full/DL image accuracy remains a future comparison question.
- validation gate: Record container/image version, service health, model/profile, API responses, TEI parse success, and bibliography/citation quality against golden papers.
- blocks adoption until:
  - TEI adapter contract tests pass
  - bibliography/citation/header eval passes
  - source/ref anchors reviewed

### opendataloader_hybrid_backend_and_cache

- risk: Hybrid docling-fast backend has lifecycle/model-cache cost; if cache is absent, model downloads or network dependency may reappear.
- validation gate: Future probe must preflight backend health, cache inventory, network-disabled mode, per-paper runtime, and fallback/blocker diagnostics.
- blocks adoption until:
  - backend lifecycle is scripted
  - cache/no-network behavior is proven
  - typed blocker taxonomy covers backend/model failures

### layout_table_ocr_fidelity

- risk: S03 did not include scanned/image-only PDFs or independent table ground truth, so OCR/table fidelity remains unproven.
- validation gate: Use golden papers with known table structures, scanned/layout-heavy controls, page coordinates, reading order checks, and reviewer adjudication.
- blocks adoption until:
  - OCR quality threshold met or typed blocker emitted
  - table structure fidelity measured
  - reading order quality measured

### source_span_and_coordinate_anchoring

- risk: Bounding boxes and TEI refs are candidate anchors; deterministic EvidencePath conventions and stale-hash protections are not yet specified for combined sidecars.
- validation gate: Define stable element IDs, JSON-pointer/path conventions, PDF hash binding, page-coordinate normalization, and invalid/stale span refusal checks.
- blocks adoption until:
  - EvidencePath adapter tests pass
  - stale source hash refusal is verified
  - invalid span refusal is verified

### adaptix_structural_vs_semantic_boundary

- risk: Adaptix can map shape/types but cannot validate semantic correctness, table fidelity, reading order, or graph-readiness.
- validation gate: Adapter tests must be paired with semantic quality reviewers and daily-archive validation gates.
- blocks adoption until:
  - typed mapping tests pass
  - semantic reviewer packet completion is required
  - no direct adapter-to-graph path exists

### quantmind_pattern_reimplementation

- risk: quant-mind patterns are useful, but runtime adoption would pull OpenAI/API/network and placeholder GraphKnowledge/storage/retrieval/RAG assumptions.
- validation gate: Only reimplement selected patterns under daily-archive schemas; prohibit quant-mind runtime imports in implementation milestone.
- blocks adoption until:
  - daily-archive-owned PageIndex/card/provenance schemas exist
  - no quant-mind runtime dependency is introduced

### graph_readiness_and_no_write_import_boundary

- risk: Parser success could be mistaken for import eligibility unless graph-readiness review remains independent.
- validation gate: Run review artifact post-check before manifest synthesis and require completed review with output_contract_completed=true before any future eligibility promotion.
- blocks adoption until:
  - graph-readiness review post-check passes
  - no-write import rehearsal remains false until explicitly authorized
  - LadybugDB writes remain blocked

## Validation gates for S06

- GROBID TEI adapter contract tests
- GROBID bibliography/citation/header quality evaluation
- OpenDataLoader backend/cache/no-network preflight
- OpenDataLoader OCR/layout/table fidelity evaluation
- Adaptix typed adapter contract coverage
- TreeKnowledge/PageIndex/PaperKnowledgeCard/provenance schema fit review
- EvidencePath source-span and coordinate anchoring tests
- low_quality_source/no_substantive_body refusal preservation
- review packet completion and graph-readiness review post-check
- no-write import rehearsal with all graph/import flags false

## S06 handoff

S06 must turn these risks into a bounded quality plan. It must not execute production integration or authorize graph import.
