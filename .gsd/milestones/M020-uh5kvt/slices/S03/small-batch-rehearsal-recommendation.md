# M020 S03 small-batch rehearsal recommendation

## Verdict

```text
PROCEED_TO_S04_INDEPENDENT_SEMANTIC_REVIEW
```

## Guard result

```text
m020-s03-small-batch-guard-ok
```

## Batch metrics

```text
paper_count=10
locator_count=35
missing_span_count=0
ambiguous_span_count=27
conflicting_evidence_count=0
repair_required_count=0
import_eligible_count=0
promoted_to_fact_count=0
```

## Interpretation

The S01 locator protocol scales from the S02 one-paper fixture to the full bounded M011 target batch. All source artifacts were represented through source ledgers and locator spans, and the guard confirmed schema conformity, redaction boundaries, import-disabled semantics, and required failure-mode metrics.

The high ambiguous-span count is useful evidence, not a failure. It means the current heuristic locator generation can often identify coordinate-bearing source regions but cannot yet disambiguate semantic support well enough for import. That is exactly the condition S04 independent review should inspect.

## Recommendation

Proceed to S04 independent semantic review with these questions:

1. Are the coordinate-bearing locators meaningful enough to support a future positive import-gate milestone after refinement?
2. Is ambiguity primarily caused by broad keyword spans, conversion structure, missing chunk semantics, or protocol gaps?
3. Should the next milestone implement deterministic locator code, improve chunk segmentation, or add reviewer UI/packets first?
4. Which locator classes are safe to keep as retrieval-only versus candidates for future fact review?

## Safety state

Do not proceed to positive import from S03 alone.

Required state remains:

```text
production_import_attempted=false
ladybugdb_written=false
trusted_kg_import_allowed=false
semantic_kg_readiness_claimed=false
```
