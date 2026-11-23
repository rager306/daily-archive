# Functionality ledger (M001 orientation/assessment)

Assessment snapshot at base `3031bf3ec` (branch `gsd-path-task/T003`). Method: static reading +
codebase-memory-mcp + recorded logs only (`/tmp/arch-20261123-pass2.log`,
`/tmp/preflight-20261122.log`); **no new cargo/test runs were performed for this report**. This is
assessment only: no gate flip, no pin, no threshold, no status value changes (N1 — diff is
.md-only). Verdict vocabulary is exactly {работает / кандидат / заглушка / unverified}, one verdict
per enumerated item, each with a proof-weight tag (ran-in-gate / static-only / unverified) and an
evidence path that resolves at HEAD.

**Execution plane of record (U1).** GitHub CI executes the workflow
`.github/workflows/architecture-guardrail.yml` (lines 95–105): `--lib` tests for kg-*, da-domain,
da-parse, da-mcp and `--tests` for da-application, da-adapters, da-cli. For the da-parse parse
plane this means **only `cargo test -p da-parse --lib` runs in CI** (line 101) — the 146 da-parse
integration suites are **not CI-protected**; the local governor gate is the only recorded execution
plane for them (23/23 PASS, 0 timed out: `/tmp/arch-20261123-pass2.log`, lines
`architecture gate steps: 23/23 passed` / `ARCHITECTURE GATE: PASS`). No suite below is claimed as
CI-protected; all execution evidence below cites the recorded local gate log or explicit CI
workflow lines.

**Enumeration method (E4 — closed over the dispatch match).** The command surface was enumerated
from the `match cli.command` dispatch in `crates/da-cli/src/main.rs` (all arms, including
help-only/internal arms), never from README or historical ledgers. Count rendered from code:
**38 top-level arms**, of which 4 carry nested subcommand enums (`source`, `policy`, `agent-run`,
`parse-job`) with **32 nested leaves** — 24 `source` actions (main.rs lines 574–635), 2 `policy`
actions (main.rs lines 536–553, 1144–1236), 2 `agent-run` subcommands
(`crates/da-cli/src/agent_run.rs`), 4 `parse-job` subcommands
(`crates/da-cli/src/parse_job.rs`, lines 20–31). No dispatch entry is omitted; there are no
`#[command(hide)]` arms in `crates/da-cli/src/main.rs` (grep over the file at HEAD).

Pairing rule: each command is paired with the da-application use-case(s) its dispatch arm calls
(citation given); a command with no use-case row still gets a ledger row ("none — self-contained").

---

## 1. CLI dispatch enumeration (38 arms)

### 1a. Top-level commands

