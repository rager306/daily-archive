# M187 Transition Verification Plan

## Verdict

**M187 may retire the four manifest residuals only through staged transition-ratchet evidence.**

## Starting inventory state

Current inventory summary before source movement:

- `script-only=4`
- `unknown=0`
- `shared-state=0`
- total records: 341

## Target inventory delta

| Phase | Intended residual state | Expected script-only state | Baseline policy |
|---|---|---:|---|
| Start of S01 | four manifest residuals script-local | 4 | Existing canonical baseline remains valid. |
| After S02 | M055 and M055deep retired if behavior passes | 2 | Drift from old baseline is expected and must be explained, not hidden. |
| After S03 | M058 and M059 retired if behavior passes | 0 | Drift from old baseline is expected and must be explained, not hidden. |
| S04 | Canonical baseline updated after proof | 0 | Baseline update allowed only for intended residual retirement delta. |
| S05 | Final validation | 0 | Strict drift must pass against updated baseline. |

## S02 movement gates

Before edits:

- re-run exact GitNexus impact for `build_corpus_manifest`,
- re-run exact GitNexus impact for `write_manifest`,
- inspect current implementation and tests,
- identify rollback boundary.

After edits:

- focused M055 corpus manifest tests pass,
- focused M055deep tests pass,
- manifest contract tests pass,
- inventory delta explains transition from `script-only=4` to target `script-only=2`,
- `unknown=0`, `shared-state=0`.

## S03 movement gates

Before edits:

- re-run exact GitNexus impact for `write_json`,
- re-run exact GitNexus impact for `finalize_manifest`,
- review M059 six direct builder callers,
- identify rollback boundary.

After edits:

- focused M058 tests pass,
- focused M059 tests pass,
- manifest contract tests pass,
- inventory delta explains transition to target `script-only=0`,
- `unknown=0`, `shared-state=0`.

## S04 baseline gates

- Update canonical inventory baseline only after S02/S03 behavior proof.
- Document old vs new strict inventory counts.
- Run inventory tests.
- Run strict drift against updated baseline.
- Run onion layering, test architecture, catalog/manifest, pyrefly, and GitNexus detect_changes.

## S05 final gates

- Run final representative gate set.
- Record GSD milestone validation PASS only if gates pass.
- Complete milestone only after all slices complete.

## Forbidden shortcuts

- No broad write-path classification rules such as generic `path`, `output_path`, `json_path`, `manifest`, or `cache`.
- No canonical baseline update before behavior proof.
- No unscoped replacement of arbitrary JSON helpers.
- No parser/chunk/graph readiness claims.
- No `.gsd/*` commit requirement.
