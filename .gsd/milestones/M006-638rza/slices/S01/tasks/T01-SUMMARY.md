---
id: T01
parent: S01
milestone: M006-638rza
key_files:
  - .gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json
  - .gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-rationale.md
key_decisions:
  - The 30-paper corpus keeps all 10 M005 papers as overlap and adds 20 deterministic local candidates selected by availability score then paper id.
  - The corpus is diagnostic only and does not authorize KG import, embeddings, vectors, or production writes.
duration: 
verification_result: passed
completed_at: 2026-05-19T16:25:19.611Z
blocker_discovered: false
---

# T01: Selected the 30-paper deviation-scan corpus with M005 overlap preserved.

**Selected the 30-paper deviation-scan corpus with M005 overlap preserved.**

## What Happened

Selected a deterministic 30-paper corpus for M006. The manifest includes all 10 M005 gold-corpus papers for baseline overlap and adds 20 expansion papers discovered from local research/cache evidence. Each paper record includes selection role, risk tags, source availability status, and redacted source paths only. The manifest keeps all no-import/no-write/no-payload safety flags false.

## Verification

Manifest guard passed: 30 unique paper ids, 10 M005 overlap, 20 expansion papers, and no import/write/raw-text flags enabled.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `uv run python - <<'PY' ... manifest guard ... PY` | 0 | ✅ pass — paper_count=30, m005_overlap_count=10 | 4400ms |

## Deviations

None.

## Known Issues

The expansion selection is availability-biased toward local cached/research artifacts. S02/S03 must treat this as a deviation scan, not a representative random sample of arXiv.

## Files Created/Modified

- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-manifest.json`
- `.gsd/milestones/M006-638rza/slices/S01/thirty-paper-corpus-rationale.md`
