# Independent Semantic Review: M020 S04

**Verdict: FLAG**

The reviewed artifacts are safe and useful as **candidate locator evidence**, but they do not yet support positive KG import. The protocol blocks fact promotion correctly, the fixtures preserve redaction boundaries, and the small-batch rehearsal exposes the right failure mode: locator generation can produce source-coordinate pointers, but most candidate spans remain ambiguous and semantically unverified.

## Findings

### PASS: Protocol sufficiency for review-only locators

**Artifacts:**

- `.gsd/milestones/M020-uh5kvt/slices/S01/candidate-locator-protocol.md`
- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-schema.json`
- `.gsd/milestones/M020-uh5kvt/slices/S01/run-evidence/candidate-locator-protocol-guard.json`

The protocol is sufficient for M020's stated purpose: redacted, coordinate-bearing evidence pointers that remain non-factual and import-disabled. It defines source ledgers, span coordinates, locator states, support labels, uncertainty labels, safety flags, and summary counts clearly enough for downstream reviewer tooling or deterministic implementation.

**Caveat:** It is not sufficient as a positive import protocol. It lacks a semantic acceptance gate, reviewer decision model, contradiction handling beyond labels, and evidence-to-fact promotion criteria.

### PASS_WITH_LIMITATION: One-paper fixture is meaningful

**Artifacts:**

- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-locator-fixture.json`
- `.gsd/milestones/M020-uh5kvt/slices/S02/run-evidence/one-paper-locator-guard.json`
- `.gsd/milestones/M020-uh5kvt/slices/S02/one-paper-semantic-spot-check.md`

The one-paper fixture is meaningful as a protocol exerciser: it has a source ledger, hash identity, coordinate-bearing locators, redacted span hashes, review-only states, and import-disabled flags. It covers multiple locator categories rather than only a happy path.

**Limitation:** Semantic support is explicitly not evaluated. The fixture proves shape and safety, not correctness of candidate meaning.

### FLAG: Small-batch ambiguity is high enough to block positive import work

**Artifacts:**

- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-locator-rehearsal.json`
- `.gsd/milestones/M020-uh5kvt/slices/S03/run-evidence/small-batch-locator-guard.json`
- `.gsd/milestones/M020-uh5kvt/slices/S03/small-batch-rehearsal-recommendation.md`

The small-batch rehearsal is valuable because it scales the protocol to 10 papers and 35 locators while preserving source hashes and redaction. However, the ambiguity signal is dominant: 27 of 35 locators are marked ambiguous, with only 1 review-required and 7 retrieval-only.

That pattern suggests the current locator generation is mostly finding broad coordinate regions, not precise semantic support. This is useful diagnostic evidence, but it argues against positive import-gate work next.

### PASS: Redaction and safety boundaries hold

Across all reviewed artifacts:

- `production_import_attempted=false`
- `ladybugdb_written=false`
- `trusted_kg_import_allowed=false`
- `import_eligible_count=0`
- `promoted_to_fact_count=0`
- raw text/chunk text/model payload/vector/embedding inclusion flags remain false
- guards report forbidden exact payload keys absent

No raw paper, chunk, or claim text is included in the reviewed artifacts. The artifacts consistently treat locators as evidence pointers, not KG facts.

## Risks

1. **Semantic overclaim risk:** Counts and coordinate presence could be mistaken for evidence support unless downstream tooling keeps review-only semantics prominent.
2. **Ambiguous-span risk:** Broad keyword-style spans may create reviewer burden and false confidence.
3. **Protocol-to-implementation gap:** The protocol is well-defined, but deterministic enforcement in code is not yet demonstrated here.
4. **Chunking/structure risk:** Many ambiguous locators may reflect weak source segmentation or poor structural preservation, not only locator logic.
5. **Premature import risk:** Positive KG import or LadybugDB writes would be unsafe from these artifacts alone.

## Recommendation

Proceed next with **deterministic locator implementation plus ambiguity diagnostics**, not positive import-gate work.

Recommended ordering:

1. **Deterministic locator implementation** — encode the protocol into reproducible code with schema validation, safety guards, source hash checks, coordinate validation, and explicit import-disabled outputs.
2. **Chunking/structure repair investigation** — use the high ambiguity rate to identify whether broad spans come from chunking, conversion quality, missing section structure, or locator heuristics.
3. **Reviewer packet/UI after deterministic output exists** — build review surfaces once locator generation is stable enough to present compact, comparable evidence packets.
4. **Defer positive import-gate work** — no positive KG import or LadybugDB writes should proceed until deterministic locators plus independent semantic review demonstrate precise support on a meaningful batch.

**Final decision:** Candidate-locator work may continue; positive KG import and LadybugDB writes must remain blocked.
