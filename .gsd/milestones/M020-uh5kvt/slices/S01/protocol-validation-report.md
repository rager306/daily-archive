# M020 S01 protocol validation report

## Verdict

```text
PASS
```

The candidate locator protocol defines a review-only evidence pointer contract and keeps positive KG import blocked.

## Evidence

Guard artifact:

```text
.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-guard.json
```

Schema artifact:

```text
.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json
```

Protocol artifact:

```text
.gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md
```

Fresh guard command returned:

```text
m020-s01-protocol-guard-ok
```

## Checks covered

The guard verified that the protocol/schema define:

- required top-level artifact fields;
- source ledger fields;
- source span fields;
- semantic chunk coordinate fields;
- review queue reasons;
- import/write/embedding/source-of-truth exclusions;
- forbidden raw payload keys;
- M020-required safety flags.

## Safety state

The protocol requires these states for M020 artifacts:

```text
production_import_attempted=false
ladybugdb_written=false
trusted_kg_import_allowed=false
raw_text_included=false
chunk_text_included=false
embeddings_included=false
vectors_included=false
secrets_included=false
minimax_source_of_truth=false
kg_fact_promotion_allowed=false
counts_alone_establish_readiness=false
```

## Interpretation

S01 does not produce candidate locators yet. It defines the contract that S02 must use for a one-paper fixture. The contract is intentionally conservative: candidate locators are review evidence, not KG facts, and all positive import paths remain blocked.

## Next slice input

S02 may use the protocol to produce a single-paper locator fixture only if it preserves:

- exact source/chunk coordinate references;
- no raw source text in machine artifacts;
- `import_eligible=false`;
- `promoted_to_fact=false`;
- no LadybugDB writes.
