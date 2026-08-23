# M031 Process Continuity Audit

Final S05 continuity checkpoint for M031. This is a metadata-only, no-write audit that preserves fail-closed graph import and LadybugDB boundaries.

## Stage Owners, Evidence, Verifiers, and Failure Modes

| Stage | Owner | Evidence | Verifiers | Failure Modes |
|---|---|---|---|---|
| `url_intake` | S01/S02 catalog-backed selection | `data/article_corpora/m031-catalog-backed-replay-v1/selection.json` | `scripts/verify_m031_catalog_backed_replay.py` | missing requested ref; typed catalog blocker suppressed; network fetch attempted |
| `article_catalog` | article catalog index lookup | `data/article_corpora/m031-catalog-backed-replay-v1/selection.json`; `data/article_corpora/m031-catalog-backed-replay-v1/replay-closeout-summary.json` | `scripts/verify_m031_catalog_backed_replay.py` | catalog JSON absent; index path drift; placeholder treated as cataloged |
| `source_acquisition` | S02 local source acquisition replay | `data/article_corpora/m031-catalog-backed-replay-v1/source-acquisition-summary.json` | `scripts/verify_m031_catalog_backed_replay.py` | missing local source path; hash/size drift; unexpected network fetch |
| `loader_evidence` | S02 loader evidence replay | `data/article_corpora/m031-catalog-backed-replay-v1/loader-evidence-summary.json` | `scripts/verify_m031_catalog_backed_replay.py` | loader row absent; metadata-only PDF treated as parsed text; raw payload embedded |
| `parser_conversion` | S03 parser conversion replay | `data/article_corpora/m031-catalog-backed-replay-v1/conversion-quality/conversion-quality-summary.json`; `data/article_corpora/m031-catalog-backed-replay-v1/parser-conversion-closeout-summary.json` | `scripts/verify_m031_parser_conversion_replay.py` | parser-ready count drift; low-quality HTML promoted; permissive graph flag |
| `chunking` | S04 chunk evidence replay | `data/article_corpora/m031-catalog-backed-replay-v1/chunk-evidence/chunk-evidence-summary.json`; `data/article_corpora/m031-catalog-backed-replay-v1/chunk-evidence-closeout-summary.json` | `scripts/verify_m031_chunk_evidence_replay.py` | missing zero-chunk refusal; chunk text leaked; import-eligible chunks claimed |
| `graph_readiness_review` | S04 independent review handoff | `data/article_corpora/m031-catalog-backed-replay-v1/chunk-evidence/independent-review-events.jsonl`; `data/article_corpora/m031-catalog-backed-replay-v1/graph-readiness-review/independent-review-summary.md`; `data/article_corpora/m031-catalog-backed-replay-v1/graph-readiness-review/arxiv_cs-cl_2507.19457_arxiv_pdf-review.md` | `arxiv_archive.graph_readiness_review validate-only`; `scripts/verify_m031_process_continuity_audit.py` | completed review claimed without verdict; review placeholders accepted; structural route label treated as semantic approval |
| `graph_import_boundary` | S05 refusal-only import boundary rehearsal | `data/article_corpora/m031-catalog-backed-replay-v1/import-boundary-rehearsal/import-boundary-summary.json`; `data/article_corpora/m031-catalog-backed-replay-v1/import-boundary-rehearsal/import-boundary-diagnostics.jsonl`; `data/article_corpora/m031-catalog-backed-replay-v1/import-boundary-rehearsal/import-boundary-report.md` | `scripts/replay_m031_import_boundary_rehearsal.py`; `scripts/verify_m031_process_continuity_audit.py` | missing refusal artifact; accepted/import-eligible count above zero; LadybugDB write flag true |

## Unsafe Claims to Preserve

- Do not claim parser readiness for low-quality, metadata-only, blocked, or placeholder-pruned rows.
- Do not claim chunk readiness for zero-chunk refusal rows.
- Do not treat ok_for_graph or trusted_graph labels as semantic graph approval before independent review verdict evidence exists.
- Do not enable graph_import_allowed, trusted_kg_import_allowed, production_import_attempted, graph_write_attempted, production_persistence_attempted, or ladybugdb_written.
- Do not embed raw article text, chunk text, PDF bytes, HTML, embeddings, vectors, secrets, or optimizer traces in checkpoint artifacts.

## Fail-Closed Flags

- graph_import_allowed=false
- trusted_kg_import_allowed=false
- production_import_attempted=false
- graph_write_attempted=false
- production_persistence_attempted=false
- ladybugdb_written=false
- raw_text_included=false; chunk_text_included=false; embeddings_included=false; vectors_included=false

## Structural Route Label Notice

`ok_for_graph` and `trusted_graph` route labels are structural states only while independent semantic review is incomplete. They are not graph import approval, trusted KG approval, or LadybugDB write authorization.

## Failure Modes

- Filesystem: missing JSON/JSONL/Markdown artifacts fail with M031_CONTINUITY_INPUT_MISSING before writes.
- Malformed artifacts: invalid JSON/JSONL fail with M031_CONTINUITY_INVALID_JSON or M031_CONTINUITY_INVALID_JSONL.
- Stale counts or rows: missing seven-row/stage evidence fails with M031_CONTINUITY_ROW_COUNT or M031_CONTINUITY_STAGE_EVIDENCE.
- Permissive graph/import/LadybugDB flags fail with M031_UNSAFE_FAIL_CLOSED_FLAG or M031_IMPORT_BOUNDARY_PERMISSIVE.
- Completed-review claims without PASS/FLAG/REPAIR/BLOCKER verdict evidence fail with M031_COMPLETED_REVIEW_WITHOUT_VERDICT.

## Load Profile

- Expected load: 7 progression rows, 8 stages per row, 8 source/checkpoint artifacts, 7 import refusal diagnostics
- 10x breakpoint: local JSON/Markdown serialization and recursive metadata scanning saturate first at about 70 rows; there is no network, subprocess, model, graph, or LadybugDB runtime path
- Protection: single-pass row joins by deterministic keys, bounded local files, no raw payload reads, no remote calls, no background processes, no database writes

## Negative Tests

- missing stage evidence
- missing progression row
- unsafe permissive flags
- raw payload leakage
- missing import-boundary refusal artifacts
- completed-review claim without verdict evidence

## Import Boundary Checkpoint

The import-boundary rehearsal has seven deterministic `M031_IMPORT_BOUNDARY_REFUSED` diagnostics, zero accepted candidates, zero import-eligible candidates, and no LadybugDB writes.
