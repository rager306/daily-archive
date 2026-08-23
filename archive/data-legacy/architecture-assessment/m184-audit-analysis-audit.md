# M184 Audit Analysis Audit

## Verdict

**Movement decision: move all 24 audit-analysis residual records by exact source path.**

## Baseline

```text
script-only=79
unknown=0
shared-state=0
```

## GitNexus

- `_classify` impact: UNKNOWN, not safety proof.
- GitNexus surfaced active M056 `analyze_wave_*` flows and tests around M056 wave analysis and fd contract behavior.
- S04 edits scanner classification and tests only; no runtime audit/analyze behavior changes.

## Candidate decisions

| Path group | Records | Decision | Category |
|---|---:|---|---|
| `scripts/analyze_m056_wave_1.py` through `scripts/analyze_m056_wave_6.py` | 11 | Move | `audit-analysis-output` |
| `scripts/audit_locator_evidence.py` | 2 | Move | `audit-analysis-output` |
| `scripts/audit_m042_connectivity_groups.py` | 2 | Move | `audit-analysis-output` |
| `scripts/audit_m053_grobid_pilot.py` | 1 | Move | `audit-analysis-output` |
| `scripts/audit_pipeline_scripts.py` | 1 | Move | `audit-analysis-output` |
| `scripts/check_project_trajectory.py` | 2 | Move | `audit-analysis-output` |
| `scripts/test_fd_contract.py` | 3 | Move | `audit-analysis-output` |
| `scripts/verify_test_architecture.py` | 2 | Move | `audit-analysis-output` |

## Boundaries

- No broad `analyze`, `audit`, `verify`, `test`, `trajectory`, `tmp_path`, `path`, `json_path`, `markdown_path`, or `artifact_dir` rule.
- No runtime code movement in S04.
- These are reviewed process-boundary audit/analysis outputs, not shared cache/index state.
