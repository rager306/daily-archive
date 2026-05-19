---
id: T03
parent: S02
milestone: M007-opaont
key_files:
  - .gsd/milestones/M007-opaont/slices/S02/run-evidence/batch-state.json
  - .gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json
  - .gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-diagnostics.jsonl
  - .gsd/milestones/M007-opaont/slices/S02/source-preflight-report.md
  - src/arxiv_archive/validation_batch_workflow.py
key_decisions:
  - Persist the dry-run artifacts in S02 run-evidence and keep initial init artifacts under a batch subdirectory.
  - Treat 20 `ready_with_missing_markdown_risk_tag` diagnostics as expected warnings: they indicate historical risk tags after successful acquisition, not current blockers.
  - Keep source preflight as path inspection only; no acquisition, conversion, scan, import, or LadybugDB write occurred.
duration: 
verification_result: passed
completed_at: 2026-05-19T19:13:49.340Z
blocker_discovered: false
---

# T03: Ran the bounded M007 source preflight dry run and produced redacted 30-paper readiness evidence.

**Ran the bounded M007 source preflight dry run and produced redacted 30-paper readiness evidence.**

## What Happened

Ran a bounded source preflight dry run through the new validation-batch CLI against the M006 30-paper manifest. The initial run exposed a source-path fallback gap in the preflight helper; after fixing fallback path conventions, the regenerated run reported 30/30 Markdown-ready, 8 PDFs present, 22 PDFs missing, 20 warning diagnostics, and 0 blockers. The warnings surface historical `missing_markdown` risk tags on papers that are now scan-ready, matching the M006 review requirement to expose readiness/risk-tag contradictions. No source acquisition, conversion, scan execution, KG import, or LadybugDB write was performed.

## Verification

Artifact guard confirmed 30 papers, 30 ready for Markdown scan, 8 PDFs present, production import false, LadybugDB writes false, and report present. 20 focused tests passed and ruff passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python -m arxiv_archive validation-batch init ... && uv run python -m arxiv_archive validation-batch preflight ...` | 0 | ✅ pass — paper_count=30; ready_for_markdown_scan_count=30; pdf_present=8; warnings=20; blockers=0; no writes/import | 5200ms |
| 2 | `test -s .gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json && uv run python - <<'PY' ... guard ... PY && uv run pytest tests/test_validation_batch_workflow.py tests/test_validation_batch_cli_preflight.py tests/test_validation_batch_state.py -q && uv run ruff check src/arxiv_archive/cli.py src/arxiv_archive/validation_batch_workflow.py src/arxiv_archive/validation_batch_state.py tests/test_validation_batch_workflow.py tests/test_validation_batch_cli_preflight.py tests/test_validation_batch_state.py` | 0 | ✅ pass — source-preflight-ok; 20 passed; ruff all checks passed | 6600ms |

## Deviations

The first dry run showed only 10/30 Markdown-ready because preflight trusted stale manifest source paths and did not use deterministic fallback locations. The workflow helper was fixed to inspect `/root/.research/papers/{paper_id}/full_text.md` and cache PDF/Markdown fallback paths, then the dry run was regenerated with 30/30 Markdown-ready.

## Known Issues

The 20 missing_markdown risk-tag warnings need future resolution semantics so historical acquisition gaps are not confused with current readiness. S02 does not run conversion quality scoring.

## Files Created/Modified

- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/batch-state.json`
- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-summary.json`
- `.gsd/milestones/M007-opaont/slices/S02/run-evidence/source-preflight-diagnostics.jsonl`
- `.gsd/milestones/M007-opaont/slices/S02/source-preflight-report.md`
- `src/arxiv_archive/validation_batch_workflow.py`
