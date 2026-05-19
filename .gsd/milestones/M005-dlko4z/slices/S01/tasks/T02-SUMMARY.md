---
id: T02
parent: S01
milestone: M005-dlko4z
key_files:
  - .gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json
  - .gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-rationale.md
key_decisions:
  - Use the existing M004 ten-paper corpus as the M005 outer gate rather than adding new papers.
  - Require a six-paper inner review minimum focused on repaired conversion, S10 chunk-review blocker, trusted S07 claims, math/theory, multimodal, and table/figure risk cases.
  - Treat missing artifacts as S02 measurement findings rather than broadening the corpus during S01.
duration: 
verification_result: passed
completed_at: 2026-05-19T05:04:57.167Z
blocker_discovered: false
---

# T02: Selected the representative M005 gold corpus from the existing ten-paper M004 validation set.

**Selected the representative M005 gold corpus from the existing ten-paper M004 validation set.**

## What Happened

Created the S01 gold corpus manifest and rationale. The manifest uses the existing deterministic M004 ten-document corpus as the outer benchmark gate and tags each paper with expected hard cases and checks. It also defines a six-paper inner review minimum covering repaired conversion failures, S10 chunk review evidence, trusted S07 candidate claims, math/theory, multimodal, table/figure, and method/result boundary risks. The manifest explicitly records that no broad corpus run, production import, LadybugDB write, raw text, or embeddings are included.

## Verification

Manifest verification loaded JSON, checked schema version, required fields, paper count, and broad_corpus_run=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python - <<'PY' ... verify gold-corpus-manifest.json ... PY` | 0 | ✅ pass — papers=10, inner_review_minimum=6, broad_corpus_run=False | 0ms |

## Deviations

Selected the full existing M004 ten-paper corpus rather than a smaller six-paper subset so S02 baseline measurement cannot cherry-pick around known failures. The inner review minimum remains six papers.

## Known Issues

Some manifest paths may not exist in the current filesystem because earlier M004 artifacts recorded missing per-paper full_text paths before later repairs. S02 must report missing artifacts as blockers/repair-required diagnostics instead of silently skipping them.

## Files Created/Modified

- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-manifest.json`
- `.gsd/milestones/M005-dlko4z/slices/S01/gold-corpus-rationale.md`
