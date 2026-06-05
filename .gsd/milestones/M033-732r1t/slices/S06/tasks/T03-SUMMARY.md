---
id: T03
parent: S06
milestone: M033-732r1t
key_files:
  - data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.json
  - data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.md
  - data/article_corpora/m033-external-parser-quality-plan-v1/adoption-and-rollback-criteria.md
  - data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-events.jsonl
key_decisions:
  - Future diagnostics must log hashes/paths/IDs and typed codes, not secrets or raw article bodies.
  - Any implicit network download, untyped parser failure, refusal bypass, invalid EvidencePath, incomplete review packet, or graph/import/write flag becoming true triggers rollback/no-adoption.
duration: 
verification_result: passed
completed_at: 2026-06-05T11:49:57.937Z
blocker_discovered: false
---

# T03: Defined future artifact contracts, diagnostics, failure taxonomy, and rollback/no-adoption criteria.

**Defined future artifact contracts, diagnostics, failure taxonomy, and rollback/no-adoption criteria.**

## What Happened

Created `artifact-contracts-and-diagnostics.json`, `.md`, and `adoption-and-rollback-criteria.md`. The artifacts define the future quality milestone artifact tree, JSON shape expectations, no-secret/no-raw-body logging rules, typed diagnostic taxonomy, no-write import rehearsal expectations, typed blocker requirements, future adoption minimums, rollback/no-adoption triggers, and explicit non-authorizations. The diagnostic taxonomy includes missing/stale source, unhealthy backend, missing model cache under no-network, TEI parse failure, bibliography quality failure, layout/table/OCR quality failures, Adaptix mapping failure, invalid EvidencePath, low_quality_source, incomplete review packet, and graph-readiness post-check failure.

## Verification

Fresh T03 verification passed in `gsd_exec[d9a5a594-fec6-4f10-9d34-7e40ca260d49]`: the script validated no-secret/no-raw-body logging rules, no-write import rehearsal counts, required diagnostic codes, M033 adoption decision disabled, upstream metrics complete, and all false safety flags. Exit code 0.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `python3 artifact-contracts/diagnostics generation validation script via gsd_exec purpose 'M033 S06 T03 create artifact contracts diagnostics taxonomy'` | 0 | ✅ pass | 66ms |

## Deviations

None.

## Known Issues

Contracts are schema-shape expectations for a future milestone, not formal JSON Schema files yet. A future implementation milestone can convert them into strict schemas once exact artifact names and corpus IDs are selected.

## Files Created/Modified

- `data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.json`
- `data/article_corpora/m033-external-parser-quality-plan-v1/artifact-contracts-and-diagnostics.md`
- `data/article_corpora/m033-external-parser-quality-plan-v1/adoption-and-rollback-criteria.md`
- `data/article_corpora/m033-external-parser-quality-plan-v1/quality-plan-events.jsonl`
