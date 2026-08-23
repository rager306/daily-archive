# M025 S07 Evidence Boundary Report

## Scope
- Selection: `m025-rlm-dspy-pageindex-smoke-v1`
- Articles: 5
- Boundary: assets, tables, links, identity, and evidence metadata are separated from chunk text.

## Per-Article Counts
| Article | Assets | Tables | Links | Identity | Diagnostics |
|---|---:|---:|---:|---:|---:|
| `arxiv/cs-ai/2512.24601` | 1 | 1 | 1 | 1 | 0 |
| `arxiv/cs-ai/2605.28617v1` | 1 | 1 | 1 | 1 | 0 |
| `arxiv/cs-cv/2605.26525v1` | 1 | 1 | 1 | 1 | 0 |
| `arxiv/cs-cl/2507.19457` | 1 | 1 | 1 | 1 | 0 |
| `company_blog/cs-ir/pageindex_zhang2025pageindex` | 1 | 1 | 1 | 1 | 0 |

## Aggregate Counts
| Evidence Type | Count |
|---|---:|
| assets | 5 |
| tables | 5 |
| links | 5 |
| identity | 5 |

## Missing or Unsupported Evidence Diagnostics
- None observed in the separated evidence artifacts.

## Provenance Coverage
- Evidence artifacts checked: 20
- Evidence items checked for provenance: 20 / 20
- Each artifact must carry `source_ref` and `chunk_refs`; each evidence item must carry chunk, span, or element provenance.

## Redaction Checks
- Required: True
- Forbidden value findings: 0
- Passed: True

## No-Import / No-Write Safety State
- `metadata_only=true` and `review_only=true`.
- `trusted_kg_import_allowed=false`, `ladybugdb_written=false`, and `production_import_attempted=false`.
- `import_eligible_count=0` and `promoted_to_fact_count=0`.

## Failure Modes
- Local filesystem inputs (`catalog`, `index`, `selection`, `evidence`, `events`) fail with explicit path-bearing `EvidenceReplayError` messages when missing or malformed.
- JSON and JSONL decoding errors bubble as validation failures with the offending path and line where applicable.
- Missing or mismatched no-import flags, event counts, provenance pointers, or redaction checks are accumulated as findings and cause a non-zero verifier exit.
- No network, API, database, or graph-write dependency is used by this report path.

## Load Profile
- The first 10x load pressure point is local filesystem JSON reads across `articles × evidence_types`; report generation is linear and bounded by the explicit selection file.
- Protection: no repository-wide scan, no raw article payload loading, no embeddings/model calls, no network calls, and one bounded JSON parse per selected evidence artifact plus one JSONL event pass.

## Negative Tests
- `tests/test_m025_evidence_replay.py::test_validate_evidence_writes_summary_and_report` covers the positive reporting contract, aggregate counts, provenance, redaction, and fail-closed safety state.
- `tests/test_m025_evidence_replay.py::test_validate_evidence_fails_on_import_flag_violation` covers non-zero/unsafe import flag rejection.
- Existing replay tests cover missing chunking input, empty evidence diagnostics, event emission, and metadata-only separated artifacts.

## Validation Findings
- None. Validation passed.
