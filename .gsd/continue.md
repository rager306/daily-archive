# Handoff — 2026-08-11 (M376 sealed)

## Completed

M371-M376 complete. Latest: **M376-y7um7o Revision Aware Article Query**.

## Product operator

```bash
# Build/prove accepted revision graph (M375)
cargo run -p da-cli --example revision_graph_canary

# Query durable snapshot; selector = current | history | vN
cargo run -p da-cli -- revision-query \
  --id 2506.07853 \
  --selector history \
  --snapshot artifacts/article_context/revision_graph_2506.07853.sgsnap \
  --json
```

## M376 delivered

Commit `7ea36bc3`:
- typed read-only `RevisionQueryUseCase` in da-application;
- selectors: `current`, exact positive `vN`, numeric `history`;
- response: Paper/revision/source identities, current, predecessor, Section count, manifestations, D127;
- typed Paper/revision/current/ambiguous/invalid-graph errors;
- dedicated CLI `revision-query`, human and clean JSON output;
- invalid selector exit 2, query failure exit 3;
- quiet snapshot preload for machine JSON;
- architecture schema audit corrected to workspace-aware ADR-050 production DATA;
- temporal SUPERSEDES fixture corrected to mandatory temporal stamp helper.

Real committed M375 snapshot proof:
- history: v1 current=false Sections=14 manifestations=2 predecessor=None;
- history: v5 current=true Sections=11 manifestations=2 predecessor=v1;
- current → v5;
- exact v1/v5 → correct revision;
- v3 → exit 3, no fallback;
- latest → exit 2, no implicit latest;
- snapshot SHA before/after unchanged: `9e5c2b8a168cbcb3581d352d8bfae4ebd8f18e301261894263a290b4547c93b0`;
- D127 false.

Verification:
- revision query 4 tests;
- schema audit 2;
- da-cli 72;
- all-target clippy PASS;
- governor 33, YAML valid, violations=0;
- full architecture guardrail PASS;
- logs clean;
- UAT/validation/closeout PASS.

GSD auto-closeout commit: `3c10b5ba`.

## Next product waves

1. Extraction-stage PipelineCache with exact rule/config/output-schema fingerprints and honest LoadReport records.
2. Review queued M363-ma2kmw relevance before spending on its stale roadmap.
3. Full remote GraphStore remains a future integration; revision-query currently uses same-process snapshot import intentionally.
4. ADR-057 remains Proposed.

## Locks

D127 false; ADR-057 Proposed; ADR-058 Accepted. Current revision must come from exactly one `is_current`, never insertion order or implicit latest. Query path is read-only; cache/catalog evidence remains provenance, not graph truth.
