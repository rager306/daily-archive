# M033 S06 T01 Future Probe Scope and Corpus Strategy

- status: `complete`
- derived_from_recommendation: `recommended-bounded-combined-sidecar-architecture`
- not_executed_in_M033: `true`
- candidate_only: `true`
- graph_import_allowed=false
- ladybugdb_written=false
- production_import_attempted=false
- import_eligible=false

## Corpus classes

- **native_digital_arxiv_pdf** (`min:3`): baseline scholarly PDF with text layer and common section/reference structure
- **long_appendix_or_supplement_heavy_pdf** (`min:2`): stress reading order, page indexing, references, and large-document runtime
- **table_heavy_pdf** (`min:2`): evaluate table detection, cell structure, caption linkage, and coordinate anchors
- **figure_or_layout_heavy_pdf** (`min:2`): evaluate layout blocks, figures, captions, and multi-column reading order
- **scanned_or_image_only_control_pdf** (`min:1`): separate OCR capability from normal text-layer extraction and require typed blocker if unsupported
- **known_low_quality_or_metadata_only_control** (`min:1`): verify low_quality_source/no_substantive_body refusals still fire

## Source locality and runtime controls

- All inputs must be repo-local or staged under a documented artifact root with sha256, byte size, and source provenance.
- Default execution mode is no-network; any network requirement must become a typed blocker or explicit preflight finding, not an implicit fetch.
- Model/backend caches must be inventoried before parser execution; missing cache cannot silently download during quality gate.
- model/backend cache preflight is mandatory for GROBID service images and OpenDataLoader/Docling model caches.
- No API keys are required for the planned parser-quality probe; quant-mind runtime remains excluded.

## Excluded production actions

- production parser integration
- dependency adoption into runtime path
- graph import
- LadybugDB write
- positive import eligibility claim
- OpenAI/API quant-mind runtime
- unreviewed parser-to-SemanticChunk promotion