| Command (clap arm) | da-application use-case | Verdict | Proof weight | Evidence |
|---|---|---|---|---|
| `policy` | `parse_policy_table`, `parse_policy_health` | кандидат | static-only (code read; no recorded CLI run) | `crates/da-cli/src/main.rs` (lines 1144-1236), `crates/da-application/src/parse_policy_table.rs`, `crates/da-application/src/parse_policy_health.rs` |
| `source` | 24 nested actions — see §1b | кандидат | static-only (dispatch read; actions individually tagged in §1b) | `crates/da-cli/src/main.rs` (lines 553-645) |
| `validate-objects` | `validate_schema_objects` | кандидат | static-only (code read; no recorded run) | `crates/da-cli/src/main.rs` (lines 646-692), `crates/da-application/src/validate_schema_objects.rs` |
| `health` | none — self-contained probe | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 693-697), `crates/da-cli/src/health_report.rs` |
| `version` | none — static printer | кандидат | static-only (prints hardcoded banner, not Cargo version) | `crates/da-cli/src/main.rs` (lines 698-702) |
| `ingest` | `ingest` (`IngestUseCase`) | кандидат | static-only (D127 import locked by default; unlock flag honored) | `crates/da-cli/src/main.rs` (lines 703-715 and 1305-1404), `crates/da-application/src/ingest.rs` |
| `acquire-arxiv` | `acquisition_batch` (`BatchAcquisitionUseCase`) | кандидат | static-only (code read) | `crates/da-cli/src/acquire_arxiv.rs` (line 11), `crates/da-application/src/acquisition_batch.rs` |
| `batch-ingest` | `batch_ingest` | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 723-740), `crates/da-application/src/batch_ingest.rs` |
| `load-snapshot` | none — snapshot restore | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 741-745) |
| `snapshot-identity` | none — digest computation | кандидат | static-only (code read) | `crates/da-cli/src/snapshot_identity.rs` |
| `graph-stats` | none — count report | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 752-756) |
| `query` | none — da-graph builders | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 757-766) |
| `graph-query` | none — provider-neutral scalar read | кандидат | static-only (code read) | `crates/da-cli/src/graph_query.rs` |
| `agent-run` | `agent_run` (`AgentRunUseCase`) | кандидат | static-only (code read; run needs live serve-graph daemon) | `crates/da-cli/src/agent_run.rs`, `crates/da-application/src/agent_run.rs` |
| `revision-query` | `revision_query` | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 803-812), `crates/da-application/src/revision_query.rs` |
| `extract` | `extraction` (`ExtractionUseCase`) | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 813-821), `crates/da-application/src/extraction.rs` |
| `summarize-revisions` | `extractive_summary` | кандидат | static-only (code read; no-LLM by design, M383 S03) | `crates/da-cli/src/summarize_revisions.rs` (line 10), `crates/da-application/src/extractive_summary.rs` |
| `summarize` | `summary` (`SummaryUseCase`) | кандидат | static-only (code read; requires FD_API_KEY per `require_fd_api_key_for_ingest` pattern) | `crates/da-cli/src/main.rs` (lines 833-841), `crates/da-application/src/summary.rs` |
| `schema-init` | `schema_init` | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 842-844), `crates/da-application/src/schema_init.rs` |
| `heal` | `healing` (`GraphHealingUseCase`) | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 845-870), `crates/da-application/src/healing.rs` |
| `enrich` | `enrich` (`EnrichUseCase`) | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 871-879), `crates/da-application/src/enrich.rs` |
| `batch-enrich` | `enrich` (batch loop over `EnrichUseCase`) | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 880-884) |
| `scheduler-run` | `scheduler` (`GraphScheduler`), `scheduler_runner`, `scheduler_executors` | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 885-892), `crates/da-application/src/scheduler.rs`, `crates/da-application/src/scheduler_runner.rs`, `crates/da-application/src/scheduler_executors.rs` |
| `schema-check` | none — static audit vs `da_domain` registry | кандидат | static-only (code read; ADR-045 Wave E) | `crates/da-cli/src/main.rs` (lines 893-895) |
| `edge-contracts` | none — registry printer | кандидат | static-only (renders `da_domain::edge_contract`) | `crates/da-cli/src/main.rs` (lines 896-899) |
| `schema-list` | none — registry printer (`--from-yaml` via OntologyRegistry) | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 900-915) |
| `prompt-registry` | none — read-only registry resolver | кандидат | static-only (non-zero exit on unresolved pair is implemented) | `crates/da-cli/src/main.rs` (lines 916-918) |
| `cross-refs` | none — registry printer | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 919-922) |
| `audit-fields` | none — static drift audit (ADR-045/MEM508) | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 923-925) |
| `validate-graph` | none — validator over live store | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 926-930) |
| `validate-node` | none — stdin snapshot validator | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 931-934) |
| `ontology-export` | none — YAML export (ADR-050 Phase D) | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 935-938) |
| `serve-graph` | none — kg-storage `ReadOnlyServer` | кандидат | static-only (code read; RESP read-only plane, mutations rejected server-side) | `crates/da-cli/src/serve_graph.rs`, `crates/da-cli/src/main.rs` (lines 939-945) |
| `graph-ping` | none — RESP probe | кандидат | static-only (code read) | `crates/da-cli/src/graph_ping.rs` |
| `analyze` | none — kg-algorithms projection (page-rank/ppr/wcc) | кандидат | static-only (code read) | `crates/da-cli/src/analyze.rs` |
| `load` | `article_load` (+ `catalog_pdf_locator`, `article_load_queue`) | кандидат | static-only (ADR-056 load plane; M461 fetch intent present) | `crates/da-cli/src/load.rs`, `crates/da-application/src/article_load.rs`, `crates/da-application/src/catalog_pdf_locator.rs` |
| `parse-batch` | parse composition (`parse_grammar`, `parse_policy_table`) over da-parse `ManagedParseBatch` | работает | ran-in-gate (governor gate `da-cli tests... OK` + `da-parse tests... OK`; bin-module comment in main.rs lines 26-33 states lib tests exercise the full composition surface) | `crates/da-cli/src/parse_batch.rs`, `/tmp/arch-20261123-pass2.log`, `crates/da-cli/src/main.rs` (lines 26-33) |
| `parse-job` | `parse_job_runner` (`ParseJobRunner`) | кандидат | static-only (crate-level suites ran in gate/CI; no recorded CLI-level run) | `crates/da-cli/src/parse_job.rs` (line 8), `crates/da-application/src/parse_job_runner.rs` |

