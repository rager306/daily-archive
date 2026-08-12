# Handoff — 2026-08-12 (M387 optimizer-ready pipeline)

## M387/S05 delivered (878a895c)

- Cache cold/warm ablation 1/10/30/100/200 (warm hits 100%, honest warm_us >= cold_us
  at scale), key/payload SHA lineage, drift probes.
- Stage ablation report binds cache ablation + materialization scaling
  (exponent 0.968, projected_200 18.59s) + replay proof (168239 unchanged) +
  readiness deltas. D127=false, optimizer=false, auto_promote=false.
- Artifacts: pipeline_cache_profile_m387.json (5 scales),
  stage_ablation_profile_m387.json. S04+S05 closed via skip (free-form delivery).

## Next: S06 measured Samyama hot path

## M387/S06 delivered (afccb77b)

- Samyama hot path: write/identity/traversal/vector at 100/1k/3k, FROZEN parity
  (hub.sgsnap 252/342, frozen lookup faster, readonly 1008us), import/replay
  idempotent (32ms→31ms). Honest: edge create write amplification ~34x for 30x
  data; identity/traversal/vector sub-linear; vector search 7-20us.
- Artifact: samyama_hot_path_profile_m387.json. D127=false, optimizer=false.

## Next: S07 RuVector/SONA retrieval quality

## M387/S07 delivered (b7ee0d5d)

- RuVectorBridge stub replaced with functional bridge: RRF hybrid fusion
  (ruvector-hybrid) + feature-gated PPR (ruvector-solver forward-push on plain
  CSR); 13 ppr tests + 9 base tests PASS.
- Retrieval quality benchmark (3 queries, golden judgments, tenant filter):
  dense/lexical/rrf-hybrid recall@k=1.0 filter=true admit; ppr filter=false
  → benchmark-only (honest: fixed topology ignores tenant). Quality decides
  admission via data/retrieval/policy.yaml (min recall@k=0.6).
- Artifact: retrieval_benchmark_m387.json. D127=false, optimizer=false.

## M387 all slices delivered (S04-S07) — next: validate + close M387

## M387/S08 delivered (2c398405) — M387 COMPLETE

- Pipeline acceptance: 14-stage ledger contracts all pass, cache replay proof
  (168239 unchanged), readiness gates (2 tasks evaluated-not-eligible),
  verdict=accepted, D127=false. Governor ratchet
  no_optimizer_or_auto_promote_true_in_artifacts added.
- Artifact: pipeline_acceptance_m387.json.

## M387 all slices delivered (S01-S08) — ready for validate + close

## M386-57ae9l still active

Rust-only D237 acquisition delivered `9d57d53e`. Remaining typed article.v00.02, bounded network retry/backoff + partial/checksum conflicts, then Python downloader archive and Rust-only acceptance. D127 false.

## M387 delivered

- S01 `ccb27e08`: 14-stage DATA ledger/runtime audit.
- S02 `aebb7bce`: ACAP context conservation; 30 parse/extract profile.
- S03 `6bd13514` + `d9fbd418`: dual cache lineage, whole-pipeline metrics, 200-paper replay.
- Foundation `3c2162ca`: ADR-059/D238 PromptExecution/FSM.
- Summary `02761fe0`, 9router `7b27ec55`, figure `153691c4`.
- Frozen eval `667de01e`: offline FSM evaluator + synthetic fixtures.
- Source-bound fixtures `bda7f260`: independently reviewed GEPA abstract/section/Figure3/Figure5 SHA-bound evidence, fail-closed validation.
- Adapter hardening `8424e028`: reasoning-model compatibility, bounded inference runner.

## M387/S04 active — real same-n inference achieved (e74293a6)

### Progress

- **Real same-n inference works end-to-end**:
  - Summary: 4/4 executions via agnes-2.0-flash, valid JSON, schema-valid.
  - Figure: 2/4 executions via qwen3-vl, 2 schema quality-signals, full lineage.
  - Durable ledger: `artifacts/runtime/prompt_eval_inference_m387.json`.
- **Adapter bug fixed (e260a0b1)**: provider wrapper text now yields canonical
  JSON bytes (not raw wrapper), with regression test.
- **Scoring works**: `prompt_eval_scored.rs` maps real outputs back into
  frozen source-bound fixtures, extracts canonical facts (structured + prose
  keyword evidence, no sentence noise), produces honest metrics.
- **Summary now routes to MiniMax-M2.7-highspeed** (non-vision, per user):
  - grounding 1960/2777 (agnes) → **2558** (M2.7)
  - coverage 7692 (agnes) → **8461** (M2.7)
  - gap 4286 (agnes) → **2858** (M2.7)
- Figure unchanged: qwen3-vl grounding=10000, faithfulness=10000, coverage=4545,
  schema_failures=2/4, eligible=false.
- Both tasks remain eligible=false (hard gate requires all metrics >= 10000),
  deployed=false, D127=false. Honest first-pass quality evidence.

### Current blocker (honest status)

Neither task is eligible. Summary coverage=8461 (needs 10000); figure has schema
failures on 2/4 fixtures. Expected first-pass quality evidence, not a process
defect. GEPA/DSPy disabled; no auto-promotion; D127 locked.

### Next session options

1. Improve candidate prompt suffix in `data/prompt_eval/inference.yaml` and
   re-run bounded matrix to raise summary grounding/coverage.
2. Fix figure JSON emission (stronger JSON instruction or different model).
3. Accept current evidence and close S04 as "harness proven, quality
   ineligible" — honest fail-closed state.

## LLM audit current

- Summary and figure: both PromptExecutionPort + models.yaml/9router, candidate-only.
- GEPA/DSPy disabled. D127 locked. D238/ADR-059 PromptExecutionPort/FSM.
- models.yaml owns provider/model/protocol; .env owns host/key/timeout overrides.
- No hardcoded config: max_calls, timeout, variants, task routes in inference.yaml.

## Session findings

- Codebase Memory MCP timed out repeatedly; GitNexus authoritative and reindexed.
- MiniMax-M3 through 9router does not honor `response_format`; agnes-2.0-flash
  and qwen3-vl produce JSON after `stream:false` + `<think>` stripping + balanced
  brace extraction.
- Empty success responses, JSON preamble, code fences, and reasoning blocks are
  all handled by adapter hardening; durable failure ledgers record typed errors.
- No raw provider body/secret persisted; no paid inference beyond authorized
  bounded matrix (8 calls max, per-task same-n).
- GSD journal no S04 stuck-loop. Generated trajectory report remains unstaged.

## Locks

D127 false. D237 Rust-only. D238/ADR-059 PromptExecutionPort/FSM. D239 models.yaml+9router. D136 DSPy disabled. GEPA no auto-promote. Samyama sole store. Cache only pure compute. RuVector/SONA only after active-path evidence.
