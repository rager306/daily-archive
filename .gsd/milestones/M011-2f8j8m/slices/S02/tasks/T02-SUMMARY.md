---
id: T02
parent: S02
milestone: M011-2f8j8m
key_files:
  - .gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/redacted-semantic-judgments.json
  - .gsd/milestones/M011-2f8j8m/slices/S02/semantic-judgment-summary.md
key_decisions:
  - Classify all outlier targets as repair_required because they need chunk-boundary or route/span repair before semantic import review.
  - Classify non-outlier controls as retrieval_only because they are useful sources but still lack chunk-level span and candidate-claim locators.
  - Do not classify any M010-derived target as import_candidate.
duration: 
verification_result: passed
completed_at: 2026-05-20T08:27:34.495Z
blocker_discovered: false
---

# T02: Applied redacted semantic judgments: 7 repair_required, 3 retrieval_only, 0 import candidates.

**Applied redacted semantic judgments: 7 repair_required, 3 retrieval_only, 0 import candidates.**

## What Happened

Applied the semantic import-readiness rubric to all 10 S01 targets. The resulting redacted judgments classify 7 outlier targets as repair_required and 3 controls as retrieval_only. No targets are import_candidate. The primary blocker across targets is missing chunk-level span provenance and missing candidate claim locators in the M010 artifact contract. The packet creates no trusted facts and includes no raw paper text, chunk text, claim text, embeddings, vectors, secrets, optimizer traces, binary payloads, or base64.

## Verification

redacted-semantic-judgments.json exists and confirms target_count=10, raw_text_included=false, and trusted_facts_created=false.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `apply rubric over S01 targets and write redacted-semantic-judgments.json` | 0 | ✅ pass — 10 targets; repair_required=7; retrieval_only=3; import_candidate_count=0 | 5200ms |
| 2 | `judgment packet guard assertions` | 0 | ✅ pass — redacted-semantic-judgments-ok | 5200ms |

## Deviations

None.

## Known Issues

Judgments are based on redacted M010 aggregate metadata and source path/hash provenance, not quoted source text or chunk-level spans. This is intentionally conservative and should be independently reviewed in S03.

## Files Created/Modified

- `.gsd/milestones/M011-2f8j8m/slices/S02/run-evidence/redacted-semantic-judgments.json`
- `.gsd/milestones/M011-2f8j8m/slices/S02/semantic-judgment-summary.md`
