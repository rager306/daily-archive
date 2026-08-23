# M180 Residual Verify Candidates

## Baseline

```text
script-only=142
verify_script_only=40
verify_exact_paths=23
verify_families=9
```

## Families

| Family | Count | Exact source paths |
|---|---:|---|
| `verify_m031` | 10 | `scripts/verify_m031_s05_closeout.py` (3)<br>`scripts/verify_m031_validation_remediation.py` (3)<br>`scripts/verify_m031_process_continuity_audit.py` (2)<br>`scripts/verify_m031_chunk_evidence_replay.py` (1)<br>`scripts/verify_m031_parser_conversion_replay.py` (1) |
| `verify_m033` | 10 | `scripts/verify_m033_combined_parser_architecture.py` (2)<br>`scripts/verify_m033_external_parser_quality_plan.py` (2)<br>`scripts/verify_m033_grobid_probe.py` (2)<br>`scripts/verify_m033_opendataloader_adaptix_adapter.py` (2)<br>`scripts/verify_m033_quantmind_pattern_study.py` (2) |
| `verify_m029` | 8 | `scripts/verify_m029_unified_source_acquisition.py` (4)<br>`scripts/verify_m029_post_validation_remediation.py` (1)<br>`scripts/verify_m029_unified_conversion_quality_boundary.py` (1)<br>`scripts/verify_m029_unified_readiness.py` (1)<br>`scripts/verify_m029_validation_remediation.py` (1) |
| `verify_m027` | 4 | `scripts/verify_m027_source_acquisition_boundary.py` (3)<br>`scripts/verify_m027_mixed_source_catalog.py` (1) |
| `verify_m023` | 2 | `scripts/verify_m023_artifact_scaffold_gate.py` (2) |
| `verify_m025` | 2 | `scripts/verify_m025_article_catalog.py` (1)<br>`scripts/verify_m025_baseline_recovery_outputs.py` (1) |
| `verify_test_architecture` | 2 | `scripts/verify_test_architecture.py` (2) |
| `verify_article_catalog` | 1 | `scripts/verify_article_catalog.py` (1) |
| `verify_m022` | 1 | `scripts/verify_m022_final_gate.py` (1) |

## Candidate notes

- Family grouping is for review only; implementation must match exact source paths.
- Prefer cohesive verification families with enough records to justify tests.
- Reject broad `verify_m*` or target-name based rules.
