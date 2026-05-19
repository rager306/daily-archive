# M005/S02 Baseline Chunk Quality Report

## Verdict

The current `PageIndex → SemanticChunk` baseline is **not import-ready** for the scientific KG.

It is useful as a retrieval-only baseline and as a comparison point for S03, but it does not satisfy the S01 import-ready chunk contract for graph import, claim extraction, entity extraction, relation extraction, table extraction, citation graph construction, or metadata graph construction.

## Evidence Inputs

- Baseline run summary: `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-summary.json`
- Package diagnostics: `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/baseline-package-diagnostics.jsonl`
- Human review samples: `.gsd/milestones/M005-dlko4z/slices/S02/review/baseline-review-samples.md`
- Redacted review sample index: `.gsd/milestones/M005-dlko4z/slices/S02/run-evidence/review-sample-index.json`
- Contract: `.gsd/milestones/M005-dlko4z/slices/S01/import-ready-chunk-contract.md`

## Aggregate Baseline Result

| Metric | Value |
|---|---:|
| Gold-corpus papers measured | 10 |
| Valid package count | 10 |
| Current chunks measured | 345 |
| Import-ready packages | 0 |
| Import-eligible chunks | 0 |
| Refused chunks | 345 |

Distribution:

| Dimension | Count |
|---|---:|
| `route=retrieval_only` | 345 |
| `state=ok_for_retrieval_only` | 345 |
| `chunk_type=retrieval_context` | 345 |
| `refusal=baseline_retrieval_only_not_import_ready` | 345 |

Safety flags:

| Flag | Value |
|---|---|
| Raw text in machine diagnostics | `false` |
| Embeddings included | `false` |
| Production import attempted | `false` |
| LadybugDB written | `false` |

## Inner Review Sample Coverage

The six-paper inner review minimum produced bounded markdown samples for all required papers:

| Paper | Samples | Status |
|---|---:|---|
| `2605.14259v1` | 2 | sampled |
| `2605.14517v1` | 2 | sampled |
| `2605.14995v1` | 2 | sampled |
| `2605.14743v1` | 2 | sampled |
| `2605.14799v1` | 2 | sampled |
| `2605.14291v1` | 2 | sampled |

The markdown artifact intentionally includes bounded snippets for human review. The machine index remains redacted with `raw_text_in_machine_logs=false`.

## Quality Findings

### 1. The baseline has section-ish retrieval context, not graph-grade provenance

Current chunks preserve a route and stable chunk identifier, but their source span coordinate space is `semantic_chunk_text`. That makes them unsuitable as canonical graph evidence spans. S01 requires import-ready source spans to point back to canonical normalized Markdown or an explicitly named coordinate space with sufficient lineage for review.

### 2. Every chunk is conservatively refused for KG import

All 345 chunks are marked:

- `route=retrieval_only`
- `state=ok_for_retrieval_only`
- `chunk_type=retrieval_context`
- refusal reason `baseline_retrieval_only_not_import_ready`

This is the correct baseline behavior. It prevents accidental promotion from retrieval context into KG facts.

### 3. The six-paper review samples expose the next chunking priorities

The bounded samples show several expected S03 design pressures:

- repaired conversion cases (`2605.14259v1`, `2605.14517v1`) need lineage that distinguishes abstract, authorship/front-matter, related work, and scientific body sections;
- theory/math papers (`2605.14743v1`) need definition/equation-aware boundaries;
- multimodal/security papers (`2605.14799v1`, `2605.14291v1`) need table, figure, numeric-result, and cross-modal dependency flags;
- prose/method-result papers (`2605.14995v1`) need route-aware segmentation before claim extraction.

### 4. No missing full-text blockers appeared in this baseline run

The run measured all 10 packages as valid baseline packages. Missing-artifact blockers remain supported by the validator and measurement code, but this specific gold-corpus run did not surface a missing artifact.

## Explicit Non-Claims

This report does **not** claim:

- improved chunking has been implemented;
- current chunks are KG-import-ready;
- current chunks are safe for claim extraction, entity extraction, relation extraction, table extraction, citation graph construction, or metadata graph construction;
- semantic/vector retrieval has been validated;
- production LadybugDB persistence is safe;
- broad corpus scaling is approved;
- bounded markdown snippets are machine-ingestion artifacts.

## Go / No-Go

- **Go for S03:** implement deterministic structure-aware chunking against this baseline.
- **No-go for KG import:** keep production import, trusted fact persistence, and broader corpus scaling blocked.
- **No-go for import-readiness claims:** use this report only as baseline measurement evidence.

## S03 Priorities

1. Replace retrieval-only chunk boundaries with structure-aware boundaries: paper → sections → paragraphs / tables / figures / equations / references.
2. Emit canonical source spans against normalized Markdown rather than `semantic_chunk_text` spans.
3. Preserve parent-child lineage and route compatibility per chunk.
4. Add route-specific states for claim/method/entity/relation/table/citation/metadata handling.
5. Keep deterministic annotations as weak sidecars; do not promote them to KG facts.
6. Re-run this same baseline/report path after S03 so deltas are measurable.