### 1b. Nested leaves (32)

`source` — 24 actions, dispatched at `crates/da-cli/src/main.rs` (lines 557-635):

| `source` action | da-application use-case | Verdict | Proof weight | Evidence |
|---|---|---|---|---|
| `status` | none — FSM status read | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` |
| `plan` | none — FSM plan | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` |
| `import` | FSM import (dry-run default) | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` |
| `run` | full per-article pipeline | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` |
| `run-batch` | batch over incomplete-pipeline ids | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 563-580) |
| `acquire` | acquisition path | кандидат | static-only (code read) | `crates/da-cli/src/source_acquire.rs` |
| `build-navigation` | `navigation_stage` | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` (line 714), `crates/da-application/src/navigation_stage.rs` |
| `build-structural` | `structural_elements_stage` | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` (line 967), `crates/da-application/src/structural_elements_stage.rs` |
| `build-figures` | `figures_stage` | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` (line 1052), `crates/da-application/src/figures_stage.rs` |
| `build-figures-tei` | `tei_figures_stage` | кандидат | static-only (code read) | `crates/da-application/src/tei_figures_stage.rs` |
| `parse-html` | `parse_grammar` decision path | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` (lines 531-560) |
| `parse-pdf` | `parse_grammar` + sona-policy features | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` (lines 560-561) |
| `parse-pdf-toc` | `pdf_toc_stage` | кандидат | static-only (code read) | `crates/da-application/src/pdf_toc_stage.rs` |
| `enrich-layer` | `enriched_layer_stage` | кандидат | static-only (code read) | `crates/da-application/src/enriched_layer_stage.rs` |
| `audit-fidelity` | none — fidelity audit | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` |
| `sona-analyze` | `sona_bridge` | кандидат | static-only (code read) | `crates/da-cli/src/background_tasks.rs` (lines 417-426), `crates/da-application/src/sona_bridge.rs` |
| `sona-policy` | `sona_bridge` + `fsm_policy_consult` | кандидат | static-only (code read) | `crates/da-cli/src/background_tasks.rs` (line 471), `crates/da-application/src/fsm_policy_consult.rs` |
| `critic-repair` | `critic_repair_stage` | кандидат | static-only (code read) | `crates/da-cli/src/background_tasks.rs` (lines 529-564), `crates/da-application/src/critic_repair_stage.rs` |
| `memory-admit` | none — background admit | кандидат | static-only (code read) | `crates/da-cli/src/background_tasks.rs` |
| `background-run` | background task runner (per_section_summaries default) | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 613-622) |
| `fetch-openalex` | enrichment fetch path | кандидат | static-only (code read) | `crates/da-cli/src/main.rs` (lines 623-631) |
| `build-extracted` | `extraction_materialization_stage` | кандидат | static-only (code read) | `crates/da-application/src/extraction_materialization_stage.rs` |
| `build-summary` | summary build path | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` |
| `eval-frozen` | none — frozen eval | кандидат | static-only (code read) | `crates/da-cli/src/source_cli.rs` |

`policy` — 2 actions (`learn`, `health`), ADR-078 candidate-only surface: both кандидат,
static-only (code read), evidence `crates/da-cli/src/main.rs` (lines 1144-1236). The learned table
is candidate-only by design; nothing in this surface is production.

`agent-run` — 2 subcommands: `run` (live journey through a serve-graph daemon) and `verify`
(offline deterministic recomputation of the evidence chain, D273 contract) — both кандидат,
static-only (code read), evidence `crates/da-cli/src/agent_run.rs`, `crates/da-cli/src/main.rs`
(lines 775-802).

`parse-job` — 4 subcommands (`submit`, `run`, `status`, `cancel` at
`crates/da-cli/src/parse_job.rs`, lines 20-31): durable job admission + control plane over
`ParseJobRunner` (`crates/da-application/src/parse_job_runner.rs`) — кандидат, static-only
(crate-level suites ran in gate/CI; no recorded CLI-level run).

**Finding (help-string drift, static-only):** the `unknown source action` error message
(`crates/da-cli/src/main.rs`, lines 632-635) lists 17 actions but the dispatch match handles 24 —
`run-batch`, `parse-pdf`, `audit-fidelity`, `background-run`, `fetch-openalex`, `eval-frozen`,
`run` (listed) — several dispatched actions are missing from the hint text. Cosmetic; recorded,
not fixed here (N1).

### 1c. Заглушка sweep (closed)

No production dispatch path is a заглушка. The only `unimplemented!()` occurrences under
`crates/` are `#[cfg(test)]` mock-trait impls inside test modules (e.g.
`crates/da-application/src/cross_layer_signals.rs`, lines 105-139; same pattern in
`crates/da-application/src/enriched_layer_stage.rs`, `figures_stage.rs`, `navigation_stage.rs`,
`structural_elements_stage.rs`, `summaries_agent_stage.rs`, `tei_figures_stage.rs`). The
D127/production-route locks are deliberate fail-closed refusals, not stubs.

