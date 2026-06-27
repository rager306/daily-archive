# M185 Candidate Inventory

## Wrapper extraction candidates

| Candidate | Source | Current tests or evidence | Initial risk | Notes |
|---|---|---|---|---|
| Test architecture audit helper | `scripts/audit_test_architecture.py` | `tests/test_test_architecture_guardrail.py`, architecture guard command | Medium | Small helpers like `write_outputs` and `render_markdown`; must avoid introducing application->script coupling. |
| Pipeline script audit helper | `scripts/audit_pipeline_scripts.py` | `tests/test_pipeline_script_audit.py`, existing `src/research_graph/application/pipeline_script_inventory.py` | Medium | Already imports application inventory types; likely a consolidation seam rather than new abstraction. |
| M025 article catalog verifier boundary | `scripts/verify_m025_article_catalog.py` | `tests/test_m025_article_catalog_verifier.py`, catalog safety tests | Medium-high | Many local helper functions; extract only tiny pure helpers if impact allows. |
| Validation evidence helper | `scripts/verify_m031_validation_remediation.py` | validation remediation tests and GitNexus process result | Medium-high | Large script; only a small path/json/text helper should be considered. |

## Manifest/cache residuals

| Residual | Target | Line | Initial decision |
|---|---|---:|---|
| `scripts/benchmark_m055_corpus_manifest.py` | `output_path` | 118 | no-move until lifecycle proof |
| `scripts/build_m055deep_corpus_manifest_20.py` | `output_path` | 224 | no-move until lifecycle proof |
| `scripts/m058_build_graph_manifest.py` | `path` | 53 | no-move until lifecycle proof |
| `scripts/m059_build_manifest.py` | `actual_output` | 179 | no-move until lifecycle proof |

Lifecycle proof means owner, invalidation, consumer, concurrency, and lifecycle evidence.

## Dependency groups

1. Baseline contracts before extraction: wrapper tests and architecture guard checks.
2. Low-risk wrapper pilots: audit/test and pipeline audit before large verifier scripts.
3. Higher-risk verifier probes: M025 and M031 only after baseline contracts are green.
4. Manifest/cache probes: review-only unless all lifecycle proof exists.
5. Final verification: strict drift, guards, quality stack, and GitNexus detect_changes.

## No-move constraints

- No broad scanner categories.
- No generic path/target/output classification.
- No manifest/cache movement without full proof.
- No dynamic script import in tests.
- No new `Protocol` or factory for a single implementation.
