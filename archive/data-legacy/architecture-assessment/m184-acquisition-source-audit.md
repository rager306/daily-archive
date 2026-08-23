# M184 Acquisition Source Audit

## Verdict

**Movement decision: move all 10 acquisition-source residual records by exact source path.**

## Baseline

```text
script-only=89
unknown=0
shared-state=0
```

## GitNexus

- `_classify` impact: UNKNOWN, not safety proof.
- Acquisition/source query surfaced M056 analyze/acquisition-adjacent flows and tests for linked PDF acquisition.
- Because this slice edits only scanner classification and tests, behavior safety is proven by focused inventory tests, generated deltas, and strict canonical drift.

## Candidate decisions

| Path | Records | Current target examples | Decision | Category |
|---|---:|---|---|---|
| `scripts/acquire_linked_target_pdfs.py` | 2 | `tmp_path`, `log_path` | Move | `source-acquisition-evidence-output` |
| `scripts/acquire_m056_wave.py` | 2 | `tmp_path` | Move | `source-acquisition-evidence-output` |
| `scripts/audit_m054_pdf_acquisition.py` | 1 | `DEFAULT_AUDIT_PATH` | Move | `source-acquisition-evidence-output` |
| `scripts/capture_m027_mixed_source_sources.py` | 2 | `report_path` | Move | `source-acquisition-evidence-output` |
| `scripts/convert_m027_source_quality_boundary.py` | 1 | `fd` | Move | `source-acquisition-evidence-output` |
| `scripts/convert_m029_unified_source_quality_boundary.py` | 1 | `fd` | Move | `source-acquisition-evidence-output` |
| `scripts/emit_m056_candidate_edges.py` | 1 | `output` | Move | `source-acquisition-evidence-output` |

## Boundaries

- No broad `acquire`, `source`, `pdf`, `tmp_path`, `fd`, `output`, or `report_path` rule.
- No runtime code movement in S03.
- These are reviewed process-boundary acquisition/source evidence outputs, not shared cache/index state.
