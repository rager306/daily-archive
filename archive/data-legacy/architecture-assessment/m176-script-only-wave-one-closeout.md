# M176 Script Only Wave One Closeout

## Verdict

**M176 status: PASS.**

M176 reduced the largest remaining inventory bucket by classifying the first exact script-only wave. It moved 30 reviewed script records and left mixed or unreviewed scripts conservative.

## Categories added

| Category | Count | Exact scope |
|---|---:|---|
| `m061-acquisition-pipeline-output` | 11 | `scripts/m061_anchor_pilot.py`, `scripts/m061_full_5_anchors.py` |
| `figure-extraction-benchmark-output` | 13 | `scripts/m057_marker_extract.py`, `scripts/m058_plotextractor_extract.py`, `scripts/m058_compare_v2_vs_m057.py`, `scripts/m058_marker_compare_5.py` |
| `m028-acquisition-evidence-output` | 6 | `scripts/build_m028_pdf_acquisition_diagnostics.py`, `scripts/build_m028_universal_loader_evidence_bundles.py` |

## No-move groups preserved

- M029 unified source verifier remains `script-only`.
- R024 corpus build, replay, probe, coverage, and quality scripts remain `script-only` for a dedicated wave.
- Architecture audit scripts remain `script-only`.
- Scanner self-output scripts remain `script-only`.
- All unreviewed scripts remain `script-only`.

## Final inventory counts

```text
total_records=341
unknown=0
shared-state=0
script-only=235
m061-acquisition-pipeline-output=11
figure-extraction-benchmark-output=13
m028-acquisition-evidence-output=6
```

## Generated delta highlights

```text
m061-acquisition-pipeline-output +11
figure-extraction-benchmark-output +13
m028-acquisition-evidence-output +6
script-only -30
total delta +0
```

## Tests

```text
uv run pytest tests/test_inventory_write_paths.py -q
13 passed
```

Coverage includes:

- M061 script family positive classification;
- figure extraction script family positive classification;
- M028 acquisition evidence script family positive classification;
- M029 verifier fallback remains `script-only`;
- existing M171-M175 categories and delta renderer behavior.

## Verification

Integrated verification:

```text
focused inventory tests=13 passed
test architecture guard=dynamic=0, legacy=0, violations=0
onion guard=violation_count=0, allowed_violation_count=0
final artifact assertions=PASS
```

Quality stack:

```text
scoped ruff=PASS
pyrefly=0 errors
pre-commit=PASS
GitNexus detect_changes=LOW risk, affected_processes=0
scope hygiene=expected M176 files only
```

## Decisions

- D098: M176 adds exact script-family categories for M061 acquisition pipeline, M057-M058 figure extraction benchmark, and M028 acquisition evidence outputs while preserving mixed and unreviewed scripts as `script-only`.

## Residual risks

1. Pre-edit GitNexus impact remained UNKNOWN because scanner symbols did not resolve authoritatively.
2. `script-only=235` remains large and needs additional waves.
3. R024 scripts are intentionally deferred to a dedicated wave because they mix corpus construction, probes, coverage, and quality metrics.
4. Cache-like, queue-like, and scanner self-output policies remain future work.

## Follow-ups

Recommended next GSD scopes:

1. R024 script inventory wave.
2. Scanner self-output ownership review.
3. Cache policy review for markdown converter outputs.
4. Queue and smoke output ownership review.
5. CI job wiring for generated inventory delta artifacts.

Each future wave should keep the same pattern: baseline, exact source review, scope decision, pre-edit impact, focused positive/fallback tests, generated inventory, generated delta, quality stack.
