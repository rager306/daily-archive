---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T02: Mapped the current M031 artifact contracts by stage for external parser comparison.

Build a stage-by-stage artifact contract map from M031 and current data: catalog/intake, acquisition, loader evidence, parser/conversion, chunk/evidence, graph-readiness reviewer packets, continuity audit, and no-write import rehearsal. For each stage, record inputs, outputs, key fields, expected counters, hashes/provenance, and downstream consumers.

## Inputs

- `.gsd/milestones/M031-vwpd8e/M031-vwpd8e-SUMMARY.md`
- `.gsd/milestones/M031-vwpd8e/M031-vwpd8e-VALIDATION.md`
- `data/article_catalog/index.json`

## Expected Output

- `data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.json`
- `data/article_corpora/m033-current-parser-baseline-v1/current-artifact-contracts.md`

## Verification

Manual review — file exists and is non-empty

## Observability Impact

Artifact contract map exposes what external parser outputs must preserve or improve.
