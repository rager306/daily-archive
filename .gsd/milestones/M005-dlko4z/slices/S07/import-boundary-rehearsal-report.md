# S07 Negative Import Boundary Rehearsal Report

## Summary

S07 re-scoped the original positive isolated import rehearsal into a negative import-boundary rehearsal because S06 independently proved that all current benchmarked chunks/candidates remain import-ineligible. The rehearsal consumes only redacted S06 benchmark artifacts and emits import-boundary evidence showing that current candidates are rejected before any trusted KG import path.

The rehearsal result is a safety proof, not an import-readiness proof: current candidates are blocked consistently, no production KG writes are attempted, and diagnostics identify the refusal reasons that must be remediated before any future positive import rehearsal.

## Result

| Metric | Value |
|---|---:|
| Candidate count | 2,471 |
| Accepted imports | 0 |
| Rejected candidates | 2,471 |
| Import-eligible candidates | 0 |
| Benchmark methods represented | 3 |
| Recommendation | `positive_import_blocked` |

## Refusal Counts

| Reason | Count |
|---|---:|
| `retrieval_only_not_import_ready` | 1,167 |
| `baseline_retrieval_only_not_import_ready` | 345 |
| `estimated_candidate_requires_review` | 295 |
| `claim_route_requires_review` | 245 |
| `equation_route_not_import_ready` | 146 |
| `method_route_requires_review` | 136 |
| `figure_route_not_import_ready` | 86 |
| `table_route_requires_review` | 38 |
| `citation_route_requires_review` | 11 |
| `administrative_metadata_requires_review` | 2 |

## Safety Boundary

All safety flags remained closed in the rehearsal summary:

- `raw_text_included=false`
- `chunk_text_included=false`
- `raw_binary_included=false`
- `base64_included=false`
- `embeddings_included=false`
- `vectors_included=false`
- `secrets_included=false`
- `optimizer_traces_included=false`
- `ladybugdb_written=false`
- `production_import_attempted=false`

The diagnostics JSONL contains redacted candidate identities, method ids, package ids, refusal reasons, and remediation hints only. It does not include paper text, chunk text, image/PDF payloads, embeddings, vectors, secrets, or optimizer traces.

## What This Proves

- The current import boundary can consume package-shaped benchmark evidence and reject all current ineligible candidates.
- Current M005 artifacts do not accidentally authorize `trusted_kg_import`.
- No production LadybugDB writes are attempted during the rehearsal.
- The no-write/no-import boundary is observable through summary counts and candidate diagnostics.
- The blocking reasons are explicit enough to drive future remediation work.

## What This Does Not Prove

- Positive trusted KG import readiness.
- Entity or relation extraction quality.
- Semantic/vector retrieval quality.
- Multimodal extraction or multimodal retrieval readiness.
- Real external chunker performance; S06 still marks real-library candidates as not executed.
- Production LadybugDB persistence safety beyond the negative no-write rehearsal.

## Caveats

- `dry_run_only`
- `real_library_candidates_not_executed`
- `production_import_blocked`
- `missing_original_pdf:16`

The `missing_original_pdf:16` count is aggregate across compared S06 methods, not a new source-preservation finding. S05 remains the authoritative source-preservation slice and recorded 8 missing original PDFs in current source paths.

## Remediation Required Before Positive Import

A future positive import slice must create a small, independently reviewed import-eligible subset. Minimum prerequisites:

1. Select candidate chunks with complete source spans, annotations, and source/asset linkage.
2. Perform route-specific review proving the candidate is suitable for trusted KG import.
3. Convert candidate state from repair/retrieval-only into an explicitly reviewed import-eligible state.
4. Preserve redaction and no-payload rules in all machine artifacts.
5. Run an isolated positive import rehearsal against that subset only.
6. Require independent review before any production LadybugDB write path is considered.

## Evidence Files

- `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-summary.json`
- `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-diagnostics.jsonl`
- `.gsd/milestones/M005-dlko4z/slices/S07/run-evidence/import-boundary-review-summary.md`
- `src/arxiv_archive/import_boundary_rehearsal.py`
- `tests/test_import_boundary_rehearsal.py`
