---
id: T02
parent: S05
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.json
  - data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.md
  - data/article_corpora/m033-combined-parser-architecture-v1/synthesis-events.jsonl
key_decisions:
  - Recommend a bounded combined sidecar architecture rather than adopting any external parser as a replacement or production dependency.
  - Keep daily-archive as the owner of validation, review, and graph-readiness decisions.
duration: 
verification_result: passed
completed_at: 2026-06-05T11:44:55.308Z
blocker_discovered: false
---

# T02: Wrote the combined sidecar architecture recommendation for GROBID, OpenDataLoader, Adaptix, quant-mind patterns, and daily-archive validators.

**Wrote the combined sidecar architecture recommendation for GROBID, OpenDataLoader, Adaptix, quant-mind patterns, and daily-archive validators.**

## What Happened

Created `combined-parser-recommendation.json` and `.md` with the explicit verdict `recommended-bounded-combined-sidecar-architecture`. The recommendation keeps the architecture bounded and candidate-only: GROBID is the scholarly TEI/metadata/references/citations sidecar, OpenDataLoader-style extraction is the layout/OCR/table/coordinate sidecar, Adaptix is the typed adapter layer over fixed parser JSON, quant-mind contributes architecture patterns only, and daily-archive remains owner of contracts, validators, review gates, graph-readiness, and no-write import boundaries. The artifact rejects GROBID-only replacement, OpenDataLoader-only replacement, Adaptix-as-semantic-validator, quant-mind runtime adoption, and direct parser-to-LadybugDB import.

## Verification

Fresh T02 verification passed in `gsd_exec[c17f2acd-d779-4f98-9c16-99d9a9eabff1]`: the script validated the recommendation verdict, all five component responsibility entries, at least five rejected alternatives, production adoption disabled, and all safety flags false. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `python3 recommendation-generation/validation script via gsd_exec purpose 'M033 S05 T02 create combined parser recommendation'` | 0 | ✅ pass | 60ms |

## Deviations

None.

## Known Issues

Recommendation is bounded research synthesis, not implementation authorization. S06 must still define the future quality plan before any implementation milestone.

## Files Created/Modified

- `data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.json`
- `data/article_corpora/m033-combined-parser-architecture-v1/combined-parser-recommendation.md`
- `data/article_corpora/m033-combined-parser-architecture-v1/synthesis-events.jsonl`
