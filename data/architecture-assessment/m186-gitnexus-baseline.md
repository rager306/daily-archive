# M186 GitNexus Baseline

## Verdict

**Baseline ready for architecture crystallization.**

## GitNexus planning evidence

### Verifier safety boundary

Query: `verify_m025_article_catalog verifier helpers safe_catalog_path check_safety_flags article_ref_from_path`

Observed relevant flow:

- `proc_6_main`: `Main -> Normalize_posix_path`, with `scripts/verify_m025_article_catalog.py:validate_index` participating.
- Related definitions include `article_entry_from_record` and M025/M029/M031 verifier tests.

Planning implication: M025 movement remains safety-sensitive. Any edit must begin with `gitnexus_impact` on the exact symbol and focused M025 tests.

### Validation evidence boundary

Query: `verify_m031_validation_remediation build_evidence _safe_output_path _repo_relative_path _json_path`

Observed relevant flow:

- `proc_59_run`: `Run -> _json_path`, connected to validation remediation scripts.
- Definitions include `scripts/verify_m031_validation_remediation.py`, `build_runtime_diagnostics`, and focused tests in `tests/test_m031_validation_remediation.py`.

Planning implication: M031 path/evidence helpers are plausible extraction candidates only after contract tests pin traversal rejection, repo-relative normalization, evidence shape, and diagnostics.

### Manifest lifecycle boundary

Query: `manifest lifecycle benchmark_m055 build_m055deep m058_build_graph_manifest m059_build_manifest write path script-only`

Observed relevant definitions:

- `tests/test_inventory_write_paths.py::test_m183_benchmark_m055deep_outputs_get_precise_category`
- `tests/test_inventory_write_paths.py::test_m184_graph_connectivity_probe_outputs_get_precise_category_without_manifest`
- `tests/test_inventory_write_paths.py::test_m184_remaining_residual_outputs_get_precise_categories_without_manifests`
- `scripts/m059_build_manifest.py::build_m055deep`

Planning implication: manifest/cache movement must start with lifecycle ownership and atomicity proof, not with broad write-path classifier changes.

### Wrapper patterns

Query: `audit_test_architecture audit_pipeline_scripts wrapper contract test architecture inventory pipeline audit inventory`

Observed relevant definitions:

- `tests/test_pipeline_script_audit.py`
- `tests/test_pipeline_script_wrapper_contracts.py`
- `tests/test_inventory_write_paths.py::test_m184_canonical_inventory_ratchets_script_only_without_guardrail_regression`
- `scripts/verify_test_architecture.py::verify_inventory`
- `scripts/audit_test_architecture.py`
- `scripts/audit_pipeline_scripts.py`

Planning implication: M186 should reuse the M185 extraction pattern: application module owns logic, script remains CLI wrapper, tests assert wrapper contract.

## Current GitNexus change risk

`gitnexus_detect_changes(scope=all, repo=daily-archive)` reported:

```text
changed_files=16
risk_level=low
changed_symbols=[]
affected_processes=[]
```

This includes local `.gsd` state and should not be treated as commit scope. Non-GSD source edits still require symbol-specific impact analysis before modification.

## Candidate order

1. Contract-first verifier tests.
2. M031 path primitive pilot.
3. M025 safety primitive pilot.
4. Manifest lifecycle contract.
5. Atomic manifest writer model.
6. Individual manifest pilots.
7. Ratchet expansion and integrated guard hardening.

## Non-negotiables

- No broad target/path/output/cache/manifest classification rules.
- No direct extractor-to-graph write.
- No weakening fail-closed verifier behavior.
- No `.gsd/*` commit.
