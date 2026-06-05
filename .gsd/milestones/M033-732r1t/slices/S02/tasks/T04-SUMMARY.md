---
id: T04
parent: S02
milestone: M033-732r1t
key_files:
  - scripts/verify_m033_grobid_probe.py
  - data/article_corpora/m033-grobid-probe-v1/grobid-closeout-summary.json
  - data/article_corpora/m033-grobid-probe-v1/grobid-closeout-report.md
key_decisions:
  - Use a validate-only closeout verifier that treats GROBID output as candidate evidence and rejects any positive graph/import/write safety flag.
duration: 
verification_result: passed
completed_at: 2026-06-05T10:18:31.811Z
blocker_discovered: false
---

# T04: Added and ran a validate-only closeout checker for the GROBID bounded probe.

**Added and ran a validate-only closeout checker for the GROBID bounded probe.**

## What Happened

Implemented `scripts/verify_m033_grobid_probe.py` as a fail-closed verifier for S02 artifacts. It checks runtime readiness, Docker image selection, TEI run summary, per-paper TEI and diagnostics, TEI quality summary, probe verdict, mapping report boundary language, and all graph/import/LadybugDB safety flags. The verifier writes `grobid-closeout-summary.json` and `grobid-closeout-report.md`, and rejects any permissive graph/import/write flag. The full T04 gate ran the verifier and Ruff successfully.

## Verification

Fresh T04 gate passed: `uv run python scripts/verify_m033_grobid_probe.py --probe-dir data/article_corpora/m033-grobid-probe-v1` returned `status: passed`, `failure_count: 0`, `verdict: grobid-scholarly-sidecar-candidate`; `uv run ruff check scripts/verify_m033_grobid_probe.py` returned `All checks passed!`. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python scripts/verify_m033_grobid_probe.py --probe-dir data/article_corpora/m033-grobid-probe-v1 && uv run ruff check scripts/verify_m033_grobid_probe.py` | 0 | ✅ pass | 4700ms |

## Deviations

None.

## Known Issues

Closeout validates structural consistency and fail-closed boundaries, not semantic correctness of every TEI extraction or full/DL image accuracy.

## Files Created/Modified

- `scripts/verify_m033_grobid_probe.py`
- `data/article_corpora/m033-grobid-probe-v1/grobid-closeout-summary.json`
- `data/article_corpora/m033-grobid-probe-v1/grobid-closeout-report.md`
