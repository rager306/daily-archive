# M186 S16 Integration Scope

## Verdict

**S16 is a verification-only integration wave.** It should confirm that the manifest wave (S08-S14) and catalog drift remediation (S15) coexist under the active architecture ratchets.

## GitNexus planning inputs

GitNexus query for M027/M030 catalog verification surfaced these relevant boundaries:

- `tests/test_m027_mixed_source_catalog.py::test_m030_requested_ref_intake_closeout_baseline_is_current`
- `tests/test_m027_mixed_source_catalog.py::test_m030_requested_ref_intake_rejects_unsafe_claims`
- `scripts/verify_m030_requested_ref_intake.py::validate_catalog_status`
- `scripts/verify_m030_requested_ref_intake.py::main`
- adjacent catalog-backed replay surfaces in M031 selection/replay tests

GitNexus query for architecture guardrails surfaced these governance boundaries:

- `scripts/verify_test_architecture.py::verify_inventory`
- `scripts/verify_test_architecture.py::main`
- `tests/test_test_architecture_guardrail.py`
- `tests/test_inventory_write_paths.py::test_m184_canonical_inventory_ratchets_script_only_without_guardrail_regression`
- `scripts/verify_onion_layering.py::scan_layer`
- `scripts/verify_onion_layering.py::main`

GitNexus `detect_changes(repo=daily-archive, scope=all)` remains **MEDIUM** because it sees accumulated M186 working-tree changes, including earlier verifier wrapper edits. S16 should not expand that scope.

## Scope decision

S16 will not edit functions/classes/methods and will not wire manifest residuals. The integration proof is command evidence and documentation only:

- full M027/M030 catalog verification after S15
- manifest lifecycle and ratchet contract verification after S14
- manifest IO verification for the standalone writer model
- inventory and strict write-path drift checks
- test architecture guard
- onion layering guard
- pyrefly type check
- final GitNexus detect_changes summary

## Explicit non-goals

- No transition from `preserve-ratchet` to `transition-ratchet`.
- No movement of the four manifest residual writers.
- No canonical inventory baseline update.
- No source-code edits unless a fresh blocker proves S16 cannot be completed by verification-only closeout.
