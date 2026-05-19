---
id: T02
parent: S03
milestone: M006-638rza
key_files:
  - src/arxiv_archive/thirty_paper_deviation_scan.py
  - tests/test_thirty_paper_deviation_scan.py
  - .gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json
  - .gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-diagnostics.jsonl
key_decisions:
  - Treat the corrected S03 run evidence as authoritative because it includes byte-size/density metrics.
  - Keep import eligibility at zero and preserve negative/no-write boundary.
duration: 
verification_result: passed
completed_at: 2026-05-19T18:04:17.702Z
blocker_discovered: false
---

# T02: Ran the 30-paper deviation scan and produced redacted evidence for 4,289 chunks across 30 papers.

**Ran the 30-paper deviation scan and produced redacted evidence for 4,289 chunks across 30 papers.**

## What Happened

Ran the 30-paper deviation scanner and wrote redacted summary/diagnostics artifacts. The corrected run covers all 30 Markdown-ready papers, emits 30 per-paper diagnostics, and reports 4,289 structure-aware chunks over 1,761,102 Markdown bytes. It found 11 outlier papers by simple deterministic flags and zero import-eligible chunks. Route/type/refusal distributions are now available for S03 comparison against M005. No raw text, chunk text, embeddings, vectors, production import, or LadybugDB writes are emitted.

## Verification

Fresh verification passed: artifact guard confirmed 30 papers, 30 Markdown-ready, 4,289 chunks, non-zero Markdown byte total, 11 outliers, zero import eligibility, and safety flags false; 34 focused tests passed; ruff passed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `gitnexus_impact({target: "write_thirty_paper_deviation_run", direction: "upstream", repo: "daily-archive"})` | 0 | ℹ️ target not found because new scanner not indexed yet; T02 was run-only after T01 commit | 0ms |
| 2 | `uv run python - <<'PY' ... build/write scan ... PY` | 0 | ✅ pass — paper_count=30; chunk_count=4289; markdown_byte_size_total=1761102; outlier_count=11; import_eligible=0; safety_flags_false=true | 10400ms |
| 3 | `uv run python - <<'PY' ... artifact guard ... PY && uv run pytest tests/test_thirty_paper_deviation_scan.py tests/test_structure_aware_chunking.py tests/test_chunking_benchmark.py -q && uv run ruff check src/arxiv_archive/thirty_paper_deviation_scan.py tests/test_thirty_paper_deviation_scan.py` | 0 | ✅ pass — artifact guard, 34 tests, and ruff passed | 9900ms |

## Deviations

The first run produced valid counts but had `markdown_byte_size_total=0` because source byte size was taken from the package source artifact field instead of the selected full_text.md path. The scanner was fixed and the run evidence regenerated before completing the task.

## Known Issues

The run is Markdown-based. PDF availability remains 8/30 and multimodal completeness is not measured here.

## Files Created/Modified

- `src/arxiv_archive/thirty_paper_deviation_scan.py`
- `tests/test_thirty_paper_deviation_scan.py`
- `.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-summary.json`
- `.gsd/milestones/M006-638rza/slices/S03/run-evidence/thirty-paper-deviation-diagnostics.jsonl`
