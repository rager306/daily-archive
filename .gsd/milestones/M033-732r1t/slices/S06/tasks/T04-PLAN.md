---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T04: Added and passed the S06 validate-only quality-plan closeout checker.

Add a validate-only closeout checker for S06 artifacts and run the acceptance gate. It must reject missing quality dimensions, missing no-write/no-import boundaries, permissive flags, or any claim that M033 authorized production integration.

## Inputs

- `data/article_corpora/m033-external-parser-quality-plan-v1/`

## Expected Output

- `scripts/verify_m033_external_parser_quality_plan.py`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-summary.json`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-closeout-report.md`

## Verification

`uv run python scripts/verify_m033_external_parser_quality_plan.py --plan-dir data/article_corpora/m033-external-parser-quality-plan-v1 && uv run ruff check scripts/verify_m033_external_parser_quality_plan.py` exits 0.

## Observability Impact

Provides machine-checkable S06 closeout for milestone validation.
