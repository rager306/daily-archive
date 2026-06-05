---
id: T04
parent: S06
milestone: M033-732r1t
key_files:
  - scripts/verify_m033_external_parser_quality_plan.py
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-summary.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-report.md
  - data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.md
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.md
key_decisions:
  - S06 closeout must fail if the future plan omits graph-readiness review post-check, no-write/no-import boundaries, component quality dimensions, typed diagnostics, or no-secret/no-raw-body logging rules.
duration: 
verification_result: passed
completed_at: 2026-06-05T11:52:26.199Z
blocker_discovered: false
---

# T04: Added and passed the S06 validate-only quality-plan closeout checker.

**Added and passed the S06 validate-only quality-plan closeout checker.**

## What Happened

Implemented `scripts/verify_m033_external_parser_quality_plan.py` as a fail-closed checker for S06 artifacts. The verifier validates the future probe scope, quality metrics/gates, artifact contracts/diagnostics, rollback criteria, and event log. It rejects missing corpus classes, missing excluded production actions, missing quality dimensions, missing graph-readiness review post-check, missing no-secret/no-raw-body rules, missing typed diagnostics, nonzero no-write rehearsal counts, unsafe graph/import/write flags, or any production integration/import eligibility authorization. The first run failed on missing explicit markdown wording for `model/backend cache`, `GROBID`, `OpenDataLoader`, and `Adaptix`; the reports were clarified without weakening safety checks, and the retry passed verifier plus Ruff.

## Verification

Fresh retry gate passed: `uv run python scripts/verify_m033_external_parser_quality_plan.py --plan-dir data/article_corpora/m033-external-parser-quality-plan-v1` returned `status: passed`, `failure_count: 0`, `verdict: bounded-future-quality-plan-ready`; `uv run ruff check scripts/verify_m033_external_parser_quality_plan.py` returned `All checks passed!`. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/verify_m033_external_parser_quality_plan.py --plan-dir data/article_corpora/m033-external-parser-quality-plan-v1 && uv run ruff check scripts/verify_m033_external_parser_quality_plan.py` | 0 | ✅ pass | 3400ms |

## Deviations

The initial verifier run failed because markdown reports lacked exact component/cache wording expected by the closeout checker. I clarified the markdown evidence and reran successfully; no safety criteria were relaxed.

## Known Issues

The verifier validates the future quality plan artifacts only; it does not execute the future parser-quality probe.

## Files Created/Modified

- `scripts/verify_m033_external_parser_quality_plan.py`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-summary.json`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-report.md`
- `data/article_corpora/m033-external-parser-quality-plan-v1/future-probe-scope.md`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-metrics-and-gates.md`
