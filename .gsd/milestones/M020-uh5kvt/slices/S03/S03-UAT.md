# S03: Small-batch locator rehearsal — UAT

**Milestone:** M020-uh5kvt
**Written:** 2026-05-21T09:27:51.619Z

# S03 UAT

## Scenario

A reviewer needs batch-level evidence before deciding if candidate locators are meaningful enough for future KG import-gate work.

## Expected behavior

- The batch rehearsal exists at `small-batch-locator-rehearsal.json`.
- It covers 10 M011 targets.
- It reports 35 locators.
- It reports failure-mode metrics: missing, ambiguous, conflicting, retrieval-only, repair-required.
- It records zero import-eligible locators and zero fact promotions.
- Guard safety flags keep production import, LadybugDB writes, raw text, chunk text, embeddings, vectors, secrets, and MiniMax authority blocked.

## Evidence

Fresh verification returned:

```text
m020-s03-final-verification-ok
```

## Verdict

PASS for small-batch rehearsal readiness. Proceed to S04 independent semantic review. Do not proceed to positive KG import from S03 alone.

