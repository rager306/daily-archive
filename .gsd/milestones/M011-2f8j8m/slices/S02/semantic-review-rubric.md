# M011 semantic import-readiness rubric

## Purpose

Evaluate whether M010 review targets can support trusted scientific KG extraction without performing positive import or writing to LadybugDB.

This rubric is intentionally conservative. It may classify evidence as retrieval-only or repair-required even when the source paper itself is valid, because import readiness requires chunk-level provenance and reviewable support paths, not just paper-level availability.

## Evidence allowed

Allowed in review artifacts:

- Paper id
- Source path and SHA256 hash
- Paper-level source locator
- M010 aggregate chunk metrics
- Route, state, refusal, and outlier categories
- Redacted categorical judgments
- Reviewer notes that do not quote source text or claim text

Not allowed in review artifacts:

- Raw paper text
- Chunk text
- Claim text
- Extracted trusted facts
- Embeddings or vectors
- Binary or base64 payloads
- Secrets or credentials
- Optimizer traces

## Classification labels

### `import_candidate`

Only allowed when all conditions hold:

- Chunk-level source span provenance is available.
- The target contains a precise reviewable claim or relation candidate.
- Support can be checked against the referenced span without ambiguity.
- No review blocker or repair-required state is present.
- A future negative or positive import rehearsal is explicitly scoped before production writes.

### `retrieval_only`

Use when:

- Source material is useful for search/retrieval.
- Chunk or route metadata lacks enough provenance for trusted fact import.
- The target may support later review but must not create KG facts now.

### `repair_required`

Use when:

- Chunk boundaries, source spans, parent references, route labels, or source conversion quality need repair before semantic review can proceed.
- The artifact contract is missing fields needed for supportability review.

### `reject`

Use when:

- The source cannot support review.
- Required source path/hash is missing.
- The target contains an explicit safety blocker.
- The target would require embedding prohibited raw text or claim text to justify import readiness.

## Review dimensions

Each target receives categorical judgments for:

1. `source_provenance`: whether source path and SHA256 hash are present.
2. `span_provenance`: whether chunk-level spans or equivalent source locators are present.
3. `chunk_boundary_quality`: whether aggregate metadata suggests chunk boundaries are normal, outlier, or unknown.
4. `claim_supportability`: whether the artifacts contain enough redacted evidence to review a candidate fact.
5. `route_readiness`: whether route/state/refusal counts indicate import readiness or review blockers.
6. `import_recommendation`: one of the classification labels above.

## M011 S02 expected conservative rule

For M010-derived targets, `span_provenance` is expected to be `missing_chunk_spans` because M010 diagnostics are paper-level aggregate records. Therefore, no target may be classified as `import_candidate` in S02 unless additional chunk-span evidence is produced without violating the redaction boundary.

## Safety invariant

S02 may recommend future work, but must not create trusted KG facts, perform positive import, or write to production LadybugDB.