---

## 2. da-application use-case enumeration (closed over `lib.rs`)

`crates/da-application/src/lib.rs` declares **103 `pub mod` use-case modules** (104 files
including `lib.rs` itself); the enumeration below is closed over that list. Default verdict
rationale: every module compiles and its crate suite ran in the recorded gate
(`/tmp/arch-20261123-pass2.log`, `da-application tests... OK`; CI target
`.github/workflows/architecture-guardrail.yml` line 102), but **no recorded run exercises an
individual use-case end-to-end at CLI level**, so per N5 no module below claims «работает»
without named execution evidence — crate-level test execution alone is downgraded to кандидат.

| # | Use-case module | Paired command | Verdict | Proof weight | Evidence |
|---|---|---|---|---|---|
| 1 | `agent_run.rs` | agent-run | кандидат | static-only (code read) | `crates/da-application/src/agent_run.rs` |
| 2 | `article_load.rs` | load | кандидат | static-only (code read) | `crates/da-application/src/article_load.rs` |
| 3 | `article_load_queue.rs` | load | кандидат | static-only (code read) | `crates/da-application/src/article_load_queue.rs` |
| 4 | `artifact_stage.rs` | (pipeline stage) | кандидат | static-only (code read) | `crates/da-application/src/artifact_stage.rs` |
| 5 | `assurance.rs` | (pipeline stage) | кандидат | static-only (code read) | `crates/da-application/src/assurance.rs` |
| 6 | `candidate_materialization_stage.rs` | (load journey) | кандидат | static-only (code read) | `crates/da-application/src/candidate_materialization_stage.rs`, `crates/da-cli/src/load_journey.rs` (line 39) |
| 7 | `catalog_materialization.rs` | (catalog plane) | кандидат | static-only (code read) | `crates/da-application/src/catalog_materialization.rs` |
| 8 | `catalog_pdf_locator.rs` | load | кандидат | static-only (code read) | `crates/da-application/src/catalog_pdf_locator.rs` |
| 9 | `catalog_reconcile.rs` | (catalog plane) | кандидат | static-only (code read) | `crates/da-application/src/catalog_reconcile.rs` |
| 10 | `cluster.rs` (`ClusterUseCase`) | (queries plane) | кандидат | static-only (code read) | `crates/da-application/src/cluster.rs` |
| 11 | `cluster_stage.rs` | (pipeline stage) | кандидат | static-only (code read) | `crates/da-application/src/cluster_stage.rs` |
| 12 | `corpus_assurance.rs` | (corpus plane) | кандидат | static-only (code read) | `crates/da-application/src/corpus_assurance.rs` |
| 13 | `corpus_checkpoint.rs` | (corpus plane) | кандидат | static-only (code read) | `crates/da-application/src/corpus_checkpoint.rs` |
| 14 | `critic_repair_stage.rs` | source critic-repair | кандидат | static-only (code read) | `crates/da-application/src/critic_repair_stage.rs` |
| 15 | `cross_layer_signals.rs` | (signals plane) | кандидат | static-only (code read) | `crates/da-application/src/cross_layer_signals.rs` |
| 16 | `context_conservation.rs` | (conservation plane) | кандидат | static-only (code read) | `crates/da-application/src/context_conservation.rs` |
| 17 | `decision_recorder.rs` | (recording plane) | кандидат | static-only (code read) | `crates/da-application/src/decision_recorder.rs` |
| 18 | `derived_evidence_maturity.rs` | (maturity plane) | кандидат | static-only (code read) | `crates/da-application/src/derived_evidence_maturity.rs` |
| 19 | `derived_prompt_contract.rs` | (prompt contract plane) | кандидат | static-only (code read) | `crates/da-application/src/derived_prompt_contract.rs` |
| 20 | `derived_transition_evaluator.rs` | (transition plane) | кандидат | static-only (code read) | `crates/da-application/src/derived_transition_evaluator.rs` |
| 21 | `document_context_stage.rs` | (pipeline stage) | кандидат | static-only (code read) | `crates/da-application/src/document_context_stage.rs` |
| 22 | `embedding_write.rs` | (ingest stage) | кандидат | static-only (code read) | `crates/da-application/src/embedding_write.rs` |
| 23 | `enrich.rs` (`EnrichUseCase`) | enrich, batch-enrich | кандидат | static-only (code read) | `crates/da-application/src/enrich.rs` |
| 24 | `enriched_layer_stage.rs` | source enrich-layer | кандидат | static-only (code read) | `crates/da-application/src/enriched_layer_stage.rs` |
| 25 | `entity_resolution.rs` | (resolution plane) | кандидат | static-only (code read) | `crates/da-application/src/entity_resolution.rs` |
| 26 | `equation_extraction_stage.rs` | (load journey) | кандидат | static-only (code read) | `crates/da-application/src/equation_extraction_stage.rs`, `crates/da-cli/src/load_journey.rs` (line 40) |
| 27 | `equation_materialize.rs` | (materialization) | кандидат | static-only (code read) | `crates/da-application/src/equation_materialize.rs` |
| 28 | `extraction.rs` (`ExtractionUseCase`) | extract | кандидат | static-only (code read) | `crates/da-application/src/extraction.rs` |
| 29 | `extraction_materialization_stage.rs` | source build-extracted | кандидат | static-only (code read) | `crates/da-application/src/extraction_materialization_stage.rs` |
| 30 | `extractive_summary.rs` | summarize-revisions, source build-summary | кандидат | static-only (code read) | `crates/da-application/src/extractive_summary.rs` |
| 31 | `figure_analysis.rs` | (figure plane) | кандидат | static-only (code read) | `crates/da-application/src/figure_analysis.rs` |
| 32 | `figure_analysis_stage.rs` | (figure plane) | кандидат | static-only (code read) | `crates/da-application/src/figure_analysis_stage.rs` |
| 33 | `figure_candidate_assembly.rs` | (figure plane) | кандидат | static-only (code read) | `crates/da-application/src/figure_candidate_assembly.rs` |
| 34 | `figure_classify.rs` (`FigureClassifyUseCase`) | (figure plane) | кандидат | static-only (code read) | `crates/da-application/src/figure_classify.rs` |
| 35 | `figure_context.rs` | (figure plane) | кандидат | static-only (code read) | `crates/da-application/src/figure_context.rs` |
| 36 | `figure_context_stage.rs` | (figure plane) | кандидат | static-only (code read) | `crates/da-application/src/figure_context_stage.rs` |
| 37 | `figure_materialize.rs` | (materialization) | кандидат | static-only (code read) | `crates/da-application/src/figure_materialize.rs` |
| 38 | `figure_schema.rs` (`FigureSchemaService`) | (figure plane) | кандидат | static-only (code read) | `crates/da-application/src/figure_schema.rs` |
| 39 | `figures_stage.rs` | source build-figures | кандидат | static-only (code read) | `crates/da-application/src/figures_stage.rs` |
| 40 | `fsm_policy_consult.rs` | source sona-policy | кандидат | static-only (code read) | `crates/da-application/src/fsm_policy_consult.rs` |
| 41 | `fsm_policy_fallback.rs` | (policy fallback) | кандидат | static-only (code read) | `crates/da-application/src/fsm_policy_fallback.rs`, `crates/da-cli/src/parse_profile.rs` (line 94) |
| 42 | `graph_finalize.rs` (`GraphFinalizeUseCase`) | (finalize plane) | кандидат | static-only (code read) | `crates/da-application/src/graph_finalize.rs` |
| 43 | `graph_inventory.rs` | (inventory plane) | кандидат | static-only (code read) | `crates/da-application/src/graph_inventory.rs` |
| 44 | `healing.rs` (`GraphHealingUseCase`) | heal | кандидат | static-only (code read) | `crates/da-application/src/healing.rs` |
| 45 | `html_parsed_stage.rs` | (pipeline stage) | кандидат | static-only (code read) | `crates/da-application/src/html_parsed_stage.rs` |
| 46 | `identity_compatibility.rs` | (identity plane) | кандидат | static-only (code read) | `crates/da-application/src/identity_compatibility.rs` |
| 47 | `ingest.rs` (`IngestUseCase`) | ingest, batch-ingest | кандидат | static-only (code read) | `crates/da-application/src/ingest.rs` |
| 48 | `influence_flower.rs` | (analytics plane) | кандидат | static-only (code read) | `crates/da-application/src/influence_flower.rs` |
| 49 | `materialization_indexes.rs` | (materialization) | кандидат | static-only (code read) | `crates/da-application/src/materialization_indexes.rs` |
| 50 | `navigation_stage.rs` | source build-navigation | кандидат | static-only (code read) | `crates/da-application/src/navigation_stage.rs` |
| 51 | `native_family_decision_packages.rs` | (native family plane) | кандидат | static-only (code read) | `crates/da-application/src/native_family_decision_packages.rs` |
| 52 | `native_family_promotion_gate.rs` | (native family plane) | кандидат | static-only (code read; promotion gate — N2: no promotion receipt exists, gate never flipped) | `crates/da-application/src/native_family_promotion_gate.rs` |
| 53 | `native_family_same_n_adapter.rs` | (native family plane) | кандидат | static-only (code read) | `crates/da-application/src/native_family_same_n_adapter.rs` |
| 54 | `ontology_contract_audit.rs` | (audit plane) | кандидат | static-only (code read) | `crates/da-application/src/ontology_contract_audit.rs` |
| 55 | `ontology_endpoint_evidence.rs` | (audit plane) | кандидат | static-only (code read) | `crates/da-application/src/ontology_endpoint_evidence.rs` |
| 56 | `ontology_v2_convergence.rs` | (ontology plane) | кандидат | static-only (code read) | `crates/da-application/src/ontology_v2_convergence.rs` |
| 57 | `ordered_mutation.rs` | (mutation plane) | кандидат | static-only (code read) | `crates/da-application/src/ordered_mutation.rs` |
| 58 | `parse_grammar.rs` | source parse-*, parse-batch | кандидат | static-only (code read) | `crates/da-application/src/parse_grammar.rs` |
| 59 | `parse_job_runner.rs` (`ParseJobRunner`) | parse-job | кандидат | static-only (crate suites ran in gate/CI; no CLI-level recorded run) | `crates/da-application/src/parse_job_runner.rs` |
| 60 | `parse_learning_cycle.rs` | (learning plane) | кандидат | static-only (code read) | `crates/da-application/src/parse_learning_cycle.rs` |
| 61 | `parse_learning_evaluation.rs` | (learning plane) | кандидат | static-only (code read) | `crates/da-application/src/parse_learning_evaluation.rs` |
| 62 | `parse_learning_split.rs` | (learning plane) | кандидат | static-only (code read) | `crates/da-application/src/parse_learning_split.rs` |
| 63 | `parse_policy_ablation.rs` | (learning plane) | кандидат | static-only (code read) | `crates/da-application/src/parse_policy_ablation.rs` |
| 64 | `parse_policy_artifact.rs` | policy | кандидат | static-only (code read) | `crates/da-application/src/parse_policy_artifact.rs` |
| 65 | `parse_policy_gold_f1.rs` | (learning plane) | кандидат | static-only (code read) | `crates/da-application/src/parse_policy_gold_f1.rs` |
| 66 | `parse_policy_health.rs` | policy health | кандидат | static-only (code read) | `crates/da-application/src/parse_policy_health.rs` |
| 67 | `parse_policy_observation.rs` | (policy plane) | кандидат | static-only (code read) | `crates/da-application/src/parse_policy_observation.rs` |
| 68 | `parse_policy_pair.rs` | (policy plane) | кандидат | static-only (code read) | `crates/da-application/src/parse_policy_pair.rs` |
| 69 | `parse_policy_table.rs` | policy learn | кандидат | static-only (ADR-078 candidate-only surface) | `crates/da-application/src/parse_policy_table.rs` |
| 70 | `parse_policy_training.rs` | (learning plane) | кандидат | static-only (code read) | `crates/da-application/src/parse_policy_training.rs` |
| 71 | `pdf_toc_stage.rs` | source parse-pdf-toc | кандидат | static-only (code read) | `crates/da-application/src/pdf_toc_stage.rs` |
| 72 | `pipeline.rs` | (pipeline orchestration) | кандидат | static-only (code read) | `crates/da-application/src/pipeline.rs` |
| 73 | `pipeline_cache_audit.rs` | (audit plane) | кандидат | static-only (code read) | `crates/da-application/src/pipeline_cache_audit.rs` |
| 74 | `pipeline_ledger_audit.rs` | (audit plane) | кандидат | static-only (code read) | `crates/da-application/src/pipeline_ledger_audit.rs` |
| 75 | `pipeline_metrics.rs` | (metrics plane) | кандидат | static-only (code read) | `crates/da-application/src/pipeline_metrics.rs` |
| 76 | `pipeline_stage_assurance.rs` | (assurance plane) | кандидат | static-only (code read) | `crates/da-application/src/pipeline_stage_assurance.rs` |
| 77 | `promotion.rs` (`PromotionService`) | (promotion plane) | кандидат | static-only (code read; N2 — no promotion receipt in recorded evidence) | `crates/da-application/src/promotion.rs` |
| 78 | `prompt_evaluation.rs` | (prompt plane) | кандидат | static-only (code read) | `crates/da-application/src/prompt_evaluation.rs` |
| 79 | `queries.rs` | query family | кандидат | static-only (code read) | `crates/da-application/src/queries.rs` |
| 80 | `revision_classification.rs` | (revision plane) | кандидат | static-only (code read) | `crates/da-application/src/revision_classification.rs` |
| 81 | `revision_materialization.rs` | (revision plane) | кандидат | static-only (code read) | `crates/da-application/src/revision_materialization.rs` |
| 82 | `revision_materialization_stage.rs` | (revision plane) | кандидат | static-only (code read) | `crates/da-application/src/revision_materialization_stage.rs` |
| 83 | `revision_query.rs` | revision-query | кандидат | static-only (code read) | `crates/da-application/src/revision_query.rs` |
| 84 | `revision_readiness.rs` | (revision plane) | кандидат | static-only (code read) | `crates/da-application/src/revision_readiness.rs` |
| 85 | `scheduler.rs` (`GraphScheduler`) | scheduler-run, load | кандидат | static-only (code read) | `crates/da-application/src/scheduler.rs` |
| 86 | `scheduler_executors.rs` | scheduler-run | кандидат | static-only (code read) | `crates/da-application/src/scheduler_executors.rs` |
| 87 | `scheduler_runner.rs` | scheduler-run | кандидат | static-only (code read) | `crates/da-application/src/scheduler_runner.rs` |
| 88 | `scholarly_evidence.rs` | (evidence plane) | кандидат | static-only (code read) | `crates/da-application/src/scholarly_evidence.rs` |
| 89 | `schema_init.rs` | schema-init | кандидат | static-only (code read) | `crates/da-application/src/schema_init.rs` |
| 90 | `section_navigation.rs` | (navigation plane) | кандидат | static-only (code read) | `crates/da-application/src/section_navigation.rs` |
| 91 | `sona_bridge.rs` | source sona-* | кандидат | static-only (code read) | `crates/da-application/src/sona_bridge.rs` |
| 92 | `structural_elements_stage.rs` | source build-structural | кандидат | static-only (code read) | `crates/da-application/src/structural_elements_stage.rs` |
| 93 | `summaries_agent_stage.rs` | (summary agent plane) | кандидат | static-only (code read) | `crates/da-application/src/summaries_agent_stage.rs` |
| 94 | `summary.rs` (`SummaryUseCase`) | summarize | кандидат | static-only (code read) | `crates/da-application/src/summary.rs` |
| 95 | `table_extraction_stage.rs` | (table plane) | кандидат | static-only (code read) | `crates/da-application/src/table_extraction_stage.rs` |
| 96 | `table_materialize.rs` | (materialization) | кандидат | static-only (code read) | `crates/da-application/src/table_materialize.rs` |
| 97 | `tei_figures_stage.rs` | source build-figures-tei | кандидат | static-only (code read) | `crates/da-application/src/tei_figures_stage.rs` |
| 98 | `temporal_stamp.rs` | (stamp plane) | кандидат | static-only (code read) | `crates/da-application/src/temporal_stamp.rs` |
| 99 | `tex_inspect.rs` (`TexInspectUseCase`) | (inspect plane) | кандидат | static-only (code read) | `crates/da-application/src/tex_inspect.rs` |
| 100 | `validate_schema_objects.rs` | validate-objects | кандидат | static-only (code read) | `crates/da-application/src/validate_schema_objects.rs` |
| 101 | `acap_contract_audit.rs` | (audit plane) | unverified | unverified (module not opened in this pass; present and gate-compiled) | `crates/da-application/src/acap_contract_audit.rs` |
| 102 | `acquisition_batch.rs` (`BatchAcquisitionUseCase`) | acquire-arxiv | кандидат | static-only (code read) | `crates/da-application/src/acquisition_batch.rs` |
| 103 | `batch_ingest.rs` | batch-ingest | кандидат | static-only (code read) | `crates/da-application/src/batch_ingest.rs` |

