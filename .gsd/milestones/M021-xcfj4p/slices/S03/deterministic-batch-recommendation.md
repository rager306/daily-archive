# M021 S03 deterministic batch recommendation

## Verdict

```text
PROCEED_TO_S04_FINAL_REVIEW
DEFER_POSITIVE_IMPORT_GATE
```

## Evidence

The deterministic candidate locator module generated a bounded batch over the 10 M011 semantic review targets after S04 review remediation for stable span hashes and overlap diagnostics.

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

Compared with M020's hand-built rehearsal:

```text
m020_locator_count=35
m021_locator_count=26
m020_ambiguous_span_count=27
m021_ambiguous_span_count=20
```

The reduction comes from deterministic route filtering against M011 `counts_by_route` metadata. The remaining ambiguity is now better explained because overlap diagnostics are present.

## Recommendation

Proceed to final S04 closeout with these conclusions:

1. Deterministic route filtering made the locator output more reproducible and less noisy than M020.
2. Stable span hashes and `overlapping_signal_window` diagnostics address the two concrete independent-review gaps.
3. The next KG milestone should focus on chunk/section structure repair plus reviewer packets, not positive import.
4. There is no basis for positive import work yet.

## Safety state

```text
production_import_attempted=false
ladybugdb_written=false
trusted_kg_import_allowed=false
semantic_kg_readiness_claimed=false
```
