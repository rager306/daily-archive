# Handoff — 2026-08-11 (M375 sealed)

## Completed

M371-M375 complete. Latest: **M375-h6s6bp Revision Aware Article Graph**.

## Product delivered

- ADR-058 Accepted (D223): one canonical Paper, immutable PaperRevision, content-addressed SourceManifestation, revision-scoped Sections, observed SUPERSEDES.
- Production DATA and compiled endpoint parity active; additive VIDs preserve existing `paper_vid`.
- Versioned arXiv locator/acquisition/cache from M374 flows into unified `RevisionContentContext` across load, ingest, extraction, and graph materialization.
- Versioned mutable properties/vectors live on PaperRevision; extracted FOUND_IN resolves revision Sections. Unversioned wrappers retain legacy behavior.
- SUPERSEDES is load-order independent and adjacent-observed; inserting an intermediate revision invalidates stale direct fact.
- SourceManifestation has no single revision owner; revisions link shared content-addressed bytes via HAS_MANIFESTATION.

## Real Samyama proof

```bash
cargo run -p da-cli --example revision_graph_canary
cargo run -p da-cli -- graph-stats --snapshot \
  artifacts/article_context/revision_graph_2506.07853.sgsnap
```

Durable artifacts:
- `artifacts/article_context/revision_graph_2506.07853.report.json`
- `artifacts/article_context/revision_graph_2506.07853.sgsnap`

PASS:
- one Paper;
- v1 + v5 PaperRevision;
- exact PDF/HTML SHA, two manifestations each;
- v1 Sections=14, v5 Sections=11 (current HtmlParser observation);
- current=v5;
- v5 SUPERSEDES v1;
- physical replay: property attempted=560, unchanged=560, applied=0, rejected=0, unobserved=0;
- snapshot 29,554 bytes, sha `9e5c2b8a…`, reload 33 nodes/33 edges;
- D127 false.

## Verification

- da-application: 126 tests
- revision context/materializer: 9 tests
- da-cli load: 7 tests
- VID: 21 tests/properties
- registry 14 / endpoints 16
- governor 33, YAML valid, 0 violations
- full architecture guardrail PASS
- app logs: no ERROR/panic
- M375 UAT/validation/closeout PASS

Commits:
- `f7c1e079` accepted ontology DATA/VIDs
- `39c9d6b1` compiled endpoint parity
- `b85c14ba` dedicated revision materializer
- `fc21b39d` unified revision section provenance
- `3972b24e` real two-revision Samyama proof
- `e125aa8a` GSD closeout auto-commit

## Next product waves

1. Revision-aware query operator/API (`current`, explicit `vN`, as-of observed history) over loaded snapshots.
2. Extraction-stage PipelineCache with exact rules/config/output-schema identity.
3. Review queued M363-ma2kmw relevance before executing its stale roadmap.
4. ADR-057 remains Proposed and requires separate explicit acceptance.

## Locks

D127 false; ADR-057 Proposed; ADR-058 Accepted. Do not change existing `paper_vid`; do not infer current by insertion order; cache/catalog evidence is provenance, not authoritative graph truth.
