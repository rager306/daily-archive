# M184 Cache Manifest Proof Gate

## Verdict

**Cache/index/manifest movement remains fail-closed.**

A cache/index/manifest path can move out of `script-only` only when all proof dimensions are explicit.

## Required proof dimensions

| Dimension | Required evidence |
|---|---|
| Owner | Named module/function/use case owns writes and maintenance. |
| Invalidation | Rules for when the artifact becomes stale and how it is refreshed. |
| Consumer contract | Stable readers and schema/format expectations are named. |
| Concurrency | Write coordination, atomicity, race behavior, and multi-run handling are defined. |
| Lifecycle | Creation, update, cleanup, and historical retention semantics are defined. |

## Stop conditions

- No broad `cache`, `index`, `manifest`, `graph_manifest`, `corpus_manifest`, or target-name scanner rule.
- No movement based only on filename, target variable, or benchmark family.
- No movement when GitNexus surfaces related flows but not exact ownership/invalidation proof.
