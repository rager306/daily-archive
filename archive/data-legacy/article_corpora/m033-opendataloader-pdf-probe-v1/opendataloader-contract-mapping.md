# OpenDataLoader Contract Mapping

## Bounded verdict

`hybrid-sidecar-candidate`

All three selected PDFs processed successfully with the hybrid docling-fast backend and produced JSON/Markdown/HTML/Text outputs without Java-only fallback. The outputs show useful sidecar potential for layout-aware parsing and evidence anchoring, but OCR-on-scanned-PDF and table fidelity remain unproven and no graph-readiness/import claim is made.

## Contract mapping

| Daily-archive boundary | Mapping | Status | Remaining gap |
|---|---|---|---|
| `SourceRef` | Supported as candidate sidecar input: each output can be tied to manifest article_key, source_path, sha256, local PDF provenance, and generated output paths. | `mapped` | Need stable schema for storing OpenDataLoader output references without copying large payloads into KG artifacts. |
| `EvidencePath` | Partially supported: JSON includes page/layout metadata and bounding boxes, so evidence anchors can be derived from output elements plus source PDF hash. | `mapped_with_review_required` | Need deterministic element-id/path convention and review before using as evidence paths. |
| `PageIndex` | Promising sidecar candidate: JSON/Markdown expose headings, page metadata, reading-order text, and layout blocks useful for page/section indexing. | `sidecar_candidate` | Need compare against current PageIndex expectations and decide how tables/figures become nodes. |
| `SemanticChunk` | Candidate input only: Markdown/text can seed chunks, but chunk boundaries must still pass daily-archive chunk/evidence validation and refusal handling. | `candidate_only` | No direct promotion to SemanticChunk without chunk replay and source-span validation. |
| `table artifact` | Partial candidate: table-like signals are present, but no ground-truth table fidelity benchmark was run. | `needs_quality_gate` | Need table-specific benchmark/review before accepting table structures. |
| `refusal diagnostic` | Runtime diagnostics are usable: command, exit code, backend health, fallback usage, cache dependency, stderr/stdout excerpts, and per-paper status are captured. | `mapped` | Need normalize OpenDataLoader runtime errors into daily-archive diagnostic codes if integrated. |
| `graph-readiness packet` | Not satisfied by parser output alone. Outputs can become reviewer packet candidates only after mapping/review. | `not_graph_ready` | Independent graph-readiness review remains required; no import eligibility claim. |

## Operational requirements

- Python 3.13 uv-built wrapper venv: `/root/vendor-source/opendataloader-pdf/python/opendataloader-pdf/.venv-py313-build-check`.
- Hybrid backend: `opendataloader-pdf-hybrid --port 5002`.
- Cache dependency:
  - `docling-project/docling-layout-heron` at `/root/.cache/huggingface/hub/models--docling-project--docling-layout-heron` snapshot `8f39ad3c0b4c58e9c2d2c84a38465abf757272d8` size `171764747` bytes
  - `docling-project/docling-models` at `/root/.cache/huggingface/hub/models--docling-project--docling-models` snapshot `None` size `358236863` bytes

## Safety

- `graph_import_allowed=false`
- `ladybugdb_written=false`
- `production_import_attempted=false`

## Remaining gaps
- No scanned/image-only PDF was included, so OCR quality remains not proven.
- No independent table ground truth was used, so table fidelity is qualitative only.
- Outputs are candidate evidence only and are not graph-ready or import-eligible.
- Need a larger probe before production adoption or sidecar schema commitment.
