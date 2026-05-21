# S02: One-paper locator fixture — UAT

**Milestone:** M020-uh5kvt
**Written:** 2026-05-21T09:22:03.284Z

# S02 UAT

## Scenario

A future agent wants to inspect a one-paper candidate locator artifact before running a small-batch rehearsal.

## Expected behavior

- The fixture exists at `one-paper-locator-fixture.json`.
- It uses `schema_version=candidate_locator_protocol.v1`.
- It references `paper_id=2001.00281v1`.
- It has source path/hash identity and exact coordinate spans.
- It has four review-only locators.
- Every locator has `import_eligible=false`, `promoted_to_fact=false`, and `minimax_source_of_truth=false`.
- Guard safety flags keep production import, LadybugDB writes, raw text, chunk text, embeddings, vectors, and secrets blocked.

## Evidence

Fresh verification returned:

```text
m020-s02-final-verification-ok
```

## Verdict

PASS for one-paper fixture readiness. This is not a positive KG import signal.

