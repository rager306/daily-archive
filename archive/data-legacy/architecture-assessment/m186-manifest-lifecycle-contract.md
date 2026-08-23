# M186 Manifest Lifecycle Contract

## Verdict

**All four manifest/cache residuals are blocked until lifecycle proof is complete.**

## Required proof dimensions

1. owner
2. invalidation
3. consumer
4. atomicity
5. lifecycle_tests

## Movement rule

A residual may move out of `scripts/` only when every proof dimension is present and the focused lifecycle tests pass. Partial proof keeps the residual blocked and preserves `script-only=4`.

## Machine-checkable contract

See `data/architecture-assessment/m186-manifest-lifecycle-contract.json`.

## Current state

The four residuals remain blocked. Existing tests provide partial lifecycle coverage, but owner, invalidation, and atomicity are not yet proven.
