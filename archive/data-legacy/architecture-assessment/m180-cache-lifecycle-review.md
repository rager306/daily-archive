# M180 Cache Lifecycle Review

## Verdict

**No scanner movement in M180 cache lifecycle review.**

The review found cache-like, index-like, markdown-like, manifest-like, converter-like, and newly moved verify-output records, but no new stable shared cache lifecycle with exact ownership and concurrency proof that would justify a new scanner category in this milestone.

## Candidate summary

See `data/architecture-assessment/m180-cache-lifecycle-candidates.md`.

Relevant current categories remain conservative:

- `caller-owned`: caller-provided markdown converter and coverage report outputs.
- `caller-owned-index`: paired review index output.
- `parser-replay-output`: reviewed parser replay cache path.
- Exact script/package output categories for M057, M060, M061, verify_m031, verify_m033, R024, inventory, and article artifacts.
- Residual `script-only` for process-boundary scripts without exact ownership review.

## Decision

Do not add a broad cache, markdown, manifest, converter, or index target-name rule. Do not move residual `script-only` rows only because their target contains `md`, `manifest`, `markdown`, `cache`, or `index`.

## Rationale

A cache lifecycle category should prove all of the following before movement:

1. Exact source path ownership.
2. Stable lifecycle semantics, not just a markdown/report artifact.
3. Clear concurrency and invalidation behavior.
4. No hiding of shared mutable state.

M180 does not need new cache movement to satisfy the user request; the dedicated review is complete as an explicit no-move policy.

## Regression assertions

```text
unknown=0
shared-state=0
markdown_converter remains caller-owned
chunk_baseline_measurement index remains caller-owned-index
parser replay cache remains parser-replay-output
no broad cache category added
```

## Follow-up trigger

Open a future cache lifecycle milestone only if a stable shared cache/index file appears with exact lifecycle ownership and concurrency behavior to review.
