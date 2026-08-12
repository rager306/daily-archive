# Handoff — 2026-08-12 (M387 optimizer-ready pipeline)

## M386-57ae9l still active

Rust-only D237 acquisition delivered `9d57d53e`. Remaining typed article.v00.02, bounded network retry/backoff + partial/checksum conflicts, then Python downloader archive and Rust-only acceptance. D127 false.

## M387 delivered

- S01 `ccb27e08`: 14-stage DATA ledger/runtime audit.
- S02 `aebb7bce`: ACAP context conservation; 30 parse/extract profile, 140 candidates/117 grounded, 12 no_claim.
- S03 `6bd13514` + `d9fbd418`: dual cache lineage, whole-pipeline metrics, real 1/10/30 cache profile and 200-paper replay; UAT PASS.

## M387/S04 active

Foundation `3c2162ca`: ADR-059/D238, PromptTask/Lineage/Evaluation/Candidate FSM, PromptExecutionPort, strict prompt task DATA, optimizer disabled.

Summary authority migration `02761fe0`: `da summarize` uses PromptExecutionPort; exact prompt/schema/route/input/output lineage; nonretrieval/non-authoritative/D127-false Summary candidates only; no LLM Method/Dataset/Metric/QualitySignal authority; real Samyama canary PASS.

### 9router correction delivered `7b27ec55` (D239)

The prior Anthropic env description was wrong and is removed from summary routing.

- `models.yaml` is canonical for model/provider/protocol roles.
- `paper-summary-generate-default` resolves to `agnes-ai/agnes-2.0-flash` through 9router OpenAI-compatible `/v1/chat/completions`.
- Separate existing summary roles remain: quality `minimax/MiniMax-M2.7-highspeed`, fallback `xai/grok-code-fast-1`, primary judge `glm/glm-5.2`, secondary judge `cx/gpt-5.4-mini`.
- Added `figure-qa-primary` binding to present 9router model `novita/qwen/qwen3-vl-235b-a22b-instruct`; previous remembered Qwen IDs were absent from live inventory.
- `.env` now supplies only `NINEROUTER_URL`, optional `NINEROUTER_KEY`, `DA_LLM_TIMEOUT_SECONDS`, `DA_PROMPT_TASK_REGISTRY`, `DA_MODEL_REGISTRY` for summary composition.
- Added strict typed `models.yaml` loader: referential bindings, provider/protocol mapping, HTTPS/local-HTTP policy, bounded 9router-only endpoint override, registry digest.
- Added functional OpenAI-compatible Rust LLM client: bounded timeout, optional bearer auth, content/reasoning-content response handling, token accounting, model mismatch rejection, hash-only HTTP/JSON errors.
- Prompt route now carries `model_binding_id`; request carries `ResolvedModelBinding`; lineage and Summary nodes carry `model_binding_sha256` plus endpoint/model/prompt/schema/input/output identity.
- Prompt registry and model registry are cross-validated before execution.
- Live read-only discovery: `GET http://127.0.0.1:20128/v1/models` HTTP 200 in ~6.0s, 592 models. Six required summary+figure bindings confirmed present. No inference was issued.
- Durable evidence `artifacts/runtime/ninerouter_model_inventory_m387.json`: response/report SHA, 592 count, six bindings, inference_requested=false, optimizer_enabled=false, D127=false.
- Added Governor high-severity ratchet: prompt task DATA may not reintroduce ANTHROPIC_BASE_URL/ANTHROPIC_MODEL or anthropic-text/multimodal routes; 35 Governor tests PASS.
- Corrected stale models.yaml validation comment to real Rust test; canonical Samyama canary rebuilt after model-binding lineage change.

Verification: 14 focused tests including local ephemeral OpenAI-compatible HTTP roundtrip; CLI all-target check; per-crate Clippy; full architecture PASS; Governor drift_risk/0 violations; GitNexus HIGH expected and reindexed; live inventory proof PASS.

### Exact next action S04/T03 and T04

1. Migrate `FigureAnalysisUseCase` to PromptExecutionPort with typed media. Use `figure-qa-primary` 9router binding; preserve candidate-only authority and exact image digest.
2. Ensure figure JSON schema exactly matches `FigureAnalysisCandidate`; cache key includes prompt task/schema/route/resolved model binding; add provider/route/media/schema negatives.
3. Implement frozen offline summary+figure eval FSM: independent fixtures, schema/grounding/faithfulness/coverage/paraphrase/latency/tokens/cost; same-n, val_gap, contamination and budget gates; candidate reports only; GEPA/DSPy optimizer false.
4. Close S04 only after runtime UAT/full gate. Then add call-site Governor ratchet prohibiting raw LLM bypasses.

## LLM audit current

- Summary: PromptExecutionPort + 9router registry, safe candidate-only.
- Figure: still MultimodalLlmPort, candidate-only with substantial lineage; migration pending.
- Not generative: RuleBasedExtractor, GROBID, extractive summary, FD_API embedding, MCP prompt templates.
- RuVector/SONA are retrieval/graph optimization surfaces, not prompt optimizers; S07 recall/MRR/nDCG/filter/latency and active-path gate remains.

## Session findings

- Codebase Memory MCP timed out; GitNexus authoritative and reindexed.
- Live 9router inventory disproved two remembered Qwen model IDs; config was corrected to an actually present VL model before commit.
- No secret or provider response body was logged/persisted; inventory persisted only IDs/presence/count/digests.
- GSD/app logs show no stuck-loop; generated `artifacts/project-trajectory/trajectory-report.json` remains unstaged.

## Locks

D127 false. D237 Rust-only. D238/ADR-059 PromptExecutionPort/FSM. D239 models.yaml+9router routing. D136 DSPy disabled. GEPA no auto-promote/dual F1+same-n+val_gap. Samyama sole store. Cache only pure compute; graph materialization always runs. RuVector/SONA/RocksDB only after active-path evidence.
