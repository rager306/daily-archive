# M005/S03 Structure-Aware Implementation Report

## Verdict

S03 implemented deterministic structure-aware chunk construction and redacted gold-corpus validation evidence, but it remains **not KG-import-ready**.

The implementation improves the S02 baseline by replacing generic retrieval-only section chunks with structural elements and route-aware chunks using canonical normalized-Markdown source spans. It does not grant `trusted_kg_import`, does not write LadybugDB data, and does not approve production KG import or corpus scaling.

## Evidence Inputs

- S02 baseline report: `.gsd/milestones/M005-dlko4z/slices/S02/baseline-chunk-quality-report.md`
- Structure-aware run summary: `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-summary.json`
- Structure-aware package diagnostics: `.gsd/milestones/M005-dlko4z/slices/S03/run-evidence/structure-aware-package-diagnostics.jsonl`
- Implementation: `src/arxiv_archive/structure_aware_chunking.py`
- Tests: `tests/test_structure_aware_chunking.py`

## Comparison to S02 Baseline

| Metric | S02 baseline | S03 structure-aware dry run |
|---|---:|---:|
| Gold-corpus papers | 10 | 10 |
| Valid packages | 10 | 10 |
| Chunk count | 345 | 1,831 |
| Import-ready packages | 0 | 0 |
| Import-eligible chunks | 0 | 0 |
| Refused chunks | 345 | 1,831 |
| Raw text in machine artifacts | false | false |
| Embeddings in machine artifacts | false | false |
| Production import attempted | false | false |
| LadybugDB written | false | false |

The increase from 345 to 1,831 chunks is expected: S03 emits structural element-level chunks with route labels, not the previous `PageIndex → SemanticChunk` section-ish retrieval chunks.

## Structure-Aware Distributions

### Chunk types

| Chunk type | Count |
|---|---:|
| `retrieval_context` | 1,167 |
| `claim_candidate` | 245 |
| `equation_context` | 146 |
| `method_candidate` | 136 |
| `figure_caption_context` | 86 |
| `table_context` | 38 |
| `reference_entry` | 11 |
| `metadata` | 2 |

### Routes

| Route | Count |
|---|---:|
| `retrieval_only` | 1,399 |
| `claim_extraction` | 245 |
| `method_extraction` | 136 |
| `table_extraction` | 38 |
| `citation_graph` | 11 |
| `metadata_graph` | 2 |

### States

| State | Count |
|---|---:|
| `ok_for_retrieval_only` | 1,167 |
| `repair_required` | 664 |

### Refusal reasons

| Refusal reason | Count |
|---|---:|
| `retrieval_only_not_import_ready` | 1,167 |
| `claim_route_requires_review` | 245 |
| `equation_route_not_import_ready` | 146 |
| `method_route_requires_review` | 136 |
| `figure_route_not_import_ready` | 86 |
| `table_route_requires_review` | 38 |
| `citation_route_requires_review` | 11 |
| `administrative_metadata_requires_review` | 2 |

## What Improved

1. **Canonical source spans:** S03 emits `normalized_markdown` character spans instead of S02's `semantic_chunk_text` spans.
2. **Structural hierarchy:** Packages now include document, section, paragraph, table, figure-caption, equation, reference, and administrative elements with parent-child links.
3. **Route observability:** Chunks are now classified into claim, method, table, citation, metadata, equation, figure, and retrieval-only routes.
4. **Refusal observability:** Every non-importable chunk has an explicit refusal reason, avoiding count-only evidence.
5. **Redacted dry-run evidence:** Summary and JSONL diagnostics are machine-readable and exclude raw paper text, chunk text, embeddings, vectors, secrets, production import attempts, and LadybugDB writes.

## Known Limitations

- No chunk is import-eligible yet; this is a structure-aware dry run, not a production import rehearsal.
- Route assignment is deterministic and heuristic; it needs S04 annotation sidecars and S05 independent benchmark review before any import-readiness decision.
- Evidence paths are not yet populated for graph-ready chunks because no chunk is graph-ready in S03.
- Figure and equation contexts are observable, but no dedicated graph import route is approved for them.
- Claim/method/table/citation/metadata routes are marked `repair_required`; they are candidates for review, not trusted facts.

## Explicit Non-Claims

This report does **not** claim:

- chunks are safe for trusted KG import;
- claim, entity, relation, table, citation, or metadata extraction is production-ready;
- semantic/vector retrieval has been validated;
- broad corpus scaling is approved;
- LadybugDB persistence is safe;
- deterministic route labels are KG facts;
- S04/S05/S06 gates can be skipped.

## Go / No-Go

- **Go for S04:** add deterministic annotation sidecars over these structured chunks.
- **Go for S05 later:** benchmark S03 structure-aware output against S02 baseline with independent review.
- **No-go for production KG import:** keep trusted fact persistence, production LadybugDB writes, and corpus scaling blocked.
- **No-go for import-readiness claims:** all 1,831 chunks remain refused/import-ineligible in S03 evidence.

## Verification

Fresh verification for T04 passed:

```text
uv run pytest tests/test_structure_aware_chunking.py tests/test_chunk_import_contract.py -q
uv run ruff check src/arxiv_archive/structure_aware_chunking.py tests/test_structure_aware_chunking.py
uv run python -m arxiv_archive.structure_aware_chunking --manifest .gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json --output-dir .gsd/milestones/M005-dlko4z/slices/S03/run-evidence
```

Results:

- 29 tests passed.
- Ruff reported all checks passed.
- Gold-corpus dry run produced 10 valid packages, 1,831 chunks, 0 import-ready packages, and 0 import-eligible chunks.
