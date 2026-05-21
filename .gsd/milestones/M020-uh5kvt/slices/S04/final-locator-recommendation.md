# M020 final locator recommendation

## Verdict

```text
DEFER_POSITIVE_IMPORT_GATE
PROCEED_WITH_DETERMINISTIC_LOCATOR_IMPLEMENTATION_PLUS_AMBIGUITY_DIAGNOSTICS
```

## Why

M020 successfully created and tested a candidate locator protocol with source ledgers, redacted source coordinates, review states, and safety guards. The protocol scaled from one paper to the bounded M011 target batch.

However, independent review returned `FLAG` because the small-batch rehearsal has high ambiguity:

```text
paper_count=10
locator_count=35
ambiguous_span_count=27
missing_span_count=0
conflicting_evidence_count=0
import_eligible_count=0
promoted_to_fact_count=0
```

This means locator shape and redaction are ready for deterministic implementation, but semantic support is not ready for positive KG import.

## Recommended next milestone

```text
Deterministic Candidate Locator Implementation and Ambiguity Diagnostics
```

Suggested order:

1. Implement protocol-backed locator generation in code with schema validation.
2. Preserve import-disabled outputs and safety guards.
3. Add ambiguity diagnostics that separate broad keyword spans, chunking/conversion structure problems, missing source hashes, and candidate-type uncertainty.
4. Re-run on the M011 bounded batch.
5. Only after independent semantic review improves precision, consider a separate positive import-gate milestone.

## Still blocked

```text
positive_kg_import=false
production_ladybugdb_writes=false
trusted_kg_import_allowed=false
semantic_kg_readiness_claimed=false
vector_retrieval_claims=false
autonomous_scientist_behavior=false
minimax_source_of_truth=false
```

## R048 outcome

R048 should be validated for protocol definition and bounded rehearsal evidence, with a note that positive import remains deferred pending deterministic implementation and lower ambiguity.
