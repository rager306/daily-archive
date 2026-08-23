# M176 Script Scope Decision

## Verdict

**Scope status: APPROVED FOR IMPLEMENTATION after impact check.**

M176 wave one adds exact script-family categories for 30 records and leaves mixed script groups as `script-only`.

## Categories to add

| Category | Count | Exact source scope |
|---|---:|---|
| `m061-acquisition-pipeline-output` | 11 | `scripts/m061_anchor_pilot.py`, `scripts/m061_full_5_anchors.py` |
| `figure-extraction-benchmark-output` | 13 | `scripts/m057_marker_extract.py`, `scripts/m058_plotextractor_extract.py`, `scripts/m058_compare_v2_vs_m057.py`, `scripts/m058_marker_compare_5.py` |
| `m028-acquisition-evidence-output` | 6 | `scripts/build_m028_pdf_acquisition_diagnostics.py`, `scripts/build_m028_universal_loader_evidence_bundles.py` |

## No-move groups

- `scripts/verify_m029_unified_source_acquisition.py` remains `script-only`.
- `scripts/audit_test_architecture.py` remains `script-only`.
- R024 corpus build, replay, probe, coverage, and quality scripts remain `script-only` for a dedicated wave.
- `scripts/inventory_write_paths.py` remains `script-only` for scanner self-output review.
- All other scripts remain `script-only` unless explicitly reviewed in a later wave.

## Expected final movement

```text
script-only -30
m061-acquisition-pipeline-output +11
figure-extraction-benchmark-output +13
m028-acquisition-evidence-output +6
```

Expected final counts:

```text
total_records=341
script-only=235
unknown=0
shared-state=0
```

## Implementation rule

Exact script rules must run before the generic `scripts` classification in `_classify`.

## Required tests

- Positive test for `m061-acquisition-pipeline-output`.
- Positive test for `figure-extraction-benchmark-output`.
- Positive test for `m028-acquisition-evidence-output`.
- Fallback test showing an unrelated script remains `script-only`.
- Existing M171-M175 category tests still pass.

## Safety notes

- Do not classify by generic targets such as `path`, `output_path`, `summary_path`, `report_path`, or `decision_path`.
- Do not move M029, R024, architecture audit, or scanner self-output scripts in M176.
- Pre-edit GitNexus impact must be attempted before implementation.
