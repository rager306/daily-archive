# S01: Candidate locator protocol contract — UAT

**Milestone:** M020-uh5kvt
**Written:** 2026-05-21T09:16:26.565Z

# S01 UAT

## Scenario

A future agent needs to generate one-paper candidate locators without promoting facts or enabling KG import.

## Expected behavior

- The agent can read `candidate-locator-protocol.md` for human contract details.
- The agent can read `candidate-locator-protocol-schema.json` for required fields and allowed values.
- The agent can verify `candidate-locator-protocol-guard.json` passed.
- The protocol requires `import_eligible=false`, `promoted_to_fact=false`, `trusted_kg_import_allowed=false`, and `ladybugdb_written=false`.
- Machine artifacts must avoid raw paper text, chunk text, embeddings, vectors, secrets, and model payloads.

## Evidence

Fresh verification returned:

```text
m020-s01-final-verification-ok
```

## Verdict

PASS for contract readiness. S02 may proceed to a one-paper locator fixture under these constraints.

