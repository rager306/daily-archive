# M041 Mixed Connectivity Smoke Report

## Selection

- Articles: 20
- Category counts: {'baseline': 10, 'hermes_review_section': 5, 'reference_linked': 5}
- Hermes review candidates discovered: 13
- Hermes review-section articles used: 5
- Reference candidates discovered from already loaded sources: 69
- Reference-linked articles used: 5
- Fresh articles used: 0

## Runtime audit

- Completed handoffs: 20/20
- Continuity artifacts present: 20/20
- Source refs present: 20/20
- Loader refs present: 5/20
- Explicit loader absence: 15/20
- Blockers for import: []

## Safety

- graph_write_allowed=false
- promotion_allowed=false
- production_import_attempted=false
- import_eligible=false

## Interpretation

This mixed no-write smoke includes articles linked from already loaded sources and Hermes review-section articles. It tests early corpus connectivity and larger-batch continuity only. It does not authorize graph import, fact promotion, or production writes.

## Caveat

Some linked candidates may have deferred external metadata because arXiv rate-limited API access during selection. The deferred status is explicit in local article records and remains metadata-only.