Closed-set statement: the 103 rows above cover every `pub mod` declaration in
`crates/da-application/src/lib.rs` at HEAD; no module is silently skipped (E4/E6 analog for the
use-case side).

---

## 3. Beyond-dispatch da-cli surface

`crates/da-cli/src` also carries lib-only modules (not reachable from the `main.rs` dispatch
match, exported via the `da_cli` lib target and exercised by examples/tests). These are outside
the E4 closure but recorded so the enumeration is honest:

| Group | Modules | Verdict | Proof weight | Evidence |
|---|---|---|---|---|
| Native/route example-backed flows | native acceptance, route e2e, content receipt, corpus-resource same-n, learning-evaluation, feature-gated example families | работает | ran-in-gate (governor gate log names the example runners: `da-cli native-acceptance example runner... OK`, `da-cli route-e2e example runner... OK`, `da-cli content-receipt example runner... OK`, `da-cli corpus-resource same-n example runner... OK`, `da-cli learning-evaluation example runners... OK`, `da-cli feature-gated example runners/contracts... OK`) | `/tmp/arch-20261123-pass2.log`, `crates/da-cli/examples/` (e.g. `native_route_e2e.rs`, `native_content_receipt.rs`, `native_corpus_resource_same_n.rs`), `crates/da-cli/src/native_acceptance.rs`, `crates/da-cli/src/native_route_receipt.rs` |
| Parse support (lib+bin) | `parse_config.rs`, `parse_profile.rs`, `production.rs`, `load_journey.rs` | кандидат | static-only (composition modules; gate `da-cli tests... OK` ran the lib surface, no CLI-level recorded run) | `crates/da-cli/src/parse_config.rs`, `crates/da-cli/src/parse_profile.rs`, `crates/da-cli/src/production.rs`, `crates/da-cli/src/load_journey.rs` |
| Gold/canary family | `gold_corpus.rs`, `gold_graph.rs`, `catalog_drift.rs`, `catalog_materialize.rs`, `catalog_regression.rs`, `catalog_sweep.rs`, `graph_tasks.rs`, `materialization_progress.rs`, `independent_preservation_candidate.rs`, `release_readiness_receipt.rs`, `policy_ablation_reference.rs` | кандидат | static-only (present; driven by `crates/da-cli/examples/` runners, not dispatch) | `crates/da-cli/src/gold_corpus.rs`, `crates/da-cli/src/gold_graph.rs`, `crates/da-cli/src/graph_tasks.rs` |
| Java measurement slices | `java_cpu_counter.rs`, `java_free_slice.rs`, `layout_runtime.rs` | unverified | unverified (module presence confirmed; internals not inspected in this pass) | `crates/da-cli/src/java_cpu_counter.rs`, `crates/da-cli/src/java_free_slice.rs`, `crates/da-cli/src/layout_runtime.rs` |
| Misc lib-only | `visual_adapter.rs`, `uat_verdict_lineage.rs`, `batch_ids.rs`, `logging.rs`, `audit_fields.rs` | unverified | unverified (presence confirmed; internals not inspected in this pass) | `crates/da-cli/src/visual_adapter.rs`, `crates/da-cli/src/uat_verdict_lineage.rs` |

Per the proof-weight note in §3 row 1: these example-runner executions are local-gate
executions; the da-parse integration plane is `--lib`-only in CI
(`.github/workflows/architecture-guardrail.yml`, line 101) and none of the named flows is
CI-protected (U1).

---

## 4. Count summary (rendered from code, not README)

- Top-level dispatch arms: 38 of 38 enumerated (`crates/da-cli/src/main.rs` match, lines 534–981).
- Nested leaves: source 24 + policy 2 + agent-run 2 + parse-job 4 = 32.
- da-application use-case modules: 103 of 103 enumerated (`crates/da-application/src/lib.rs`).
- Verdict distribution: работает — §1a parse-batch + §3 native/route example-backed flows;
  кандидат — all remaining dispatch leaves and use-cases; заглушка — none (§1c sweep);
  unverified — §2 row 101, §3 rows 4–5.
- Every «работает» cites the recorded governor gate log `/tmp/arch-20261123-pass2.log`
  (ran-in-gate) per N5; no verdict was upgraded to «работает» on static reading alone.
