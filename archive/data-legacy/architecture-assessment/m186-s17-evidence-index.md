# M186 S17 Evidence Index

## Verdict

**S01-S16 are validation-ready as a coherent architecture hardening package.**

## GSD status

At S17 planning time, GSD reported S01-S16 complete and S17-S19 pending. S17 is therefore a closeout-prep wave, not a remediation wave.

## GitNexus planning surfaces

GitNexus planning for S17 highlighted these validation-relevant surfaces:

- `scripts/verify_m031_validation_remediation.py::run`
- `scripts/verify_m031_validation_remediation.py::build_verify_summary`
- `scripts/verify_m025_article_catalog.py` catalog safety/verifier boundary surfaces
- `tests/test_m027_mixed_source_catalog.py::test_m030_requested_ref_intake_closeout_baseline_is_current`
- `tests/test_inventory_write_paths.py::test_m184_canonical_inventory_ratchets_script_only_without_guardrail_regression`
- `scripts/check_project_trajectory.py::build_report`

## Wave index

| Wave | Slices | Outcome | Key artifacts |
|---|---:|---|---|
| Baseline and scope lock | S01-S02 | Established GitNexus baseline and verifier contract baseline before movement. | `m186-gitnexus-baseline.md`, `m186-guardrail-baseline.md`, `m186-verifier-impact-map.md`, `m186-verifier-contract-baseline.md` |
| Verifier primitive extraction | S03-S07 | Extracted/closed validation evidence and catalog safety primitives while preserving wrapper ratchets. | `m186-validation-evidence-path-pilot.md`, `m186-validation-evidence-builder-result.md`, `m186-catalog-safety-contract.md`, `m186-catalog-safety-verification.md`, `m186-verifier-wave-outcomes.md`, `m186-verifier-wave-verification.md` |
| Manifest lifecycle and ratchet wave | S08-S14 | Established lifecycle contract, standalone atomic writer model, no-move residual decisions, and preserve-ratchet closeout. | `m186-manifest-lifecycle-contract.md`, `m186-manifest-atomic-writer-verification.md`, `m186-manifest-ratchet-transition-contract.md`, `m186-m055-manifest-pilot-result.md`, `m186-m055deep-manifest-no-move.md`, `m186-m058-m059-manifest-no-move.md`, `m186-manifest-wave-closeout.md` |
| Catalog drift remediation | S15 | Fixed M027/M030 baseline drift with data-only catalog/index remediation. | `m186-m027-m030-catalog-drift-diagnosis.md`, `m186-m027-m030-catalog-drift-remediation.md`, `m186-m027-m030-catalog-drift-verification.md` |
| Integrated verification | S16 | Proved manifest and catalog waves coexist under preserve-ratchet. | `m186-s16-integration-scope.md`, `m186-s16-integrated-verification.md`, `m186-s16-downstream-readiness.md` |

## Current ratchet state

The active mode remains `preserve-ratchet`. The strict write-path inventory remains:

- `script-only=4`
- `unknown=0`
- `shared-state=0`
- total delta `+0`

## Validation-ready claim

The milestone is ready for a final validation-prep gate set. S17 should not add new source movement; it should only verify, package evidence, and decide whether S18 should run milestone validation.
