---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T04: Validate and close GROBID bounded study

Add a validate-only closeout checker for S02 artifacts and run the full S02 acceptance gate. The closeout must pass whether the service produced TEI outputs or produced a typed service blocker, but it must reject any permissive graph/import safety flag.

## Inputs

- `data/article_corpora/m033-grobid-probe-v1/`

## Expected Output

- `scripts/verify_m033_grobid_probe.py`
- `data/article_corpora/m033-grobid-probe-v1/grobid-closeout-summary.json`
- `data/article_corpora/m033-grobid-probe-v1/grobid-closeout-report.md`

## Verification

`uv run python scripts/verify_m033_grobid_probe.py --probe-dir data/article_corpora/m033-grobid-probe-v1 && uv run ruff check scripts/verify_m033_grobid_probe.py` exits 0.

## Observability Impact

Provides a single validate-only closeout surface for S05/S06 and future agents.
