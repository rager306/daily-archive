# M140 Dynamic Ratchet Candidate

Schema: `daily-archive-m140-dynamic-ratchet-candidate.v1`

## Skipped candidate

- `tests/test_dspy_extraction_boundary.py`: baseline-red before migration; stale static-scope path `src/research_graph.infrastructure.evaluation.dspy_extraction.py` does not exist.

## Selected file

- `tests/test_m024_validation_evidence_closure.py`
- Current bucket: `legacy-mixed`
- Baseline pytest: `14 passed`.
- GitNexus blast radius: LOW.

## Intended count delta

| Metric | Before | Delta | Expected after |
|---|---:|---:|---:|
| `dynamic_script_import` | 46 | -1 | 45 |
| `legacy_mixed` | 60 | -1 | 59 |
| `strict_script_wrapper` | 13 | +1 | 14 |
| `unknown` | 77 | +0 | 77 |
