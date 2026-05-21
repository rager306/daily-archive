# M021 S03 deterministic locator batch report

## Scope

Deterministic run over M011 semantic review targets using `arxiv_archive.candidate_locators`.

```text
paper_count=10
source_count=10
locator_count=26
located_count=26
missing_span_count=0
ambiguous_span_count=20
conflicting_evidence_count=0
retrieval_only_count=6
import_eligible_count=0
promoted_to_fact_count=0
```

## State counts

```text
{'ambiguous_span': 20, 'retrieval_only': 6}
```

## Diagnostic counts

```text
{'broad_signal_many_matches': 19, 'overlapping_signal_window': 10, 'review_required': 7}
```

## Interpretation

The deterministic module reproduces a bounded candidate locator batch from existing M011 targets with source hash checks, stable coordinate span hashes, route metadata filtering, overlap diagnostics, recursive safety validation, and explicit ambiguity diagnostics.

The output remains review-only. It is not a positive import gate.
