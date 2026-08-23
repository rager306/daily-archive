# M184 Render Report Audit

## Verdict

**Movement decision: move all 8 render-report residual records by exact source path.**

## Baseline

```text
script-only=55
unknown=0
shared-state=0
```

## GitNexus

- `_classify` impact: UNKNOWN, not safety proof.
- GitNexus surfaced bounded chunk repair contract and reviewer packet render flows, including tests around renderer CLI output validation.
- S05 edits scanner classification and tests only; no runtime report/contract behavior changes.

## Candidate decisions

| Path | Records | Decision | Category |
|---|---:|---|---|
| `scripts/render_bounded_repair_prototype.py` | 2 | Move | `render-report-contract-output` |
| `scripts/render_chunk_repair_contract.py` | 2 | Move | `render-report-contract-output` |
| `scripts/render_m055_report.py` | 1 | Move | `render-report-contract-output` |
| `scripts/render_m055deep_report.py` | 1 | Move | `render-report-contract-output` |
| `scripts/render_m056_report.py` | 1 | Move | `render-report-contract-output` |
| `scripts/render_reviewer_packet_prototype.py` | 1 | Move | `render-report-contract-output` |

## Boundaries

- No broad `render`, `report`, `contract`, `output`, `path`, or `temp_path` rule.
- No runtime code movement in S05.
- These are reviewed process-boundary render/report/contract outputs.
