# M021 final deterministic locator recommendation

## Verdict

```text
VALIDATE_R049_FOR_DETERMINISTIC_LOCATOR_IMPLEMENTATION
DEFER_POSITIVE_IMPORT_GATE
NEXT_CHUNK_STRUCTURE_REPAIR_AND_REVIEWER_PACKETS
```

## What changed after independent review

Independent review initially returned `FLAG` for two concrete implementation gaps:

1. path-dependent `span_hash` values;
2. missing `overlapping_signal_window` diagnostics.

Both were fixed before final closeout:

- `span_hash` now uses stable provenance fields: source ID, source hash, coordinate space, offsets, and route name;
- overlap diagnostics are added by a coordinate-only pass over locators from the same source;
- regression tests cover both behaviors.

## Final deterministic batch metrics

```text
paper_count=10
source_count=10
locator_count=26
ambiguous_span_count=20
missing_span_count=0
conflicting_evidence_count=0
retrieval_only_count=6
overlapping_signal_window_count=10
import_eligible_count=0
promoted_to_fact_count=0
```

Compared with M020:

```text
m020_locator_count=35
m021_locator_count=26
m020_ambiguous_span_count=27
m021_ambiguous_span_count=20
```

## Interpretation

M021 successfully moved candidate locators from hand-built protocol artifacts into deterministic, tested code. Route filtering reduced noise, stable span hashes improved reproducibility, and overlap diagnostics explain an important ambiguity class.

This is still not semantic KG readiness. The output remains review-only and import-disabled.

## Recommended next milestone

```text
Chunk Structure Repair and Reviewer Packet Prototype
```

Recommended scope:

1. Improve chunk/section structure so locator windows are narrower and less overlapping.
2. Generate compact reviewer packets from deterministic locator output.
3. Review whether ambiguity drops enough to justify a later semantic acceptance gate.
4. Keep positive import and LadybugDB writes blocked.

## Still blocked

```text
positive_kg_import=false
production_ladybugdb_writes=false
trusted_kg_import_allowed=false
semantic_kg_readiness_claimed=false
vector_retrieval_claims=false
minimax_source_of_truth=false
```
