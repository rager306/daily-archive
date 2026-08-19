# Continue — M402-jqs0xg code delivered (GSD seal pending Attempt)

## Last action

M402-jqs0xg “ACAP ledger isomorphism and linear article spine gate” is
**implemented and verified** in this free-form session. GSD
`gsd_task_complete` cannot seal rows (no running Attempt). Do not treat
`/gsd auto` as the only remaining work.

Delivered:

- S01: optional `acap_layer` on `PipelineStageContract`; registry lookup;
  YAML layers for the twelve historically gated stages; orchestrator
  dropped `acap_state_for_stage`.
- S02: `revision_materialization` → `structured`,
  `extraction_materialization` → `evidence_linked`; linear spine journey
  admits and cache-hit replays (`linear_article_spine_admits_materialization_and_replays_from_cache`).
- S03: Governor `ledger_layers` + forbidden `fn acap_state_for_stage`;
  ADR-055 amended (D276); residuals `materialized` / `enriched` /
  `finalized` / `task_accepted` are typed `no_claim`.

R096 recorded. D127 remains locked. Optimizer auto-promotion remains disabled.

## Next action

Highest-leverage remaining gap from the pipeline audit:

1. **One production journey** `acquire → catalog → parse → revision_materialize → extract_{compute,materialize} → [multimodal] → summary → assurance → checkpoint` through `PipelineOrchestrator` (operator continuity).
2. Table / equation / candidate as real `PipelineStageExecutor`s (M400 proved candidates; they are not in `compose_multimodal_production`).
3. Scheduler executors for Ingest / Reparse / Extract / Embed / GraphWrite (only OpenAlexEnrich is wired).
4. P1 full remote GraphStore (`DA_GRAPH_MODE=remote` is still `NotConfigured`).

To seal M402 in GSD later: skip-delivered + runtime UAT + sha256 validate
(M306 pattern / MEM567), or `/gsd auto M402-jqs0xg` if Attempt claim is available.

## Verification snapshot

- `cargo test -p da-domain --lib pipeline_ledger`: 11 PASS
- `cargo test -p da-adapters --lib pipeline_ledger`: 2 PASS
- `cargo test -p da-application --lib pipeline`: 39 PASS (includes spine)
- `cargo test -p da-application --test acap_control_plane_test`: 3 PASS
- `cargo test -p da-application --test multimodal_pipeline_test`: 2 PASS
- Governor self-test: 74/74 PASS
- Governor `--phase preflight` and `--phase active --check-only`: violations=0

## Do not

Hand-edit `.gsd` planning artifacts or `gsd.db`. Unlock D127 without
explicit user approval. Enable optimizer/deployment auto-promotion.
Invent `acap_layer` for `materialized` / `enriched` / `finalized` /
`task_accepted`. Run workspace-wide cargo tests (rocksdb trap). Use
mutating Governor `--phase active` in task verification.
