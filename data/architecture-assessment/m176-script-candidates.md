# M176 Script Candidate Review

## Verdict

**Candidate review status: PASS.** Wave one can safely move 30 exact script records across three reviewed families. Mixed verifier scripts remain `script-only`.

## Reviewed candidate records

### M061 acquisition pipeline scripts

Proposed category:

```text
m061-acquisition-pipeline-output=11
```

Exact files:

- `scripts/m061_anchor_pilot.py` x6
- `scripts/m061_full_5_anchors.py` x5

Reviewed writes include pipeline summaries, decision docs, acquired PDFs/eprints, TEI/OpenDataLoader artifacts, and anchor outputs. These are tightly scoped to M061 acquisition and graph-readiness pilot artifacts.

### M057 and M058 figure extraction scripts

Proposed category:

```text
figure-extraction-benchmark-output=13
```

Exact files:

- `scripts/m057_marker_extract.py` x3
- `scripts/m058_plotextractor_extract.py` x4
- `scripts/m058_compare_v2_vs_m057.py` x3
- `scripts/m058_marker_compare_5.py` x3

Reviewed writes include per-PDF extraction packets, summaries, comparison JSON/Markdown, and decision docs for figure extraction benchmarking.

### M028 acquisition evidence builders

Proposed category:

```text
m028-acquisition-evidence-output=6
```

Exact files:

- `scripts/build_m028_pdf_acquisition_diagnostics.py` x3
- `scripts/build_m028_universal_loader_evidence_bundles.py` x3

Reviewed writes include redacted events, summaries, reports, and evidence bundles. These are bounded M028 artifact builders.

## No-move candidates

| File | Count | Reason |
|---|---:|---|
| `scripts/verify_m029_unified_source_acquisition.py` | 4 | Mixed verifier helpers plus optional report writes; better reviewed in a source-acquisition verifier wave. |
| `scripts/audit_test_architecture.py` | 3 | Architecture guardrail script; do not mix with milestone artifact scripts. |
| `scripts/build_r024_*` and `scripts/extract_r024_*` | many | R024 corpus scripts deserve a dedicated wave because they include corpus construction, probes, coverage, and quality metrics. |
| `scripts/inventory_write_paths.py` | 3 | Scanner self-outputs should be reviewed separately after delta support stabilizes. |

## Expected count movement if accepted

```text
script-only -30
m061-acquisition-pipeline-output +11
figure-extraction-benchmark-output +13
m028-acquisition-evidence-output +6
```

Expected final high-level counts:

```text
script-only=235
total_records=341
unknown=0
shared-state=0
```

## Safety rules

- Classify by exact script file path only.
- Do not classify generic target names like `path`, `output_path`, `summary_path`, or `decision_path`.
- Keep M029, R024, architecture audit, and scanner self-output scripts as `script-only` in M176.
- Preserve all M171-M175 categories.

## Evidence

- Candidate extraction: `gsd_exec[bb7afe65-1d87-48e4-9f10-9b2e911a4b0a]`
- Source review covered write-adjacent context in M061, M057-M058, M028, and M029 scripts.
