# M133 Debt Recon

Schema: `daily-archive-m133-debt-recon.v1`

## Counts

| Metric | Count |
|---|---:|
| Total test files | 269 |
| Guardrail violations | 0 |
| Allowlisted dynamic script import | 51 |
| Allowlisted legacy mixed | 65 |
| Strict script wrapper | 7 |
| Unknown bucket | 81 |
| Missing-import suppressions | 263 |

## Missing-import suppression categories

| Category | Count |
|---|---:|
| `optional_dependency_or_stub_gap` | 11 |
| `other_missing_import` | 209 |
| `scripts_import` | 43 |

## Dynamic candidate sample

| Path | Bucket | Script refs | Loader tokens | Lines |
|---|---|---|---:|---:|
| `tests/test_article_baseline_recovery_replay.py` | `legacy-mixed` | - | 6 | 365 |
| `tests/test_article_preprocessing_replay_contract.py` | `legacy-mixed` | - | 3 | 225 |
| `tests/test_bounded_chunk_repair.py` | `legacy-mixed` | `render_bounded_repair_prototype.py`, `verify_bounded_repair_prototype.py` | 3 | 392 |
| `tests/test_codebase_memory_governance.py` | `legacy-mixed` | - | 3 | 167 |
| `tests/test_dspy_extraction_boundary.py` | `legacy-mixed` | - | 3 | 302 |
| `tests/test_m024_validation_evidence_closure.py` | `legacy-mixed` | - | 3 | 209 |
| `tests/test_m025_boundary_replay_completion.py` | `legacy-mixed` | - | 3 | 303 |
| `tests/test_m025_evidence_replay.py` | `legacy-mixed` | - | 3 | 238 |
| `tests/test_m025_requirement_scope_reconciliation.py` | `legacy-mixed` | - | 3 | 295 |
| `tests/test_m026_requirement_scope_reconciliation.py` | `legacy-mixed` | - | 3 | 427 |
| `tests/test_m027_current_pipeline_baseline.py` | `legacy-mixed` | - | 6 | 378 |
| `tests/test_m027_end_to_end_mixed_replay.py` | `legacy-mixed` | - | 6 | 533 |

## Small unknown candidates

| Path | Imports | Lines |
|---|---|---:|
| `tests/test_validation_batch_cli_contract.py` | `__future__`, `json`, `subprocess` | 64 |
| `tests/test_r024_10_document_corpus_selection.py` | `__future__`, `json`, `pathlib`, `subprocess`, `sys` | 68 |
| `tests/test_cli_contract.py` | `subprocess` | 69 |
| `tests/test_r024_20_document_quality_metrics.py` | `__future__`, `json`, `pathlib`, `pytest`, `typing` | 88 |
| `tests/test_r024_quality_metrics.py` | `__future__`, `json`, `pathlib`, `pytest`, `typing` | 89 |
| `tests/test_m055deep_report_s06.py` | `__future__`, `pathlib`, `render_m055deep_report`, `sys` | 91 |
| `tests/test_r024_20_document_parser_chunking.py` | `__future__`, `json`, `pathlib`, `pytest`, `typing` | 92 |
| `tests/test_m055_benchmark_s05.py` | `__future__`, `pathlib`, `render_m055_report`, `sys` | 93 |
| `tests/test_r024_parser_chunking.py` | `__future__`, `json`, `pathlib`, `pytest` | 93 |
| `tests/test_r024_entity_quality_metrics.py` | `__future__`, `json`, `pathlib`, `pytest` | 94 |
| `tests/test_r024_53_document_quality_metrics.py` | `__future__`, `json`, `pathlib`, `pytest`, `typing` | 96 |
| `tests/test_m056_wave_1.py` | `__future__`, `json`, `pathlib` | 103 |
