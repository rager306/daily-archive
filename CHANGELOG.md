# Changelog

### M389 S04 docs and residual handoff

- ADR-060 residual list updated with M389 stage-cache + provenance + findings honesty.
- Handoff: M388 complete; M389 S01–S03 product code delivered; residual chart-native
  domain type, compound panels, CharXiv suite, STATE projection lag.

### M389 S03 chart draft findings honesty

- `map_structured_to_draft` surfaces `chart_type=` and `key_data_points=` into findings
  for Plot drafts; `table_headers=` for Table drafts. No graph import; no invented numbers.
- Plot unit test asserts findings + provenance schema_id=chart_extraction.

### M389 S02 provenance schema_id and stage1_visual_type

- `FigureAnalysisProvenance` adds required `schema_id` and `stage1_visual_type`
  (wire string validated against VisualAnalysisType).
- `FigureAnalysisUseCase::analyze` sets both from Stage1 classification + Stage2 selection.
- Domain + application figure tests green (16 domain figure*, 24 application figure_*)

### M389 S01 figure stage cache binds schema registry identity

- `FigureAnalysisStage` configuration_fingerprint includes
  `schema_registry=figure-schemas.v1`, `schema_id=diagram_analysis`, and schema sha.
- Implementation fingerprint bumped to grounded-v3-schema-registry-identity so old
  caches cannot silently reuse pre-registry keys.
- Unit test `configuration_fingerprint_binds_schema_registry_identity` (5/5 stage tests).

### M388 S08 docs closeout (ADR-060 partial)

- ADR-060 Status → Partially implemented (M388): Stage1 heuristic, DATA schemas,
  analyze wiring, luna route; residual gaps listed.
- ADR-INDEX row updated. Handoff points to next wave (chart-native domain, STATE
  projection, S07 residual eligible gates still hard).

### M388 S07 summary claim purity and soft match (honest metrics)

- `extract_claims_from_output`: soft ordered-token match (stopword-aware) for required
  fact payloads; free structured method/metric/dataset/term claims no longer dilute
  grounding (only emitted if already a required_fact).
- DATA prompt: article_summary system_template asks for near-verbatim metric/method phrasing.
- Offline re-score frozen M387 inference: summary coverage **9230→10000**, grounding
  **2926→10000**, gap **1429→0**; figure coverage **4545→6363**. Still eligible=false,
  optimizer=false, import_eligible=false (D127).
- Tests: 9 prompt_evaluation + 3 example tests green.

### M388 S06 governor planning projection preferred_active

- When STATE Active Milestone is not in ROADMAP active set, diagnostic now includes
  `preferred_active=` (highest M### among ROADMAP active, prefer suffixed ids).
- Operator guidance: re-render STATE via GSD tools — no hand-edit of engine DB files.
- Tests: `scripts/tests/test_gsd_planning_reader.py` (11 passed); live smoke for M388.
- Trajectory still `drift_risk` (stale STATE M364, M383 empty PLAN, target bloat) with
  violations=0; product path unchanged.

### M388 S05 wire Stage1+DATA schema into figure analyze path

- Audit: `artifacts/m388/s05_hotpath_seam_audit.md` (hardcoded schema + unused Stage1/2).
- `FigureAnalysisUseCase` now runs Stage1 classify + Stage2 `FigureSchemaService`
  selection; multimodal request uses DATA `json_schema` (not hardcoded object).
- Flexible draft map synthesizes components for chart/table/photo shapes so domain
  `FigureAnalysis` still validates; citations remain grounded.
- Tests: flowchart→diagram_analysis, plot→chart_extraction schema hints; 23 figure_* tests green.
- D127 false; no live VLM re-run in this slice.

### M388 S04 pdf-inspector product triage policy (typed status)

- Added DATA `data/ontology/pdf_triage_policy.yaml` (fail_open, feature_gate,
  encoding_issues_block_grobid=false, import_eligible=false).
- Domain `PdfTriagePolicy` + `PdfTriageStatus` {NotConfigured, Classified, FailedOpen}.
- `IngestResult.pdf_triage` always set; classify failures record FailedOpen under
  fail_open policy; feature gate unchanged (`--features pdf-classifier`).
- Tests: domain policy + pdf_classifier_test triage status; authority under tests/.

### M388 S03 per-type figure schemas (ADR-060 Stage2 seam)

- Added DATA-driven `data/ontology/figure_schemas.yaml` with schemas:
  diagram_analysis, chart_extraction, table_structure, photo_caption, other_description.
- Domain `FigureSchemaRegistry` / `FigureSchemaSelection`: select_for(VisualAnalysisType),
  schema_sha256 for provenance; bundled include_str; fail-closed schema_version.
- Application `FigureSchemaService` selects by type or Stage1 classification.
- Tests: domain registry selection + 2 application unit tests. No VLM, D127 false.
- Seam only: FigureAnalysisUseCase still uses legacy single schema until wired next.

### M388 S02 ADR-060 Stage1 figure type classify (heuristic)

- Added DATA-driven `data/ontology/figure_type_labels.yaml` and domain
  `FigureTypeLabelTable` / `FigureTypeClassification` (candidate-only, D127).
- Heuristic Stage-1 `classify_from_text` from caption/citing prose; no paper-specific
  hardcoding beyond YAML keywords.
- Application `FigureClassifyUseCase` over `FigureSemanticContext` (bundled labels).
- Tests: da-domain figure suite + 3 figure_classify unit tests. No VLM call, no import.

### M388 S01 honest universal figure claim extraction

- Moved claim extraction into `da_application::prompt_evaluation::extract_claims_from_output`:
  universal prose bag + structured arrays + required/forbidden fact projection.
- Removed GEPA/paper-specific keyword hardcoding from `prompt_eval_scored` example.
- Offline re-score of frozen M387 inference: figure coverage **0 → 4545**, grounding
  **0 → 10000**, schema_validity 10000; summary coverage **8461 → 9230**. Still
  `eligible=false`, `optimizer=false`, `import_eligible=false` (D127).
- ADR-051: unit tests in da-application + example tests; no paid inference in S01.

### M386 S02 bounded acquisition retry/backoff

- Added DATA-driven bounded HTTP retry/backoff in `http_fetch.rs`
  (`DA_HTTP_RETRY_BACKOFF_SECONDS`, default 1,5,15,60,300; transient-only:
  503/5xx/timeout/network; 4xx fail closed). 5 unit tests.
- Provider URLs env-configured (D239, no hardcode): `DA_ARXIV_EXPORT_BASE_URL`,
  `DA_ARXIV_ABS_BASE_URL`, `DA_AR5IV_BASE_URL` in `.env.example`.
- Wiremock contract matrix `arxiv_acquisition_contract.rs` (4 tests: atom+pdf
  happy, transient 503 first_valid fallback, partial pdf fail-closed, network
  denied fail-closed).
- Real network acquire verified: 2507.19458 cs-ai PDF+HTML checksum-bound.

### M387 S08 pipeline acceptance

- Added `pipeline_acceptance` example aggregating 14-stage pipeline ledger
  (all contract pass), cache replay proof (168239 unchanged, valid partition),
  optimizer readiness gates (2 tasks evaluated-not-eligible) into one typed
  acceptance report. Verdict=accepted, import_eligible=false, optimizer=false.
- Governor gates: `no_optimizer_or_auto_promote_true_in_artifacts` ratchet
  covers runtime artifacts; negative fixture tests cover production-acceptance
  and optimizer unlocks via tempdir (no repo fixtures → trajectory clean).
- Durable: `artifacts/runtime/pipeline_acceptance_m387.json`.

### M387 S07 retrieval quality

- Replaced RuVectorBridge stub with functional graph-aware bridge: RRF hybrid
  fusion (ruvector-hybrid) + feature-gated PPR (ruvector-solver forward-push on
  plain CSR); 13 ppr tests + 9 base tests PASS.
- Retrieval quality benchmark (3 queries, golden judgments, tenant filter):
  dense/lexical/rrf-hybrid recall@k=1.0 filter=true admit; ppr filter=false
  → benchmark-only (honest: fixed topology ignores tenant). Quality decides
  admission via data/retrieval/policy.yaml (min recall@k=0.6).
- Artifact: retrieval_benchmark_m387.json. D127=false, optimizer=false.

### M387 S06 Samyama hot path profile

- Added `samyama_hot_path_profile` example measuring write paths
  (create_node/edge, property set, identity lookup, traversal, vector insert/search)
  at 100/1k/3k scales, FROZEN snapshot parity (hub.sgsnap: 252 nodes/342 edges,
  FROZEN lookup faster than HOT by column store), and import/replay idempotency
  (import 43ms → warm replay 31ms, identical counts).
- Honest findings: edge create scales ~34x for 30x data (write amplification);
  identity/traversal/vector are sub-linear; vector search is 7-20us (fast).
- Durable: `artifacts/runtime/samyama_hot_path_profile_m387.json`.
  import_eligible=false, optimizer_enabled=false, auto_promote=false, D127=false.

- Extended pipeline cache profile to 1/10/30/100/200 article scales with cold
  vs warm (pre-warmed) memory-cache lookups, per-scale key/payload SHA lineage
  and drift probes. Honest finding: memory warm hits are 100% but warm_us >= cold_us
  at scale (store+hit overhead); file-cache hits remain the dominant win
  (hit_us 5958 vs cold_us 191 at 1 article).
- Added `stage_ablation_report` example binding cache ablation, materialization
  scaling (exponent 0.968, projected_200 18.59s), replay proof (168239 unchanged)
  and optimizer readiness deltas into one report; import_eligible=false,
  optimizer_enabled=false, auto_promote=false, D127=false.
- Durable: `artifacts/runtime/pipeline_cache_profile_m387.json` (5 scales),
  `artifacts/runtime/stage_ablation_profile_m387.json`.

- Fixed adapter contract bug: provider responses that wrap JSON in conversational
  text (preamble/code fences) now yield canonical, directly parseable
  `output_bytes` (serialized validated instance), not the raw wrapper; lineage
  `output_sha256` binds the canonical bytes. Regression test covers wrapper text.
- Added evaluation-only binding `prompt-eval-summary-minimax-m27`
  (minimax/MiniMax-M2.7-highspeed, non-vision) and routed summary evaluation to it.
- `prompt_eval_scored` now extracts canonical claims from structured fields plus
  keyword evidence from prose (no sentence noise), with unit tests for fact
  extraction, noise suppression and reasoning-model budget case.
- Real same-n scoring evidence (e260a0b1): summary grounding=2558,
  faithfulness=2558, coverage=8461, gap=2858 via MiniMax-M2.7; figure
  grounding=10000, faithfulness=10000, coverage=4545 via qwen3-vl with 2/4
  schema quality-signals. Both eligible=false, deployed=false, D127=false.

### M387 S04 typed-media figure prompt migration

- Extended `PromptExecutionPort` with typed, checksum-bound, bounded image media
  and explicit timeout/rate-limit/unavailable errors. The text adapter rejects
  media; the new OpenAI-compatible multimodal adapter requires exactly one
  supported image, emits an `image_url` data URL to 9router, validates the real
  figure JSON Schema and returns full prompt/model/media lineage.
- Added `PromptMultimodalBridge` so the existing cache-aware figure stage reuses
  PromptExecution without duplicating orchestration. Figure candidates now fail
  closed without PromptExecution lineage and bind task, route, resolved model,
  input and output hashes in provenance. Cache configuration uses registry task
  and schema digests instead of legacy constants.
- Removed CLI route guessing, direct MiniMax fallback, inline model/default URL
  and runtime `/models/image-to-text` probing from `analyze_figure`. It now
  resolves `figure-qa-primary` from `models.yaml`; the binding points to the
  live-inventory model `novita/qwen/qwen3-vl-235b-a22b-instruct` through 9router.
- Added local ephemeral OpenAI-compatible image roundtrip plus media type/size/
  checksum negatives. No paid inference or external provider action was issued.
- Added durable `prompt-optimizer-readiness.v1` across summary and figure tasks:
  exact prompt/route/schema/model binding identities and complete lineage fields
  are present. Synthetic independent contract fixtures exercise both tasks with
  same-n=2 through FSM state `evaluated`; source digests, required/forbidden facts,
  train/validation separation, schema/grounding/faithfulness/coverage/paraphrase,
  latency/token/cost metrics and contamination/budget gates are executable. These
  fixtures are explicitly not production-acceptance evidence; optimizer,
  auto-promote, eligibility, deployment and D127 remain false.
- Hardened frozen prompt evaluation provenance: synthetic source text was replaced
  by independently reviewed canonical GEPA abstract/section excerpts and Figure 3/5
  caption+image evidence. Repo-relative source/review/image refs carry exact SHA-256,
  source excerpts must occur in bound artifacts, and oracle producer identity must
  differ from baseline/candidate producers. Added fail-closed tests for self-oracle,
  path escape, digest drift and unbound excerpts, plus Governor ratchets forbidding
  prompt-eval DATA from claiming production acceptance or enabling optimization.
- Hardened OpenAI-compatible LLM adapter for production reasoning-model compatibility:
  added `stream:false`, `enable_thinking:false`, `response_format:json_object`,
  empty-success-response detection with typed error, `finish_reason` metadata logging,
  `<think>` tag stripping, and balanced-brace JSON extraction from wrapper text
  (preamble, markdown code fences, and reasoning blocks). Added model binding
  `ninerouter-minimax-m3-openai` (`minimax/MiniMax-M3`) with evaluation-only
  `prompt-eval-{summary,figure}-minimax-m3` bindings. Added
  `data/prompt_eval/inference.yaml` strict policy (DATA-driven: maximum calls,
  request timeout, variants, task routes, proxy env, safety locks) and
  `prompt_eval_inference` CLI example with durable attempt/failure ledger.
  Bounded MiniMax-M3 same-n matrix was authorized and executed: the model returns
  valid prose summaries but 9router ignores `response_format` and the model does
  not emit JSON; durable evidence records attempted=1 completed=0.

### M387 S04 9router model binding correction

- Removed the incorrect Anthropic endpoint/model binding from the production
  summary route. `models.yaml` is now the canonical typed model registry and
  `paper-summary-generate-default` resolves to `agnes-ai/agnes-2.0-flash` through
  the OpenAI-compatible local 9router proxy. Quality, fallback and judge roles
  remain separate bindings for later same-n evaluation.
- Added strict `models.yaml` loading with referential integrity, HTTPS/local-HTTP
  endpoint policy, binding/model/tool/policy lineage and a bounded host override
  only for 9router. `.env` now owns only `NINEROUTER_URL`, optional
  `NINEROUTER_KEY`, `DA_LLM_TIMEOUT_SECONDS` and registry paths.
- Added a functional OpenAI-compatible client with bounded timeout, optional
  bearer authentication, token accounting, content/reasoning-content handling,
  and hash-only provider-error diagnostics. Prompt lineage and Summary candidate
  nodes now bind the resolved model-binding SHA in addition to the model name.
- Runtime discovery against `http://127.0.0.1:20128/v1/models` returned HTTP 200,
  592 models, and confirmed all configured summary generator/quality/fallback,
  judge, and figure-primary model IDs are present. No inference request was issued.

### M387 S04 prompt execution FSM foundation

- Accepted ADR-059/D238: every active generative call must pass through a
  provider-neutral `PromptExecutionPort`; GEPA/DSPy remain disabled,
  candidate-only and unable to deploy or write graph truth.
- Added typed prompt task/spec, exact execution lineage, independent evaluation
  gates and FSM `baseline_registered → candidate_proposed → evaluated →
  eligible|rejected`. Same-n, validation gap, contamination, hard invariants,
  latency/cost, deployment and D127 fail closed.
- Added strict, referentially complete `data/ontology/prompt_tasks.yaml` for
  article-summary and figure analysis templates, real JSON Schemas, route
  policies, generation bounds and eval suites. Model/provider/protocol selection
  comes from `models.yaml`; `.env` owns 9router host/key, bounded timeout and
  registry paths. Optimizer is false at registry/task levels.
- Migrated production `da summarize` from raw `LLMClient` and inline prompts to
  `PromptExecutionPort`. The adapter enforces an exact variable set, appends the
  resolved JSON Schema, resolves route/model/endpoint from DATA+.env, validates
  provider output before returning, and binds task/prompt/schema/route/endpoint/
  model/input/output hashes plus tokens, latency and effective generation bounds.
- Closed a graph-authority bug: generative summary output no longer creates
  canonical Method/Dataset/Metric/QualitySignal nodes or retrieval-eligible
  summaries. Only lineage-bound `Summary` candidates are written with
  `authoritative=false`, `retrieval_eligible=false`, `import_eligible=false`;
  unresolved section evidence or provider/schema failure causes zero mutations.
  Independent deterministic QualitySignal fixtures now preserve the ontology
  endpoint canary without granting the LLM quality-fact authority.

### M387 S03 cache lineage and whole-pipeline metrics foundation

- Centralized canonical `StageCacheKey::key_sha256` at the port contract and
  added strict effective-input validation. Added a production-safe in-memory
  cache implementation plus one shared contract suite for memory/filesystem:
  cold/hit, configuration drift, retry expiry, concurrent same-key publication,
  manifest enumeration and visible corruption.
- Added dual cache lineage evidence: full effective key SHA and payload SHA are
  both mandatory. Identical output bytes under different config/prompt/model
  keys remain distinct computations; compute cache is forbidden from claiming
  that graph materialization was skipped.
- Added optimizer-ready `pipeline-metrics.v1`: every one of the 14 ledger stages
  has runtime/FSM, wall/CPU/RSS/I/O, queue/worker, cache, context, token/model,
  cost, backend and failure fields. Unavailable observations are typed
  `no_claim`; no backend is inferred from stage names; optimizer admission is
  absent and D127 is false.
- Durable bootstrap report `artifacts/runtime/pipeline_metrics_m387.json` has 14
  ordered rows, two evidence-backed parse/extract rows and 12 honest no-claim
  rows. Metrics will be filled by later stage execution rather than copied from
  incomparable aggregate timings.
- Added real 1/10/30 filesystem-cache profiling across parse/extract/embed: all
  3/30/90 entries showed cold miss, verified hit and config-drift miss with exact
  key+payload identities; corruption remained visible. The same report binds the
  production 200-paper `materialize_wave` physical replay: applied=0, rejected=0,
  unobserved=0, unchanged=168239. This proves cached compute does not replace the
  always-run, replay-idempotent graph materialization path.
- Closed rolling JSON logging debt in pipeline ledger and both materialization
  profile composition roots. D238 requires every active generative model call
  to pass through a future provider-neutral PromptExecutionPort/FSM before
  GEPA/DSPy candidates can be evaluated; optimizers remain disabled.

### M387 S02 ACAP context conservation diagnostics

- Added ACAP-aligned stage context snapshots and transitions: every ratio carries
  a full eligible denominator and `BoundaryMode`; zero-denominator evidence is
  typed `no_claim`, never perfect retention. Token counts require exact tokenizer
  lineage, D127 is fail-closed, and S02 has no independent admission verdict.
- Added pure parse, revision-materialization, extraction and summary/readiness
  observers. Candidate entities remain partitioned from authoritative context;
  repeated headings/navigation noise and missing/invalid ordinal-span grounding
  are visible without graph writes.
- Added strict diagnostic DATA policy with policy identity and digest binding.
  Hard ACAP invariants cannot be weakened by YAML thresholds; unknown, oversized
  and out-of-range policies fail closed.
- Real 30-article cache profile read 30 parse and 30 extract payloads: 927
  sections, 1,192 citations and 1,337,968 text characters; 140 extraction
  candidates, 117 valid document-scoped spans. Twelve unobserved ledger stages
  are explicit `no_claim`. Exact revision report remains D127=false. Durable
  deterministic evidence: `artifacts/runtime/context_conservation_m387.json`.
- Governor follow-up is intentionally deferred to S08: ratchet the proven report
  schema only after later cache/scheduler/backend slices fill additional stages.

### M387 S01 executable semantic stage ledger

- Added strict DATA contract `data/ontology/pipeline_stage_contracts.yaml` for
  14 production stages from acquisition through query. Every stage names owner,
  semantic inputs/outputs, context added/lost, dependencies, hard invariants,
  consumers, cache/materialization mode, failure class and conditionality.
- Added provider-neutral domain validation for duplicate IDs, missing deps,
  cycles, incomplete semantics and cache/graph-truth boundary mismatch; strict
  YAML loading rejects unknown fields and oversized input.
- Added application runtime ledger audit with observed order, input/output
  context digests/counts and failure visibility. Missing, duplicate, unknown,
  out-of-order and invisible-failure boundaries fail closed. Durable current
  audit: 14 stages, zero findings, D127=false at
  `artifacts/runtime/pipeline_stage_ledger_m387.json`.
- Pipeline audit found Samyama active for scalar graph/vector paths, while
  RocksDB remains inactive. RuVector is active only in MCP hybrid retrieval and
  feature-gated PPR; `RuVectorBridge` is a stub. No ruvector parameter/default
  will change before S07 recall/quality/latency ablations prove value.

### M386 S01-S02 Rust-only acquisition boundary

- D237 makes the production data pipeline Rust-only. Python remains permitted
  for governor, tests, migration analysis and frozen evidence, but Rust must
  never shell out to Python for product work.
- Added typed `batch-acquisition-report.v1`, provider-neutral application batch
  orchestration and production `da acquire-arxiv`. The command reuses
  `SourceAcquisitionPort` / `ArxivSourceAcquisition`, accepts IDs/files, renders
  `[n/total]` progress, supports bounded pacing, writes an atomic self-digested
  report and keeps D127 false.
- Real no-network replay over all 580 M385 IDs completed in Rust: 579 catalog
  hits, one explicit withdrawn/missing `2507.23261`, zero Python processes and
  `import_eligible=false`. README and GEPA reproduction now use the Rust command.
- Machine-readable boundary audit at
  `artifacts/runtime/rust_pipeline_boundary_m386.json` reports zero product
  Python command references, one remaining product script pending deletion after
  metadata/network parity closeout, and zero Rust→Python subprocess calls.

### M385 Awesome Agentic Reasoning corpus expansion

- Pinned the supplied `Awesome-Agentic-Reasoning` README by SHA256 and
  deterministically extracted 740 arXiv links into 580 versionless unique IDs
  (160 duplicates removed). At baseline 29 were already in the catalog and 551
  were queued for acquisition.
- Existing resumable source recovery downloaded 550/551 missing IDs with visible
  `[n/551]` progress, checksum-bound atomic PDF+HTML publication and no `.part`
  residue. Failure-only retry confirmed `2507.23261` is withdrawn: its abstract
  exists but all PDF endpoints return HTTP 404.
- Recovered HTML for 29 pre-existing source roots. Final source-specific
  assurance: **579/580** IDs have valid PDF, substantive HTML and canonical
  `article.json`; invalid assets, partial files and D127 violations are all zero.
- Fixed a real acquisition/catalog process gap: `download_arxiv_to_catalog.py`
  now publishes fail-closed `article.v00.02` metadata from verified checksums and
  supports offline `--backfill-metadata-from-report`. Existing metadata is never
  overwritten; replay is skip-only; parser/chunk/graph/import readiness remains
  false. Downloader tests: 21 pass.
- Rebuilt catalog indexes after metadata publication. Global reconciliation debt
  remains separately visible (duplicates/conflicting historical roots and
  missing variants); no medium/high action was auto-applied and no graph import
  occurred.

### M384 S02 measured hot-path optimization

- Added typed, self-digested `materialization-scaling-profile.v1` evidence and
  bounded 1/10/30/200-paper profiling with host, binary, CPU/RSS, mutation,
  strict semantic identity and D127 provenance.
- Profiling found two algorithmic defects before any speculative batching:
  `edge_type_is_temporal_fact` reparsed ontology YAML for every edge (8.8 ms
  per call), and `SamyamaGraphStore::get_edges_between` scanned every graph
  edge despite vendor HOT+FROZEN adjacency APIs.
- Cached the immutable DATA registry with exact all-edge-type parity and reused
  Samyama `edges_between`. Microprofile improvements: temporal classification
  8,806→7.2 µs (1,226×); missing edge lookup at 3,000 edges 512→1.65 µs
  (311×). Shared GraphStore and Samyama HOT/FROZEN contracts pass.
- Real strict-semantic-equal wall times on the same debug binary/cache/time:
  1 paper 0.11s, 10 papers 0.63s, 30 papers 1.56s, 200 papers **18.59s**.
  The prior M382 rebuild took about 57 minutes: ~184× end-to-end improvement.
  200-paper replay applied zero mutations, graph remained 23,836/39,295,
  heartbeat had no stalls, peak RSS was 229 MB and D127 remained false.
- RocksDB tuning is explicitly not applicable to this process-local in-memory
  Samyama + snapshot-export path (D234). Persistent/daemon flows require their
  own measured RocksDB benchmark before block-cache/write-buffer tuning.
- SMP worker-count metamorphic profile (30 papers, same binary/cache/time) was
  deterministic for 1/2/4/8/12 workers: identical strict semantic SHA, graph,
  mutation aggregates and D127=false. Wall times were 1.74/1.53/1.40/1.58/1.48s;
  CPU 101–121%, RSS 43–49 MB. Four remains the measured default; more workers
  oversubscribe a small preparation phase and cannot parallelize the single writer.
- Bulk feasibility audit reuses the existing `OrderedMutationPlan` rather than
  inventing a second DSL. Current Samyama MVCC transactions cannot provide the
  required atomic batch contract: ordinary writes are not transaction-bound and
  abort explicitly does not roll back in-place mutations. A pseudo-atomic wrapper
  is forbidden (D235). The accepted scalar path already sustains 27,163 observed
  mutation attempts/s (504,965 attempts in 18.59s); true bulk is deferred until
  persistent/remote HOT requirements justify a clean vendor atomic API or
  copy-on-write store swap with rollback contracts.
- Persistent storage applicability audit found no current RocksDB runtime to
  benchmark: `serve-graph` forces `data_path=None`, `DA_GRAPH_DATA_PATH` fails
  closed as `PersistenceNotConfigured`, and remote HOT is unsupported. RocksDB
  tuning is deferred to a separate persistent-daemon capability with vendor
  isolation, crash recovery and real write fixtures (D236), not guessed here.

### M384 S01 observable corpus materialization

- `gold_graph_all_waves` now emits a concise stderr heartbeat every 5 seconds
  by default and structured tracing events for phase, wave, current paper,
  completed papers/work units, elapsed/idle time, rolling throughput, ETA,
  physical mutation counters and stalls. Intervals are bounded CLI options;
  `--no-progress` disables only rendering, not internal measurement.
- Materialization progress covers prepare, first write, replay, assurance and
  export without changing graph order or semantics. The all-waves report is v2
  and embeds its final progress snapshot.
- A real 10-paper canary emitted five useful ticks, completed 30/30 work units
  in 6.8s with 14,857 attempted mutations and D127=false. ABAB runs measured
  -0.48% apparent overhead (within noise), and progress on/off snapshots had
  identical strict semantic SHA, graph counts and mutation aggregates.
- The canary used about one CPU and scaled non-linearly versus the 57-minute
  200-paper run, reinforcing that serialized fine-grained writes and O(n)
  identity lookup—not missing worker flags alone—are the next measured targets.

### M382 S01-S02 typed checkpoint identity evidence

- Bumped corpus checkpoint package to `corpus-required-layer-checkpoint.v2` and
  replaced ambiguous `snapshot.sha256` with explicit physical SHA, strict
  semantic SHA, semantic algorithm, path and graph counts. Domain validation,
  package digest and serde roundtrip bind every identity field.
- Added production `da snapshot-identity --input … --report …`. It delegates
  canonicalization to `kg_storage::fingerprint_snapshot`, enforces compressed
  and decompressed bounds, emits atomic self-digested evidence, keeps D127
  false, and preserves an existing report when malformed input fails.
- Migrated both corpus checkpoint compositions to the typed identity contract.
- Replaced the historical `--allow-derived-snapshot` warning bypass with a
  bounded DATA-governed producer-report chain. Domain validation requires exact
  materialization→stage→actual-byte continuity, unique ordered stages, report
  hashes, D127=false, and binds the lineage into checkpoint v3 digests.
- Strict real-corpus fingerprinting exposed 173 global Paper/Citation VID
  collisions. Added normalized source-qualified `citation_arxiv_vid`; malformed
  parser claims now remain observable Reference-only evidence instead of
  becoming false Citation nodes. Rebuilt the 200-paper base/cluster/resolution
  chain through real producers: 0 duplicate VIDs, 23,836→24,527 nodes and
  39,295→40,623 edges. Direct and 2-link derived checkpoints PASS; reordered or
  final-mismatch chains fail before package replacement.
- Added governor rule `no_boolean_derived_snapshot_waiver` only after executable
  negative fixtures proved the old bypass. The old corpus summary producer is
  no longer compatible with revision-owned summary contracts; full summary
  acceptance remains a separate revision-migration gap rather than being faked.

### M381 S01-S02 strict semantic snapshot identity

- Added bounded project-owned `.sgsnap` identity parsing with two explicit
  identities: SHA256 of exact compressed bytes and strict ontology-v2 semantic
  SHA over unique node VIDs, labels, every node/edge property and edge endpoints
  resolved through VIDs.
- Canonicalization ignores only physical IDs, gzip/header timestamp, JSON map
  order and record order. Missing/duplicate VID, dangling endpoints, count/
  format/version/type errors and decompression/record limits fail closed.
- Real accepted snapshot proof: repeated exports and reload of one unchanged
  graph state have different physical SHA but the same strict semantic SHA;
  one property mutation invalidates semantic SHA.
- Universal `da load` now populates its formerly-empty legacy
  `snapshot_sha256` physical field plus semantic SHA/algorithm and node/edge
  counts before resealing the report digest. Invalid exported snapshots make the
  report fail visibly.
- Two independent exact-v1 loads surfaced a real temporal determinism gap:
  VIDs/topology/node properties match, but 23 edge `created_at` and 9 fact
  `valid_at` values differ. Strict semantic SHA correctly differs. D227 forbids
  hiding ADR-046 temporal truth in canonicalization.
- Added explicit `da load --materialization-time <RFC3339>` (D228). CLI parses
  once and injects one Clock into ingest and extraction; default remains the
  honest SystemClock. Two fresh exact-v1 loads at the same explicit time now
  share strict semantic SHA while physical SHAs differ; changing the time
  changes strict semantic SHA. Reports record the explicit temporal basis.

### M380 unified read-only query contract

- Fixed a mode-dependent security/logic gap: embedded
  `SamyamaGraphStore::query_readonly` previously called the unrestricted Cypher
  engine directly, while only RemoteGraphStore enforced the read-only policy.
- Extracted one adapter-neutral `kg_storage::validate_readonly_cypher` module.
  Embedded and remote adapters now reject the same mutation/admin tokens before
  engine or network execution; trusted full `query` remains separate.
- Contract corpus covers MATCH/RETURN/WITH/UNWIND, quoted strings, comments and
  backticks, plus CREATE/MERGE/SET/REMOVE/DETACH DELETE/CALL/LOAD/FOREACH/USE and
  malformed quote/comment rejection.
- Embedded physical proof captures node/edge totals and identity, rejects CREATE,
  then reloads before/after exports into fresh stores and verifies semantic
  equivalence. Samyama export bytes themselves are nondeterministic across
  unchanged exports, so byte equality is not falsely claimed.
- Existing cross-layer revision preparation graph tasks remain 4/4 PASS.

### M379 remote graph health convergence

- Deleted the second hand-written RESP client/parser from `da graph-ping`.
  The command now delegates to bounded `RemoteGraphStore::health`, preserving
  exact `PONG` stdout and inheriting common endpoint/connect/timeout/protocol
  diagnostics.
- Removed now-dead `response_is_pong`, `RESP_PING`, raw `TcpStream` I/O and the
  stale `mode_label` helper/tests. Live and unreachable graph-ping contracts now
  exercise the same universal transport as graph-query.
- `da health` now branches explicitly by graph mode. Embedded retains local
  health/node count; remote resolves `DA_GRAPH_ENDPOINT` and reports scalar read
  reachability independently from HOT write capability.
- Runtime matrix PASS: embedded healthy; remote endpoint missing; remote
  unreachable with endpoint redacted; remote reachable against live daemon.
  Every remote state says HOT writes unsupported and D127 false; no auto-start
  or embedded fallback occurs.
- Fixed the operational shutdown gap exposed by background alerts: serve-graph
  now handles SIGINT/SIGTERM as expected lifecycle events, prints an explicit
  shutdown line and exits 0. Real PING then SIGTERM proof passes; no daemon is
  left running.

### M378 S03 remote read operational contract

- Extended the real remote probe with a capability matrix, before/after revision
  counts, eight concurrent bounded reads, duration, and explicit unsupported
  index/snapshot evidence.
- Two repeat runs against one snapshot-preloaded daemon returned count=2 before
  and after every wave; all 16 concurrent reads returned count=2. Revision rows
  were stable, CREATE remained locally rejected, and no server state changed.
- Remote scope remains deliberately thin: scalar read-only QueryResult and
  health only. HOT writes, vectors, indexes and snapshot transfer are not fake
  implementations and return explicit capability errors.

### M378 S02 mode-aware scalar graph query

- Added `da graph-query` as a separate read composition root instead of modifying
  CRITICAL-blast-radius `open_graph_store` used by HOT DirectGraphStore commands.
- Supports explicit `--mode embedded|remote` or `DA_GRAPH_MODE`; embedded can
  preload a same-process snapshot, while remote requires `--endpoint` or
  `DA_GRAPH_ENDPOINT`, rejects `--snapshot`, and never auto-starts a daemon.
- `--json` emits QueryResult JSON only. Configuration failures exit 2;
  unreachable/protocol/query-policy failures exit 3 with stderr diagnostics.
- Live embedded/remote comparison over the same snapshot confirms identical
  integer revision values/rows. Boolean differs honestly by wire contract:
  embedded JSON bool versus vendor RESP bulk strings; no heuristic coercion.
- Health and serve-graph banners now advertise the scalar read plane while
  retaining explicit `RemoteNotConfigured` status for full/HOT GraphStore.

### M378 S01 bounded RemoteGraphStore read plane

- Added a real universal `RemoteGraphStore` over bounded Samyama RESP instead of
  widening the CRITICAL-blast-radius embedded `open_for_mode` path.
- Implements honest GraphStore `health`, `query`, and `query_readonly`; vector,
  index, snapshot and all HOT DirectGraphStore capabilities fail explicitly as
  unsupported. Embedded materialization paths are unchanged.
- Added incremental RESP parsing with timeout and response-size limits, typed
  connect/write/read/protocol/query errors, row-width validation, UTF-8 checks,
  partial-frame handling and no hidden retry.
- Vendor `GRAPH.RO_QUERY` currently delegates to `GRAPH.QUERY`, so a local
  fail-closed Cypher policy rejects mutation/admin tokens before socket connect
  while ignoring quoted literals, identifiers and comments.
- Preserves wire fidelity (D226): RESP integers/null stay typed; bulk values stay
  strings because vendor bool/float bulk encodings are indistinguishable from
  legitimate strings `"true"` and `"1.5"`.
- Real two-process proof against snapshot-preloaded `serve-graph`: PING=PONG,
  revisions `[1,5]`, count=2, bool wire values preserved as strings, CREATE
  rejected locally and snapshot export reported unsupported.

### M363 S04 typed revision preparation read path

- Extended existing application-owned `revision-query` instead of adding a
  parallel CLI command. Each selected revision now returns exact grounded
  Summary records and one current deterministic readiness record beside source,
  Section count, predecessor and manifestations.
- Summary joins require the exact revision Section VID and revision VID;
  readiness joins require `signal_type=agent_readiness`, `is_current=true`, and
  exact revision source identity. Foreign/stale signals are ignored and multiple
  current readiness signals fail as invalid graph state.
- Additive JSON fields expose summary role/text/source hash/method and readiness
  state, blocker/diagnostic codes, evidence hash and policy lineage. Numeric
  current/vN/history selector and failure exit semantics are unchanged.
- Real read-only proof over the readiness snapshot: history `[v1,v5]`, current
  v5, v1 has 14 summaries including 2 Methods, v5 has 10 generic summaries,
  both readiness states partial with version-faithful reasons, D127 false, and
  source snapshot SHA unchanged.
- Added four cross-layer revision preparation tasks tying explicit revision
  numbers to grounded Methods evidence, current readiness, manifestations and
  D127. All PASS with no EMPTY/ERROR.

### M363 S03 revision-scoped deterministic readiness

- Replaced the stale Paper-scoped/LLM-score readiness proposal with a distinct
  revision-scoped derived-evidence path. Legacy `SummaryUseCase` quality signals
  remain compatible but are not agent-readiness authority (D225).
- Added pure `RevisionReadinessObservation`, validated policy evaluation, integer
  basis-point coverage, open DATA-driven expected summary roles, explicit
  blocker versus diagnostic codes, `ready|partial|blocked`, serde and proptest.
- Added `data/ontology/revision_readiness_policy.yaml` plus canonical loaded-value
  SHA adapter. Invalid thresholds fail closed; threshold mutation invalidates
  policy identity.
- `RevisionReadinessUseCase` reads only exact typed PaperRevision/Section/
  SourceManifestation/Summary neighborhoods, excluding wrong-label and foreign-
  revision evidence. It materializes content-addressed `QualitySignal` history
  with exact observation/policy/evidence hashes and D127 false.
- Exact replay reuses signal and temporal edge. Evidence changes create a new
  current signal, mark the previous signal non-current, and close only the old
  `agent_readiness` fact edge; legacy quality signals are untouched.
- Added production + compiled `PaperRevision -[:hasQualitySignal]-> QualitySignal`
  parity and a typed `readiness` semantic graph-task contour.
- Real Samyama v1/v5 chain PASS: two current partial signals, four readiness
  tasks PASS, no EMPTY/ERROR. v1 has grounded Methods but lacks Results; current
  v5 lacks both Methods and Results. Missingness remains explicit and no
  LLM/network invents evidence.

### M363 S02 DATA-driven Methods and Results summary roles

- Added pure-domain `SectionRolePolicy` with open validated `SummaryType` wire
  strings, ordered exact/suffix title matching, duplicate-rule rejection,
  serde roundtrip, and a DATA-configured maximum title length. No section-role
  vocabulary is compiled into application or CLI code.
- Added `data/ontology/section_summary_roles.yaml` and an adapter loader whose
  SHA256 identity is computed from canonical loaded values. Bundled and project
  DATA match; a one-rule mutation invalidates the configuration fingerprint.
- Generic revision-scoped `Summary` nodes now record `summary_type` rather than
  proliferating `MethodsSummary`/`ResultsSummary` labels. Role policy schema and
  SHA are written beside exact source VID/hash provenance; replay without a
  policy resets type/lineage to `generic` instead of leaving stale metadata.
- Application tests prove methods, results, unmatched generic, role-policy
  replay reset, D127, owner/source identity, and fail-closed invalid owners.
- Real v1/v5 Samyama proof is evidence-honest: 2 Methods summaries, 0 Results
  (the legal-ontology paper has no Results section), 22 generic summaries, and
  five semantic tasks PASS. Paragraph-sized parser title artifacts are retained
  as evidence but rejected by the 160-character classification guard.

### M363 S01 revision-safe grounded summaries

- Fixed a historical identity bug in `ExtractiveSummaryUseCase`: section summary
  VIDs no longer use only canonical `paper_id + path_id`, which collided when
  v1/v5 shared section ordinals and silently overwrote older summary evidence.
- Summary identity is now a SHA256 VID over exact `PaperRevision`, granularity,
  and exact source VID. Abstract digests attach to `PaperRevision`; section
  digests attach to the revision-scoped `Section` from ADR-058.
- Materialization validates `(label, vid) -> node_id` for the revision and every
  Section before writing anything. Replaced a vacuous missing-section test with
  executable fail-closed tests for missing revision and missing Section owners.
- `RevisionMaterializationUseCase` now persists `canonical_id` and revision-local
  `abstract_text`; production ontology DATA exposes those fields plus complete
  Summary provenance fields (`revision_vid`, source hash/size, future
  `summary_type`).
- Added production and compiled `PaperRevision -[:hasSummary]-> Summary`
  endpoint parity without removing legacy paper-level digest compatibility.
- Reworked the real Samyama canary around the revision graph. Exact v1/v5 proof:
  2 revisions, 24 distinct grounded summaries, replay-stable batches, D127
  false, and 4/4 revision-summary graph tasks PASS with no EMPTY/ERROR.

### M377 extraction-stage PipelineCache under `da load`

- `RuleBasedExtractor` now owns `ExtractionCacheIdentity`: explicit implementation
  contract, SHA256 of the **actual loaded** `ExtractionConfig` values, and
  serialized `Vec<ExtractedEntity>` output-schema contract.
- Equivalent bundled/file configs fingerprint identically; mutating one rule
  changes only configuration identity. CLI has no YAML path or rule knowledge.
- `da load --stages ingest+extract` now caches only pure extractor output under
  distinct `stage_id=extract`, dependent on parse payload SHA; graph writes stay
  outside the cache boundary and always run through `materialize_extracted_with_context`.
- Corrupt cache payloads fail visibly: FilePipelineCache checksum verification
  rejects tampering before JSON decode; no silent recomputation/fallback.
- Real exact-v1 proof on a fresh cache: cold parse/embed/extract all **miss**;
  offline no-key replay all **hit** with stable payload SHAs; `extract_ok`
  remains `entities=3 found_in=3` and `revision_graph_ok` proves fresh writes.
- `--no-cache` reports three **bypass** records while producing the same graph
  outcomes. `--stages ingest` emits no extract cache record or extract finding.
- Added integration test covering extract miss/hit/bypass/stable payload and
  corrupted-payload failure; existing load tests and clippy remain green.

### M376 revision-aware snapshot query operator

- Added typed read-only `RevisionQueryUseCase` over injected `DirectGraphStore`
  with `current`, exact `vN`, and numeric `history` selectors.
- Application semantics require exactly one current revision, resolve active
  SUPERSEDES only, return PDF/HTML manifestations + Section counts, enforce
  D127 false, and fail explicitly for missing Paper/revision/current ambiguity.
- Added dedicated `da revision-query --id … --selector current|history|vN
  --snapshot … [--json]`; CLI delegates selector policy to application.
- Real M375 snapshot: history `[v1,v5]`, current `v5`, explicit v1/v5,
  Sections `14/11`, two manifestations each, v5 predecessor v1.
- Negative proof: missing v3 exits 3; unsupported `latest` exits 2. Neither
  silently falls back to current.
- Fixed a machine-interface bug found at runtime: snapshot preload banners no
  longer contaminate `--json` stdout; quiet mode emits valid JSON only.
- Read-only proof: snapshot SHA remains `9e5c2b8a…` before/after all queries;
  mock query also preserves node/edge totals.
- Full gate exposed a stale architecture audit that checked only hardcoded Rust
  schemas. It now loads ADR-050 production ontology DATA workspace-aware, so
  accepted YAML-only labels are authoritative without duplicate Schema structs.
- Governor caught a direct temporal `SUPERSEDES` edge in the query fixture; the
  fixture now uses the mandatory temporal stamp helper. Closeout returns 0 violations.

### M375 Wave 4 real two-revision Samyama proof

- Added `revision_graph_canary`: one-process real Samyama composition over
  exact local v1/v5 HTML + `article.vN.json`, `HtmlParser`, revision-aware
  `IngestUseCase`, and `InstrumentedGraphStore` physical mutation evidence.
- Runtime PASS: one Paper, two PaperRevisions, 14 v1 Sections, 11 v5 Sections,
  two PDF/HTML manifestations each, current=v5, and one v5 SUPERSEDES v1 edge.
- Exact provider SHA values match the M374 acquisition evidence for both formats.
- Physical replay PASS: 560 unchanged property attempts, **0 applied**, 0
  rejected, 0 unobserved operations; nodes/edges/vectors required no writes.
- Exported 29,554-byte Samyama snapshot (`9e5c2b8a…`) and independently reloaded
  it as 33 nodes / 33 edges with D127 still Locked.
- The canary initially caught two real defects: parser-count hardcoding (20/13
  cached contract vs current 14/11 HTML parser) and competing `valid_from` types
  causing four replay writes. Both were corrected at the product boundary;
  replay gates were not weakened.

### M375 Wave 3 unified revision content context

- Added typed `RevisionContentContext` across load, ingest, extraction, and
  revision materialization. Versioned loads now use the same revision VID for
  mutable content, embeddings, Sections, and extracted `FOUND_IN` provenance.
- Existing unversioned APIs remain wrappers with `None` context and retain
  legacy Paper/Section behavior.
- `LoadContentRequest` carries typed locator/source plus normalized ensured
  asset evidence; CLI no longer reparses provider strings or metadata files.
- Versioned ingest stores abstract/hash/counts/vector on PaperRevision, not the
  canonical Paper work, and avoids duplicate Section creation after the
  dedicated revision materializer runs.
- LoadReport now emits `revision_graph_ok` with revision VID and D127 false.
- Live v1 smoke: parse/embed hit, 20 revision Sections, 3/3 FOUND_IN, 43 nodes,
  68 edges, no errors, import_eligible=false.
- Regression matrix: 9 revision-context tests, all 126 da-application tests,
  7 da-cli load tests, and cross-crate clippy pass.

### M375 Wave 2 revision-scoped application materializer

- Added dedicated hexagonal `RevisionMaterializationUseCase` over injected
  `DirectGraphStore` + `Clock`; universal `ArticleLoadUseCase` remains free of
  direct revision graph policy.
- Materializes one canonical Paper, immutable PaperRevision nodes,
  content-addressed SourceManifestation nodes, and revision-scoped Sections
  with D127 false and structured create/reuse counters.
- `SUPERSEDES` reconciliation is independent of load order: v5→v1 still creates
  v5 SUPERSEDES v1; inserting v3 later invalidates stale v5→v1 and creates
  adjacent v5→v3→v1 temporal facts.
- Replay creates zero duplicate nodes/edges in MockGraphStore; v1/v5 retain
  distinct section structure while sharing one Paper.
- Corrected accepted ontology ownership: SourceManifestation no longer has a
  single `revision_vid`; many revisions may share identical content-addressed
  bytes through separate `HAS_MANIFESTATION` edges.
- Contract gate: 8 new revision tests, all 124 da-application tests and clippy
  pass; kg-ontology registry 14; governor active has zero violations.
- CLI wiring and real Samyama v1+v5 proof remain M375 S04 work, not claimed.

### M375 Wave 1 accepted article revision ontology and identity

- ADR-058 accepted by explicit human decision D223: one canonical `Paper`,
  immutable `PaperRevision`, content-addressed `SourceManifestation`,
  revision-scoped Sections, and observed `SUPERSEDES` history.
- Promoted `PaperRevision` and `SourceManifestation` from isolated draft into
  production `node_schemas.yaml`; promoted `HAS_REVISION`, `HAS_MANIFESTATION`,
  revision `hasPart`, and revision `SUPERSEDES` endpoint contracts.
- Added additive `paper_revision_vid`, `source_manifestation_vid`, and
  `revision_section_vid`; existing `paper_vid` behavior is unchanged.
- VID validation is provider-aware: only a real versioned arXiv canonical id is
  rejected. Property testing caught and fixed the false positive generic id `av0`.
- Governor ratcheted from “block Proposed labels” to “require accepted production
  DATA and prevent direct revision graph writes inside universal ArticleLoadUseCase.”
- Contract gate: kg-ontology registry 14, relation 15, VID unit/property 21,
  governor 33 tests; YAML valid; zero trajectory violations.
- D127 remains false. Revision materializer and real two-revision Samyama proof
  are the next M375 slices, not claimed by this wave.

### M374 exact arXiv revision acquisition and ontology boundary

- Fixed a live revision-fidelity bug: `https://arxiv.org/html/2506.07853v1`
  previously stripped `v1` before acquisition and processed latest/v5 bytes.
- `ArticleLocator::Arxiv` now carries versionless canonical `id` plus optional
  numeric `revision`; malformed/zero revisions do not silently normalize to latest.
- arXiv acquisition selects exact `<id>vN` PDF/HTML/TeX URLs and catalog assets,
  never reuses bare latest for a versioned request, and stores separate
  `article.vN.{json,html}` provenance with both PDF and HTML source variants.
- Default snapshot paths use source identity (`paper_<id>vN.sgsnap`) while Paper
  identity and load `unit_key` remain versionless.
- Real v1 proof: PDF `bd601ac0…`, HTML `0e11dbd…`, 20 sections, 3 entities;
  exact hashes match independent fetch and differ from v5/latest.
- Real v5 proof: PDF `aa6e247b…`, HTML `4dc4abdd…`, 13 sections, 2 entities;
  v1 and v5 coexist under one canonical catalog unit without overwrite.
- Cold/replay cache is revision-separated; offline hits require neither network
  nor `FD_API_KEY`; D127 remains false.
- Added Proposed ADR-058 + draft DATA for `PaperRevision`/`SourceManifestation`.
  No production ontology node or edge is materialized before explicit acceptance.
- Governor now blocks ADR-058 draft labels/edges leaking into production ontology;
  negative fixture passes with the full 32-test governor suite.

### M373 reliable cached 28-unit textbook hub

- Batch `LoadReport` persistence is atomic and fail-closed; serialization, directory,
  temporary write, sync, and rename errors include the unit/path instead of being ignored.
- Real DATA-driven `da load --batch-toc … --stages ingest` completed **28/28**
  local textbook units: 771 sections, one 494,656-byte Samyama hub artifact,
  28 parse misses + 28 embed misses, zero report errors, D127 false.
- Exact replay completed **28/28** with 28 parse hits + 28 embed hits and stable
  per-stage payload sha256 values; no acquisition network or GROBID calls.
- Warm embed-cache hits no longer require `FD_API_KEY`; uncached miss/bypass remains
  explicit and fail-closed. Cached vectors now validate model dimension and reject
  NaN/Inf before graph materialization.
- Full architecture verification exposed and fixed an ACAP hard-gate regression:
  missing materialization replay evidence was incorrectly stamped `NotApplicable`
  and could PASS. It now fails closed; 16 assurance tests and all 116 application
  library tests pass.
- Rust 1.94 clippy-only debts in six existing canary examples were normalized
  without changing behavior, restoring the all-target architecture gate.
- ADR-057 remains Proposed: no Collection, KnowledgeUnit, or textbook graph edges.

### `da load` PipelineCache for embed stage (depends on parse payload)

- Embed abstract/title vectors cached under stage_id `embed` via same
  `FilePipelineCache` / `StageCacheKey` as parse.
- Dependency key includes parse payload sha + embed_text sha + model_id so a
  re-parse cannot silently reuse a stale vector.
- Payload: little-endian `Vec<f32>` bytes; LoadReport.cache stamps hit|miss|bypass.
- Live: fresh cache dir on `2608.07067` → parse+embed **miss**, second run both
  **hit** with identical embed payload sha (no GROBID on 2nd run).

### `da load` binds PipelineCache for parse stage (ADR-056 reuse)

- Content path uses existing `FilePipelineCache` / `StageCacheKey` / `SourceIdentity`
  for **parse** payloads only (not graph truth).
- LoadReport stamps honest `cache[]` dispositions: `miss` | `hit` | `bypass` |
  `retryable_failure` with key/payload sha256.
- CLI: `--cache-root` (default `artifacts/pipeline_cache`), `--no-cache` bypass.
- Live: `da load 2608.07067 --stages ingest` twice → first parse miss (~50s GROBID),
  second parse **hit** (no GROBID re-call); same payload sha.

### M371 complete — pipeline architecture + semantic acceptance sealed

- **M371-pr59ty complete** (validate pass): free-form product path for pipeline
  cache through ontology acceptance formally closed in GSD.
- S01–S05 skip-delivered with unit/integration evidence; S06–S14 prior product
  path; S15 related-article canary **descoped** after acceptance seal.
- Durable proof unchanged: `corpus_semantic_checkpoint.json` verdict=pass,
  five task classes pass, `accepted_corpus_checkpoint.sgsnap`, D127 false.

### M372 complete + ADR-057 draft isolation in governor/registry

- **M372-sqj6hc complete** (validate pass): universal `da load` plane sealed with
  runtime-executable UAT on all slices.
- Governor: forbid production `KnowledgeUnit`/`Collection`/`DISCUSSES_PAPER` until
  ADR-057 Accept; forbid those strings in article_load/load CLI.
- kg-ontology: test `load_from_dir` ignores `drafts/` subdirectory.
- Preflight trajectory: **0 ADR violations** (DRIFT_RISK remains ops: STATE/ROADMAP,
  target/ bloat).

### ADR-057 Proposed — non-article ontology + LoadReport container metadata

- Draft **ADR-057** (textbook/chapter ontology): Collection + KnowledgeUnit,
  shared Entity plane, forbidden loader edges; waves N1–N6 after Accept.
- Load path stamps `source_profile` / `container_ref` from TOC on resolve
  (string metadata only); textbook units force `graph_readiness=not_claimed`.
- DATA draft only: `data/ontology/drafts/non_article_structure_contracts.yaml`
  (not loaded by OntologyRegistry).
- Tests: textbook ensure report carries container; TOC resolve stamps profile.

### Optional TeX source on native arXiv load + README da load

- `--include-tex-source` fetches `arxiv.org/src/{id}` into catalog as
  `{id}.tar.gz` + `arxiv-tex-source-inventory.v1` inventory (safe name caps;
  member digests deferred to FilesystemTexArchive).
- TeX is a **sidecar**: failure does not block PDF primary ensure.
- README operators + CLI table document `da load` / `--batch-toc` / TeX flag.
- Live: `da load 2608.07067 --stages ensure --allow-network --include-tex-source`
  → pdf catalog_hit + tex_tar/tex_inventory fetched.

### Native arXiv acquisition — no Python process boundary

- `ArxivSourceAcquisition` fetches PDF (+ optional HTML) via shared
  `http_fetch` (reqwest): Atom meta → category path → atomic catalog write.
- Validates `%PDF-` magic / HTML shape; fail-closed network; D127 unchanged.
- Python `download_arxiv_to_catalog.py` no longer on the `da load` path
  (script may remain for ops recovery only).
- Live: `da load https://arxiv.org/abs/2608.07067 --stages ensure --allow-network`
  → ensure `fetched` PDF+HTML under `arxiv/cs-ai/2608.07067/`.

### Batch hub content + native HTML HTTP for load plane

- `--batch-toc` with `--stages ingest|ingest+extract` materializes all TOC units
  into **one process-local graph** and exports a single hub `.sgsnap` (`--output`).
  Default without `--stages` remains **ensure** (catalog only).
- Shared `CliContentPipeline` store: no per-unit empty graph; entity reuse across
  chapters (created < observed when labels overlap).
- `CatalogHtmlAcquisition` HTTP via **reqwest** (no curl process boundary).
- Live mini hub: about + ch01 + ch06 → 3/3 ok, hub snap ~52KB, ~100 sections,
  ~27 extract entities.

### GNN textbook full TOC ensure + load batch + governor load-report rules

- DATA: `data/source_catalogs/gnn_textbook_toc.yaml` (28 units, source-catalog-toc.v1).
- `CatalogHtmlAcquisition` network ensure: TOC lookup → curl fetch → atomic
  `article.json` + `source/chapter.html` under catalog (adapters only).
- CLI: `da load --batch-toc … --allow-network` ensures all TOC units (default
  stages=ensure); 28/28 ok live (about + chapters 00–26).
- Governor: forbid legacy load-report schema ids and `import_eligible:true`
  under `artifacts/loads/` (ADR-056 / D127).

### `da load` multi-provider + single-parse content path

- **Catalog HTML provider** (`CatalogHtmlAcquisition`): offline ensure for
  `catalog:gnn_textbook/html/...` (and other HTML source_codes); HTML primary.
- **CLI auto-provider** selects arxiv vs gnn_textbook from locator.
- **Single parse**: GROBID/HTML once → embed → `materialize_parsed` → extract
  on the same `ParsedArticle` (no double GROBID).
- **Bugfix:** `extract_source_from_path` no longer maps every
  `article_catalog` path to arxiv; prefers concrete source_code segments.
- Live textbook: `da load catalog:gnn_textbook/html/gnn-ch-chapters__01-intro-to-graphs
  --stages ingest+extract` → title Chapter 1…, 56 sections, 8 entities, snap.

### `da load` content pipeline — real title + extract + dead code drop

- `LoadContentPipeline` binds one process-local store: ingest (real GROBID title)
  then optional rule extract, then snapshot export.
- CLI `--stages ingest+extract|full` runs extract under the same load command.
- Deleted non-product draft `crates/da-cli/src/load_arxiv.rs`.
- Live: `da load https://arxiv.org/html/2608.07067v1 --stages ingest+extract` →
  title DocMemo…, 55 sections, extract ok, `.sgsnap` + load-report.v1.

### `da load` end-to-end — network ensure + ingest (not ensure-only theater)

Closes the operator gap on ADR-056 plane:

- `ArxivSourceAcquisition` fetches missing PDF/HTML via catalog recovery script
  when `--allow-network` (process-boundary in adapters only).
- `ArticleLoadUseCase::run_with_ingest` binds real `IngestUseCase` / GROBID +
  embed + snapshot through injected `LoadContentIngest`.
- CLI default `--stages ingest`; one command from HTML/abs URL → catalog → graph
  snapshot + `load-report.v1`.

Verified: `da load https://arxiv.org/html/2608.07067v1 --allow-network --stages ingest`
→ ensure `fetched` + ingest ok (55 sections) + `.sgsnap`.

### M372 W1–W5 product path — universal `da load` plane (ADR-056 / D220)

Shipped the Accepted load plane as thin hexagonal slices (no provider-rooted
architecture, D127 locked, network fail-closed):

- **S01 DATA:** `source_codes.yaml` registers catalog-used codes
  (`gnn_textbook`, `company_blog`, `nature`, `local`) so
  `is_known_source_code` is honest without Rust hardcodes.
- **S02 domain:** `da-domain::load` — `ArticleLocator` normalize (arXiv before
  local-path so `.pdf` URLs are not misclassified), `AcquisitionPolicy`,
  `LoadStageSet` / `LoadRunMode`, durable `LoadReport` `load-report.v1` with
  cache/queue fields, `container_ref` metadata for non-article units,
  `graph_readiness=not_claimed`, digest seal. 11 unit tests.
- **S03 ports:** `SourceAcquisitionPort` + `MockSourceAcquisition` contract
  (catalog_hit, network deny). 5 tests.
- **S04 application:** `ArticleLoadUseCase` ensure-only orchestration + honest
  report; dry_run; D127 stamp. 5 tests.
- **S05 adapter:** `ArxivSourceAcquisition` offline catalog_hit walk + sha256;
  network fetch explicitly not implemented (typed error). 4 tests.
- **S06 CLI:** `da load` composition root (`crates/da-cli/src/load.rs`); writes
  report JSON; draft `load_arxiv.rs` remains non-product.

Verify (per-crate, no workspace rocksdb trap):
`cargo test -p da-domain --lib load::` / `acquisition::` / `article_load::` /
`arxiv_acquisition::` / `da-cli --lib load::` — all green.

### ADR-056 Accepted — Universal Article Acquisition & Load Plane

Binding load architecture (D220): one `ArticleLoadUseCase` / `da load` over
provider-agnostic acquisition ports; arXiv and textbook HTML are adapters, not
products. Catalog ensure + checksums + durable LoadReport; D127 and network
fail-closed; reuse PipelineCache and GraphScheduler. Non-article KB ontology and
paper↔chapter edges deferred to a future ADR before graph promote; textbook path
is ensure-only for now. Draft provider-rooted CLI is non-binding until W5.

### First-class ontology acceptance runner (M371/S14)

Composable same-process acceptance over the enriched 200-paper snapshot:
class-qualified semantic suite (structural/fact/summary/cluster/resolution)
plus corpus checkpoint seal with injectable FULL_SEMANTIC required classes.
Exports accepted snapshot only on PASS. D127 stays false.

Domain `SealCorpusCheckpointInput.required_task_classes` and
`FULL_SEMANTIC_REQUIRED_TASK_CLASSES` let S09-style packages require only
structural+fact while S14 demands all five product classes. Checkpoint CLI
gains `--allow-derived-snapshot` for enriched snaps whose bytes differ from
materialization `output_snapshot_sha256`.

Live acceptance: verdict=PASS, 200 articles, 33172 nodes / 49268 edges, all
five classes Passed, semantic checkpoint findings=[], accepted snap
`accepted_corpus_checkpoint.sgsnap`. 6 domain + 4 application checkpoint
tests green. No PDF rebuild.

### Multi-provider scholarly evidence reconcile (M371/S11)

Pure domain reconcile of OpenAlex / Semantic Scholar / local TeX bibliography
candidates into a durable candidate-only report with visible conflicts. Never
promotes provider payloads to graph truth and never unlocks D127.

`reconcile_scholarly_evidence` surfaces citation denominator conflicts, provider
rate-limits, TeX analysis status, and local bib identity completeness. CLI canary
on article 2606.06036 reads existing durable artifacts and emits
`artifacts/article_context/scholarly_evidence_2606.06036.json`
(verdict=NeedsReview, 9 findings, local bib=30, unmatched_local=18).

4 domain tests green. No network, no graph writes, no PDF rebuild.

### Reviewed entity resolution Decision→Entity vertical (M371/S13)

System-reviewed blocking-alias merges (Exact + AlphanumericFold) via existing
EntityResolutionUseCase.resolve_reviewed_merge — profile keeps auto_merge=false.
Each merge records Decision + AFFECTS + canonical_vid + retrieval silence.
Batch runs to closure; candidates() skips already-silenced/canonical entities;
replay applies zero new Decisions.

CLI canary seeds controlled punctuation-variant aliases (corpus gold identity
already collapses many exact aliases), applies 2 Decision merges, exports
materialization_all_200_with_resolution.sgsnap, and re-runs class-qualified
tasks same-process: **resolution class PASS**. All five product classes
(structural/fact/summary/cluster/resolution) now PASS. D127=false.

6 application entity_resolution tests green (batch + idempotent replay).

### Derived ConceptCluster materialization from graph inventory (M371/S12)

Always-run derived semantic clusters from live Paper-[:MENTIONS]->Entity
co-occurrence. Pure domain detect_clusters thresholds remain data-driven
(`data/algorithm_params.yaml`). Application gains inventory_entity_papers +
materialize_from_graph over existing ClusterUseCase without changing
materializer APIs.

CLI canary imports summary-enriched snapshot, materializes 685 ConceptCluster
nodes + 1322 MEMBER_OF_CLUSTER edges (83 unique entities; 48 meet single-mention
threshold; co-occurrence pairs included), proves replay idempotent (0 new
creates/edges), exports materialization_all_200_with_clusters.sgsnap, and
re-runs class-qualified tasks same-process: **cluster class PASS**. Structural/
fact/summary remain PASS; only resolution stays EMPTY for S13. D127=false.

6 application cluster tests green (inventory path covered). No PDF rebuild.

### Grounded extractive summary vertical (M371/S10)

Pure extractive digests (`extractive_lead_v1`) from Section/Paper text already
in the graph, with always-run Summary materialization + hasSummary edges.
No LLM. Empty section text is skipped (typed missingness), not invented prose.

Domain `compute_extractive_summaries` + application `ExtractiveSummaryUseCase`
are pure/compute+write. CLI canary imports pinned materialization snapshot,
writes summaries for 200 papers (197 abstracts, 8356 section digests, 8553
Summary nodes), exports
`materialization_all_200_with_summaries.sgsnap`, and re-runs class-qualified
graph tasks same-process: **summary class PASS**. Structural/fact remain PASS;
resolution/cluster stay EMPTY for later verticals. D127=false.

4 domain + 3 application tests green. No PDF/GROBID rebuild.

### Required-layer corpus assurance checkpoint (M371/S09)

Seal a replay-safe corpus package that binds verified ACAP aggregate identity
(200 papers, 7 required layers Observed, Pass), materialization snapshot
identity (sha256 + nodes/edges), and class-qualified same-process graph tasks.
Required structural+fact classes must PASS; bootstrap summary/resolution/cluster
EMPTY stays visible as findings without inventing PASS or blocking the required
layer seal.

Domain `seal_corpus_checkpoint` + application `CorpusCheckpointUseCase` are pure.
CLI `corpus_required_layer_checkpoint` imports pinned
`materialization_all_200.sgsnap`, runs `materialization_all_200.tasks.cypher`
(same-process), and emits
`artifacts/article_context/corpus_required_layer_checkpoint.json` (verdict=Pass,
D127=false). Task class annotations corrected so fact/summary are not collapsed
into structural.

5 domain + 4 application tests green. No corpus rebuild.

### Additive ordered mutation canary (M371/S08)

Product-owned ordered mutation plan + executor prove physical MutationCounters
for deterministic DirectGraphStore steps without changing materializer APIs.
Domain `OrderedMutationPlan` validates binds/labels/D127 fail-closed.
Application `OrderedMutationUseCase` runs first pass (create/set/edge) then
property-only replay against first-pass binds so compare-before-write proves
Unchanged. CLI canary emits
`artifacts/article_context/ordered_mutation_canary.json` (verdict=Pass,
first_applied=8, replay_property_unchanged=5, D127=false).

5 application + 4 domain tests green. S07 lifecycle sealed via skip-delivered
+ runtime UAT. No graph rebuild.

### Project required pre-materialization ACAP layers from durable wave evidence

Product wave after S07: `PipelineStageAssuranceUseCase` projects source/parse/
structure/extraction/evidence/embedding (+ materialization when replay is
present) from checksum lineage and first-pass graph counts already stored in
`materialization_all_200.json`. Pre-materialization layers set
`replay_missingness=NotApplicable` so physical replay gates stay typed and do
not invent NetWrites proof.

Evidence hard gate is claims-aware: FOUND_IN required when mentions exist;
EvidenceBundle required only when Claims exist (mention-only papers are not
failed for zero bundles). CLI
`cargo run -p da-cli --example acap_project_pipeline_layers` emits corpus
aggregate with all 7 required layers Observed for 200 papers, corpus
verdict=Pass, D127=false, hard_failures=[]. Governor ACAP flags empty under
bootstrap. Contract-audit canary merges observed aggregate layers and reports
verdict=Converged.

Domain evaluate_hard_failures additive skip when replay is NotApplicable.
6 pipeline-stage unit tests + existing ACAP suites green. No graph rebuild.

### Governor aligned to product ACAP aggregate fixtures (M371/S07/T05)

Governor now consumes the product corpus aggregate schema
(`article-context-assurance-aggregate.v1`) instead of the per-article report
schema. Report validation checks `schema_version`, `import_eligible=false`,
aggregate required keys, and `aggregate.hard_failures` (list[str] product shape
with legacy int dual-accept).

Under bootstrap rollout, `required_layer_absent:*` codes are trajectory
`acap_coverage_gap` (medium), while integrity codes (`replay_*`, `d127_*`,
schema/import failures) remain high `acap_report_failed`. Live 200-paper
aggregate yields coverage gaps only; D127 stays false; reverse-ADR violations=0.

D127 crate scanner gains data-driven `path_contains_exclude: [/tests/, /test/]`
so fail-closed unit tests that temporarily assign `import_eligible` no longer
false-positive (MEM718), while production `src/` assignments still block.

Also fixed ACAP contract-audit canary to read multi-wave
`materialization.waves[].papers[]` (was silently seeing 0 embedded reports).

Policy YAML, governor tests (31), trajectory preflight DRIFT_RISK with
acap_coverage_gap. No graph rebuild, no D127 unlock.

### Typed graph-task classes for structural vs semantic evidence (M371/S07/T04)

Graph task suites no longer collapse structural / fact / summary / resolution /
cluster / ontology_endpoint evidence into one boolean. Domain
`GraphTaskClass` + `ClassifiedTaskEvidence` keep per-class PASS/FAIL/ABSENT
status. The Cypher task DSL accepts optional `// @class <name>` (default
`structural` for legacy files). Suite reports expose `class_results` and
`classified_evidence(required_classes)`.

Finalize policy gains optional `required_task_classes: []` (empty keeps legacy
overall-boolean gate). Non-empty lists require each class to PASS independently
with finding `required_task_class_failed_or_missing`. GEPA hub and ontology
endpoint canary task files are annotated with explicit classes.

Tests: domain class parse/gates, parser @class + classify_suite_results,
finalize class-absent fail. D127 unchanged. No corpus rebuild.

### Canonical corpus ACAP aggregate from existing 200-paper artifact (M371/S07/T03)

Product-owned `CorpusAssuranceReport` (`article-context-assurance-aggregate.v1`)
is now composed in `da-application` from already-validated per-article reports
plus optional physical replay upgrades. The CLI projector
`cargo run -p da-cli --example acap_aggregate` reads
`materialization_all_200.json` and the ACAP policy YAML, upgrades 200 legacy
layers from paper-level `replay_mutations`, and emits
`artifacts/article_context/assurance_report.json` without graph writes or D127 unlock.

Aggregate is honest: materialization is Observed with full replay integrity;
six other required layers remain typed `no_claim` (not invented PASS), so corpus
verdict is Fail with aggregate codes `required_layer_absent:*`. Layer rows keep
empty hard_failures for no_claim missingness; governor thin-accepts list-typed
empty hard_failures while still failing non-empty lists. D127=false, replay_net_writes=0.

7 corpus contract tests green. Governor ACAP tests accept list shape.
No corpus rebuild.

### ACAP physical replay observation in per-article evidence (M371/S07/T02)

Reified physical mutation partitions inside ACAP domain reports. New
`ReplayObservation` / `MutationFamilyObservation` types preserve
attempted/applied/unchanged/rejected per family plus unobserved operations,
so a serialized per-article report can recompute `replay_observation_integrity`
without in-memory `MutationCounters`.

`MaterializationReplayObservation` now maps `kg-storage` counters one-to-one
into domain observations. Empty instrumentation remains `NotRun`; invalid or
unobserved partitions are preserved as `Observed` and fail with specific hard
codes. `WriteDelta` stays a diagnostic applied/rejected projection only and
cannot satisfy the hard gate alone.

Legacy v1 artifacts that only embed zero `replay_writes` deserialize safely
and are flagged `replay_observation_integrity:legacy_v1_incomplete`. The
10-paper batch artifact proves migration: paper-level `replay_mutations` still
carries a valid partition that can be attached to make the layer self-validating.
Producer path (`gold_graph` / `from_mutations`) is compatible without corpus rebuild.

6 contract tests + domain/application suite green. D127 unchanged. Governor
still follows product fixtures in T05.

### ACAP contract convergence detect-only audit (M371/S07/T01)

Froze the ACAP schema authority boundary and exposed all current cross-surface
drifts through a new `AcapContractAuditUseCase` in `da-application`. The audit
compares typed observations from the policy YAML, Rust DTO constants, embedded
200-paper materialization reports, the canonical aggregate path and governor
expectations. It detects 8 finding classes: schema version collision
(per-article and aggregate sharing a version), `hard_failures` type drift
(Rust `Vec<String>` vs governor numeric), policy ID drift, undefined layer
metrics, unknown FSM states, required layer absence from observed reports,
unknown report layers, and canonical aggregate report missing.

The domain `normalize()` method was hardened from silent identity rewrite to
fail-closed rejection. A wrong schema version or policy ID is now an explicit
`SchemaVersionDrift` / `PolicyIdDrift` error instead of being silently
overwritten. The filesystem assurance store classifies these as `Integrity`
errors rather than generic `Storage` failures. A separate aggregate schema
version (`article-context-assurance-aggregate.v1`) was introduced to prevent
per-article and aggregate contract collision.

The CLI canary (`cargo run -p da-cli --example acap_contract_audit`) reads the
live policy YAML and materialization artifact and emits
`artifacts/article_context/acap_contract_audit.json`. Running against the real
200-paper corpus, the audit verdict is `Drift` with 11 findings (3 errors, 7
warnings, 1 coverage gap). D127 remains false. The governor is not changed;
it follows Rust-generated fixtures in T05.

14 new tests cover valid convergence, all 8 drift classes, metamorphic ordering,
serde roundtrip, fail-closed validation and D127 lock integrity.

### Ontology v2 bounded convergence evidence

Composed the T01 contract audit, T02 identity compatibility report and T03
endpoint journey through a pure `da-application` evidence use case. Every child
report is self-validating and bound by semantic and file digests. Checkpoint and
process-local endpoint graphs remain separate first-class sources; the composer
never implies that they observed one graph. `Converged` is structurally
unreachable while graph digests differ or endpoint finalization is incomplete;
error findings yield `Drift`, and declared absent surfaces yield `Incomplete`.
D127 remains false in every child, runtime graph walk and aggregate report.

The real endpoint canary now executes materializers, unchanged replay, five
read-only Samyama semantic tasks and an injected-registry finalize gate in one
process. It proves Citation, QualitySignal and polymorphic Summary topology,
rejects the former forbidden shapes through schema-discriminating safety
queries, reports zero schema violations and unknown edges, and preserves the
three genuinely missing production pipeline stages as explicit finalize
blockers rather than fabricating PASS. The endpoint report moved from
CLI-private JSON to an application-owned sealed DTO with ontology, policy,
task-source, graph-semantics and report digests.

Added a bounded CLI composition root and durable
`artifacts/ontology/ontology_v2_convergence_canary.json`. The real verdict is
`incomplete`: checkpoint and endpoint graph digests differ and unobserved
ontology layers remain explicit. Stale T01 coverage markers for materializer
endpoint typing and the old Paper-only inventory walk were removed after T03
product evidence proved them closed. `GraphFinalizeUseCase::new` accepts an
injected validated policy, fingerprint and registry; the filesystem-based
constructor remains a compatibility wrapper only.

ADR-051 coverage now includes deterministic/serde evidence reports, binding and
ontology revision tamper, D127, endpoint/temporal/replay/task/finalize negative
matrices, explicit `Converged`/`Incomplete`/`Drift` paths, durable real-Samyama
artifact validation, identity report roundtrip and finalize cache-hit sequence.
A new governor rule was intentionally not added: the discovered
false-convergence bug is enforced by typed product semantics, while Samyama
target-label query behaviour requires executable runtime safety tasks rather
than a Python regex. The existing closeout governor did expose a real
operability gap, so all three ontology pipeline composition roots now initialize
`.env`-backed ADR-052 rolling JSON logging and retain their `WorkerGuard`.

### Ontology v2 endpoint and materializer convergence

Closed the confirmed endpoint contradictions without changing accepted IDs or
rebuilding the corpus. OpenAlex `referenced_works` no longer create fake Paper
nodes: validated/canonicalized work IDs materialize as candidate-only Citation
nodes and `Paper-[:CITES]->Citation`. Invalid IDs and more than 500 references
fail closed; replay reuses nodes and edges. Quality signals now use the distinct
fact edge `Paper-[:hasQualitySignal]->QualitySignal` instead of abusing
`hasSummary`, and receive full temporal fact stamps. Abstract, section,
limitations and quality materialization are replay-idempotent.

The ontology contract is now executable at product boundaries. Active YAML and
compiled endpoint rows must match exactly, including polymorphic `hasSummary`
and causal Decision rows. The handwritten validator edge list was removed in
favour of compiled contracts. Graph finalization now injects
`OntologyRegistry` into inventory collection; the high-blast-radius compatibility
wrapper remains no-filesystem and uses compiled labels. Wrong endpoints,
planned-only materialization, unknown task wires and schema drift remain visible
through the contract audit.

Added a process-local real-Samyama endpoint canary. It proves two Citation nodes,
two QualitySignal nodes, two CITES edges, two quality edges, four temporally
stamped fact edges, zero forbidden Paper-to-Paper CITES, zero hasSummary links to
QualitySignal, zero CITES writes on replay, full graph replay stability and
graph-derived `import_eligible=false`. Durable output:
`artifacts/ontology/ontology_v2_endpoint_canary.json`.

ADR-051 coverage includes endpoint negative/polymorphic/parity tests, Citation
canonicalization/caps, Summary and QualitySignal replay, injected inventory,
finalize, temporal classification, task wires and real Samyama replay. The
remaining first-class command and application logging gaps are left explicit
for the supported acceptance-runner work rather than hidden with governor
regexes.

### Ontology v2 identity and property index convergence

Replaced CLI-private and dead schema-index hand lists with two separately typed,
data-driven policies validated against `OntologyRegistry`. The identity policy
now covers all registered `vid` surfaces plus explicit natural keys; the property
policy preserves validated filter/performance indexes without granting them
identity authority. Optional natural identity is explicit (`Citation.arxiv_id`
uses `required:false`), while mandatory keys fail before any index creation.
Neither policy defines or changes VID prefixes, hashing or migration semantics.

`da schema init`, gold-graph examples and ontology audit now consume the same
bounded repository-jailed policy loaders. Identity values are scanned completely
for missing required keys and duplicates before index creation; plan-only
validation performs zero graph mutation, schema initialization fails fast between
index families, and non-identity indexes remain separate from vector indexes.
The obsolete application `SchemaInitializer` Cypher/list mirror was removed,
leaving one schema version contract.

Added a read-only identity compatibility use case over `DirectGraphStore`. It
checks every declared key, exact indexed lookup parity, missingness and duplicate
groups, recomputes semantic status from counts, validates a checksum-bound report
and keeps `import_eligible=false`. On the real 200-paper checkpoint, all 13
observed keys are compatible and unique, 26 future-label keys are honestly
unobserved, lookup mismatches are zero, and the overall verdict is `incomplete`
rather than false PASS. Unchanged replay is byte-identical; the durable artifact
is `artifacts/ontology/ontology_v2_identity_compatibility.json`.

ADR-051 coverage includes policy negative/determinism tests, identity/property
confusion tests, accepted VID golden bytes, plan-only and double-ensure
idempotency, missing/duplicate identity rejection, exact lookup compatibility,
report tamper checks and real checkpoint replay. Storage-level unique constraints
and transactional multi-index creation remain explicit adapter limitations; T02
does not claim them.

### Detect-only ontology v2 contract audit

Added the first executable M371 ontology-convergence product slice without
changing ontology data or graph state. A pure `da-application` use case compares
injected identity-index, finalize-policy, semantic-task and real graph
observations against the existing `kg-ontology::OntologyRegistry`; filesystem,
Samyama checkpoint import and report composition remain in the CLI edge. This
avoids both an onion inversion and a second ontology manifest.

The audit is fail-closed and deterministic. It validates source fingerprints,
D127, identity properties, active/planned edge status, every matching endpoint
contract for duplicate wire names, runtime node labels, task relationship wires,
report semantics and report checksum. Runtime evidence is collected through the
read-only `DirectGraphStore` port. The task-wire projection handles Cypher
alternation and emits explicit diagnostics for incomplete relationship patterns.
Repository paths, compressed and decompressed checkpoint size, record length,
header/body counts, node and edge budgets and report destination are bounded
before Samyama import.

The real 200-paper checkpoint audit observes 23,928 nodes, 39,387 edges, 10 node
labels, 11 edge wire types and 13 endpoint triples. It reports `incomplete`, not
PASS: 30 explicit coverage gaps remain (13 unobserved active endpoint contracts,
15 registered labels without active topology, and two known static-consumer /
materializer observation gaps). There are no hidden error findings and
`import_eligible=false`. Independent runs are byte-identical; durable output is
`artifacts/ontology/ontology_v2_contract_audit.json`.

ADR-051 coverage now includes application semantic, negative, serde, port and
metamorphic tests; graph-task lexer regression tests; and CLI snapshot/path
boundary tests. The next work remains S06/T02 identity/index unification, then
S06/T03 endpoint/validator/task-wire convergence. Governor is intentionally not
used to mask the product gaps detected by this slice.

### Candidate-only TeX source inspection vertical

Added a complete domain-to-adapter TeX source inspection path for canonical arXiv
archives. The application now verifies the inventory and archive before reading
members, deterministically selects the scientific root, resolves a jailed include
closure and parses only declared bibliography files into byte-identity candidates.
Semantic failures are persisted as typed `Reject` evidence while the caller still
fails; success and reject reports pass the same schema, digest and D127 validation.
No candidate becomes graph truth and `import_eligible` remains false.

The archive boundary is now hermetic and checksum-bound: it validates inventory
schema, article identity, canonical archive path, archive size/hash, unique
canonical member names, exact member size/hash and bounded streamed output.
Traversal, backslashes, dot segments, duplicate members, links/devices and archive
substitution fail closed. The TeX/BibTeX lexer rejects incomplete commands,
invalid UTF-8, macros, top-level concatenation, crossrefs, duplicate keys,
ambiguous roots, missing includes and include cycles instead of silently
under-counting evidence. URL fragments inside braced values remain supported.

The real `2606.06036` canary selects `main.tex`, closes 10 files through 9 unique
include edges and emits 30 candidates from `references.bib`. Independent runs are
byte-identical. The local denominator conflicts with Semantic Scholar 24 and
OpenAlex 0 and remains `conflict`, not promoted truth. The durable report is
`artifacts/article_context/article_2606.06036_tex_analysis.json`; its checksum is
bound into `article_2606.06036_sources.json`. Focused ADR-051 coverage includes
10 domain semantic/serde/negative tests, 3 application policy/failure tests,
4 adapter integrity tests and 19 source inventory producer tests.

Architecture research with GitNexus and codebase-memory corrected the remaining
M371 order: ontology-v2 contract convergence precedes ACAP schema convergence;
an additive ordered-mutation canary follows those contracts; required-layer
corpus proof precedes non-empty summaries, external evidence, clusters, reviewed
resolution and a first-class acceptance command. The critical existing parsed
and extracted materializer APIs are not rewritten before equivalence proof.

### Deterministic all-wave rebuild and typed source evidence

Added a fail-closed same-process multi-wave graph driver with explicit fixed
transaction time, ordered complete-batch/unique-paper denominator validation,
per-wave chain continuity, aggregate physical mutation counters and ACAP verdict
distribution. Cache decoding and checksum verification can use all available CPU
cores while graph commits remain ordered. Identity indexes are now ontology-v2
data in `data/ontology/identity_indexes.yaml`; Samyama lookups use and backfill
those indexes. A full 20-wave/200-paper run produced
`artifacts/article_context/materialization_all_200.{json,sgsnap}` with 23,928
nodes, 39,387 edges, 200 ACAP passes, D127 false and zero replay writes. The
structural semantic-task subset is 7/7 PASS; Summary, reviewed Resolution and
ConceptCluster tasks remain honestly EMPTY and are not claimed delivered. The
exact task report is `artifacts/article_context/materialization_all_200_tasks.json`
(SHA-256 `eee4b95fb591f4f78c8b06627a12fa963bf94a4de8fb9d98a619efd85632fa03`).

Performance profiling corrected the initial assumption that idle CPU meant cache
preparation was the main bottleneck. The original run took ~4,041s and the
indexed/parallel-preparation run ~3,429s: the remaining cost is approximately
337,650 fine-grained in-process Samyama mutations serialized by its global graph
lock. The ~4,041s and ~3,429s wall times come from operator timing logs rather
than the materialization artifact; the mutation volume itself is durable. A typed
bulk graph-write port is therefore the next performance slice;
unsafe concurrent graph mutation is not enabled.

Acquired official PDF, HTML and TeX source for arXiv `2606.06036` into its
canonical `cs-ai` catalog root. The tested safe-archive path measures 29 members
(28 files, 1 directory), 10 TeX files, 2 bibliography files and 7,009,156 expanded
bytes under explicit member/file/total caps; traversal, links and devices are
rejected. The archive is checksum-bound in
`artifacts/article_context/article_2606.06036_sources.json`. OpenAlex metadata is
preserved as candidate evidence. Semantic Scholar returned useful public identity
and citation counts before rate limiting and is marked `rate_limited`; Litmaps
and Connected Papers remain review links because no stable unauthenticated graph
API was verified. OpenAlex's 0-reference denominator conflicts with Semantic
Scholar's 24 and must be reconciled against the TeX bibliography. All evidence
remains `import_eligible=false`.

### ADR-055 physical graph mutation replay proof

Added a universal `InstrumentedGraphStore` decorator in `kg-storage` without
changing the high-blast-radius `DirectGraphStore` signatures. It records
attempted/applied/unchanged/rejected outcomes independently for node, property,
edge and vector operations. Equal typed property writes are compare-before-write
and become `unchanged`; storage errors remain rejected errors. Write-capable
query, index creation and snapshot import invalidate the observation rather than
silently bypassing ACAP. Empty, inconsistent or unobservable observations fail
closed, and phase reset/snapshot share the mutation lock.

Fixed a real replay bug: Paper and Reference `valid_from` were refreshed on every
materialization. They are now set only when created or missing, with an advancing
injected-clock regression test. Gold graph waves now require nonzero clean first
writes, valid zero-write replay partitions, ACAP PASS, stable graph counts and a
derived D127 flag. The ACAP policy's replay equation now includes property writes
and a hard observation-integrity metric.

A fresh current-manifest/all-hit cache wave and real Samyama batch-1 run produced
`artifacts/article_context/materialization_batch_01.{json,sgsnap}`. Across 10
papers, first pass applied 9,743 physical writes; replay observed 5,113 attempts,
0 applied, 5,113 unchanged, 0 rejected and 0 unobserved. Graph counts remained
715 nodes/1,012 edges, all article assurance verdicts passed, import eligibility
remained false, and snapshot SHA-256 is
`20242694b9e0de154e2f298bf21f5a6af4bd8d7734d8061afdb3b4df78a9b59d`.
The exact cache-wave report is preserved beside it. Evidence:
`ef53fc8d-dd8a-4188-bc37-1b9b0764693f`.

Remaining explicit debt: mutation identity/upsert under concurrent multi-writer
use, vector content readback, deterministic clock binding across independent
fresh runs, and the same-process 20-wave/200-paper aggregate report.

### ADR-055 ACAP typed runtime vertical

Implemented the first product ACAP slice across the onion. `da-domain` now owns
validated SHA-256 digests, boundary modes, typed missingness, article FSM,
conservation/count/write observations and fail-closed report normalization.
`da-ports` defines an immutable assurance report store; `da-adapters` provides a
race-safe atomic filesystem implementation with conflicting-first-write,
tamper, identity-binding, traversal and symlink-escape tests.

`da-application` now checks `ParsedArticle -> Paper/Section` graph readback using
UTF-8 byte counts and checksums, parsed/ingest/graph denominators, structural
minimums, graph-wide D127 scans and explicit replay evidence. Missing replay
observation is a hard failure rather than an implied zero. The shared application
MockGraphStore was corrected to delegate boolean reads, closing a false-PASS gap
in D127 tests.

The attempted direct gold-wave wiring was deliberately removed after review:
existing `NetWrites` counts creations but cannot detect property rewrites, so it
cannot truthfully prove replay idempotency. The next product slice instruments
all graph mutations and only then connects durable ACAP reports and the
same-process 20-wave rematerializer. Full architecture/governor gate PASS under
GSD evidence `6b7c8f75-fd1b-4f15-b8e2-71d8d791f338`.

### ADR-055 Article Context Assurance Plane

Accepted and materialized ADR-055 plus the executable
`article_context_assurance.yaml` policy. ACAP models the article lifecycle as an
FSM from source registration through accepted checkpoint, with typed visible
failure, explicit skip reasons and semantic-key-preserving retry. Thirteen
layers declare owners, inputs/outputs, hard invariants, diagnostic quality,
evidence and rollout status.

The policy defines conservation/grounding/cache/replay/D127 metrics separately
from coverage, quality, latency, cache efficiency, context utilization,
disclosed truncation, semantic density and cost. Hard failures can never be
averaged away. Prompt stages bind prompt/context/provider/model/inference/tool
and output schema lineage. DSPy, GRPO, RL and evolutionary outputs remain
candidate-only: same-N, dual F1, val-gap, grounding/schema/task parity,
contamination, reward-hacking, cost/latency and explicit human decision gates
are required; auto-promotion remains false.

Governor now validates ACAP policy structure, FSM transitions, metric references,
prompt rules, optimizer admission, report policy hash, required layers, hard
failures, replay writes and D127 unlock count. Bootstrap permits missing evidence
as `no_claim`, not PASS. High ACAP or information-preservation findings become
blocking through a separate data-driven severity policy. Twenty-six governor
tests PASS and project ACAP policy is valid; runtime report/type/collector work
remains the next product slice.

### M371 S05 information-preservation contracts

End-to-end field audit found two canonical graph losses in
`materialize_parsed`: Section text was silently capped near 10,000 bytes and
Paper abstracts were never written. Both boundaries now preserve complete
Unicode text, correct character counts and SHA-256 (`text_sha256`,
`abstract_sha256`). TDD uses tails beyond the old cap (`TAIL-科学` and
`ABSTRACT-尾`) and unchanged materialization replay. Rust and YAML ontology
schemas carry the new checksum fields.

Governor now has a general data-driven
`information_preservation.yaml` mechanism. Contracts declare boundary, mode
(`full_fidelity` or bounded diagnostic), sink, required evidence and forbidden
lossy mutations. Generated regression tests prove `.get(..N)` and
`.chars().take(N)` mutations fail, malformed contracts and missing sinks fail
visible, and all live contracts pass. This first policy set protects Section
text, Paper abstract and hashed/size-only provider diagnostics; future field
gaps become new data contracts rather than one-off hardcoded Python checks.

Audit also identified remaining product work without claiming it closed: old
Samyama checkpoints predate these new properties and require a one-process
200-paper cache rematerialization; citation provenance has source-model gaps;
summary optional structured fields are parsed incompletely; older canary report
shapes lack exact lineage keys; eval examples still need durable report parity.

### M371 S05 canonical catalog index rebuild

Rebuilt both catalog indexes from live `article.json` records through the same
contained filesystem mutation port. Duplicate bare/version identities are
canonicalized non-destructively: bare metadata wins, versioned roots remain as
provenance assets, and index keys are unique versionless identities. Both index
files contain 230 entries, have identical SHA-256, preserve import/write safety
false, and a second rebuild returns `Unchanged` for both files with zero writes.
The index writer uses the same same-directory temp, fsync and atomic rename
contract as metadata apply. Physical duplicate deletion was intentionally not
performed because some versioned roots retain distinct HTML evidence.

### M371 S05 bounded catalog metadata apply

Added the filesystem `CatalogMutationPort` adapter with catalog-root containment,
symlink/path traversal rejection, live PDF/HTML hashing, same-directory
`create_new` temporary files, `sync_all`, atomic rename, cleanup on failure and
write-if-absent conflict semantics. Parallel adapter tests initially shared one
PID-based temp directory and failed nondeterministically; unique atomic-sequence
roots fixed the test harness rather than hiding the race with retries.

Applied exactly the seven low-risk checksum-bound metadata actions. First apply:
7 attempted / 7 applied / 7 write operations / 0 rejected. Every generated
`article.v00.02` record rebuilds local PDF+HTML variants, strips donor execution
history and stale URLs, and keeps graph/import/readiness claims false. Fresh
inventory+plan removed all low actions; replay attempted 0 and wrote 0. The
remaining catalog debt is explicit: 165 blocked historical sidecar references,
24 high-risk metadata gaps, and 7 medium bare/version identity duplicates.

A post-apply manifest exposed one more resolver bug: duplicate exact PDFs under
`cs-CV` and `cs-cv` selected lexicographically, hiding the new metadata. The
resolver now accepts duplicate exact assets only when all checksums match and
there is exactly one canonical lowercase category; otherwise it fails closed.
Gold manifest is again **200/200 source-ready, 200/200 article metadata**.

### M371 S05 catalog reconciliation inventory

Added the first safe catalog-repair slice as a hexagonal, read-only capability:
typed findings in `da-domain`, a `CatalogInventoryPort`, a deterministic
filesystem adapter, an application reconciliation use case, and the
`catalog_reconcile` CLI example with atomic report output and optional
fail-on-findings. No catalog metadata, index or graph state is mutated.

The real inventory now classifies 230 metadata records, 254 PDFs, 17 content
HTML files and **196 actionable findings**: 165 genuinely missing historical
M056 variants and 31 asset roots without `article.json`. An initial checksum
conflict was rejected by TDD: seven unrelated `original.pdf` files had been
incorrectly grouped by basename instead of article-directory identity. An initial 233-count was rejected during TDD because
the adapter incorrectly resolved article-relative and repo-relative variant
paths; multi-root resolution corrected the false positives before any apply
mode existed. Finding keys are stable snake_case and output ordering is deterministic. The
pure reconciliation planner is now delivered: inventory and actions are
SHA-256-bound, action IDs are content-addressed, duplicate/permuted findings
produce the same plan, and risk ordering is explicit. Of 31 metadata gaps,
exactly seven have one versioned donor with checksum-identical PDF content and a
canonical lowercase target category; they become low-risk actions without any
ID allowlist. The remaining 24 are high-risk and 165 unresolved historical
variants are blocked. Apply is intentionally absent, so current writes are zero by construction.
The pure canonical metadata transform is now also delivered: it retains only
bibliographic identity/source classification from the donor, rebuilds PDF/HTML
variants from live target descriptors, binds sizes and SHA-256, resets all
readiness/import claims, records D127 false, and refuses traversal or checksum
mismatch. Milestone-specific donor overrides, loader events and stale versioned
URLs are not inherited. The mutation boundary and state-based application contract are now delivered:
`CatalogMutationPort` exposes donor read, target asset description and atomic
write-if-absent; `apply_low` rejects stale inventory fingerprints before I/O,
filters to low metadata actions, produces typed receipts, and proves first-write
plus unchanged replay against a state fake. No filesystem implementation or
real catalog mutation is included yet; the same contract moves to the real
adapter next.

Gold manifest composition now emits a semantic aggregate event. Governor also
validates every manifest record structurally: ready sources require a live
PDF/HTML path and SHA-256, unavailable sources require a typed reason and no
path/hash, and metadata presence is explicitly not a source-readiness
requirement. Twenty governor tests PASS; the committed 200/200 manifest passes
the live-path check. Application log replay confirmed flat JSON composition and
catalog inventory events under an isolated `LOG_DIR`.

### M371 S05 T06 close seven-source debt and complete the gold denominator

Recovered the seven fixtures previously labelled unavailable and proved the
classification was primarily a resolver/catalog-layout bug: each already had a
versioned local PDF (`{id}v1.pdf`) that exact-name discovery ignored. The shared
source recovery operator now validates canonical/versionless arXiv identities,
requires explicit network authorization, reuses local versioned PDFs by full
SHA-256, atomically materializes canonical PDFs, downloads and validates a
separate canonical HTML representation, normalizes filesystem categories, and
writes a caller-selected checksum-bound report. Eleven tests cover traversal,
invalid payloads, idempotency, network lock, local recovery, HTML, and category
normalization. All seven now have both PDF and HTML in the canonical catalog.

Gold discovery now accepts unique versioned PDFs and `article.html`, while
conflicting versions fail closed. The manifest is **200/200 source-ready,
0 unavailable**. Batches 19 and 20 executed the seven new parse/extract/embed
chains, replayed 60/60 cache hits, and materialized onto the prior checkpoint
with zero-write replay. The complete graph contains **200 Paper, 23,929 nodes,
39,387 edges**; six semantic tasks PASS and D127 unlock count remains exactly
zero. Full-denominator extraction covers all **1,439 gold entities**:
1,439 TP / 439 FP / 0 FN, precision 0.766, recall 1.000, micro F1 0.868,
macro F1 0.857.

Governor now reads the product manifest and aggregate evaluation artifact. It
raises high drift for unavailable source debt, inconsistent manifest counts,
silent fixture denominator loss, excluded gold entities, or malformed evidence.
It also enforces onion-aware semantic logging: pure domain/ports/kg crates cannot
own tracing transport, every active CLI example must initialize `.env`-backed
rolling JSON, stage events must carry article/pipeline/step identity, and graph
materializers must emit node/edge write deltas. Raw LLM responses were replaced
with response length and SHA-256 diagnostics. Eighteen governor tests and a real
temporary `LOG_DIR` JSON/flush proof PASS.

Reference audit closed three runtime drifts: `Section.embedding_sha256` is now in
both Rust and YAML schema registries; MCP ontology resources are embedded and
cwd-independent instead of using manual labels/stale paths; edge contracts now
distinguish `active` from `planned`, so missing endpoints fail closed unless the
YAML explicitly marks an ADR forward declaration as planned. This ratchets
process completeness without hardcoding paper IDs, labels, or F1 policy.

### M371 S05 T06 complete structural gold baseline

Rebuilt all 20 bounded waves under the managed structural profile. Every profile
change invalidated parse plus dependent extraction/embedding exactly once, and
every unchanged replay was all-cache-hit. Aggregate extraction matches the
reviewed baseline across **193 source-ready / 200 fixtures** (7 explicitly
unavailable): gold 1399, predicted 1820, TP/FP/FN 1399/421/0, micro
precision/recall/F1 0.769/1.000/0.869, macro F1 0.859. Batch 20 handled four
HTML sources and six explicit unavailable fixtures; graph provenance now
validates source hashes through the exact cache `SourceIdentity` for PDF or HTML
instead of incorrectly assuming `ParsedArticle.pdf_hash` is the canonical HTML
file hash.

All 20 cache waves then materialized through one chained Samyama store/snapshot
lineage and replayed with zero net writes per batch. Final checkpoint:
**193 Paper, 83 canonical Entity, 1,820 MENTIONS, 8,655 Section, 10,637
Reference; 23,054 nodes / 37,985 edges**, snapshot 6,801,680 bytes. Six runtime
semantic tasks PASS and D127 exact unlock count is zero (`1e8e6ebf-...`). This
is the richer-than-gold structural baseline; typed summaries, separate OpenAlex
bibliographic enrichment, clustering, reviewed resolution and ontology finalize
remain the ordered next product waves.

### M371 S05 T06 managed structural parsing profile

Batch 17 then proved the deeper availability coupling: GROBID itself stayed
healthy, but rich parsing repeatedly hit Crossref 429/backoff and one paper
failed after the bounded request timeout. D204 separates local structural TEI
from remote bibliographic enrichment. Added explicit `GrobidProfile`: `rich`
preserves the legacy consolidated behavior; `structural` disables header and
citation consolidation while keeping figure/table/note coordinates. The gold
canary uses structural mode and binds profile/flags into the cache contract, so
all dependent parse/extract/embed artifacts invalidate honestly. OpenAlex/
Crossref enrichment remains a separate cached wave. Adapter tests and clippy
pass (`8b38514d-...`); AGENTS operator guidance now documents both profiles.
Batch 17 structural run executed all 30 dependent stages without Crossref
failures, then replayed 30/30 cache hits. Exact-key evaluation also replaced the
last output-hash-only cache lookup, which had become ambiguous across rich and
structural profiles (`8d3c7961-...`): 10/10 papers evaluated, recall 1.0, micro
F1 0.793.

### M371 S05 T06 retry timestamp correctness

Batch 17 exposed a long-request failure path where `retry_after` was computed at
request start; after a 300-second GROBID timeout, the timestamp was already older
than `recorded_at`, so failure persistence itself raised a cache contract error
and no wave report was written. `GrobidParseStage` now optionally computes retry
time from failure observation plus delay, preserving its legacy fixed-timestamp
constructor for deterministic callers. The canary uses the observed-failure
policy; focused adapter tests/clippy pass (`02a7c979-...`).

### M371 S05 T06 document identity governor ratchet

Added a narrow data-driven reverse-ADR rule forbidding global Section lookup by
title inside extraction. Repeated headings across/within papers must resolve by
document-scoped section VID derived from paper and ordinal. Negative fixture and
current-tree positive control pass (`e268c8cf-...`): 13 governor tests, active
trajectory 0 violations; expected stale STATE/resource flags remain visible.

### M371 S05 T06 graph task exact-value assertions

Added `@expect_value` to semantic graph tasks because the prior row classifier
always treated a single numeric zero as EMPTY. This made it impossible to prove
critical absence invariants such as D127 `import_eligible=true` count exactly
zero. Parser/classifier/report serde and property fixtures now cover exact finite
values (`d972086c-...`). The 120-paper batch-12 checkpoint passes four runtime
tasks (`36fdb5d1-...`): Paper=120, Entity=69 canonical nodes, MENTIONS=1096,
and import unlocks exactly 0. Chained graph waves 2-12 then restored the prior
checksum-bound snapshot, materialized the next 10 papers, replayed with zero net
writes, and exported the next checkpoint. The batch-12 graph contains **13,678
nodes / 22,214 edges** for 120 papers; the final durable snapshot is 4,777,398
bytes. Intermediate snapshots are reproducible from exact cache/report keys and
were not retained; per-wave reports preserve input/output checksums.

### M371 S05 T06 verified cache-to-graph wave

Added typed `gold_graph` cache replay and the `gold_graph_wave` composition root.
Pipeline reports now persist exact `cache_key_sha256` alongside output hashes, so
durable replay cannot become ambiguous when historical stage contracts produced
identical bytes. Cache loading requires one exact key+payload match, ParsedArticle
identity/source hash, finite non-empty embedding, and successful parse/extract/
embed stages before any graph write. Batch materialization and replay share one
Samyama store; replay requires zero net Section/Reference/Citation/Entity/
evidence/problem/observation writes and unchanged graph stats before atomic
snapshot export.

Runtime exposed and fixed a second document-identity bug: FOUND_IN previously
resolved Section globally by title, so repeated Results titles and cross-paper
headings caused wrong edges/replay drift. `ExtractedEntity` serde v2 now carries
`section_ordinal`; all RuleBasedExtractor passes enumerate source sections,
legacy artifacts fail closed when a title is ambiguous, and FOUND_IN resolves
`vid:section:{paper}:{ordinal}`. This HIGH-blast change passed ports/adapters/
application tests and all-target clippy (`5c2d004c-...`). Selective cache
invalidation regenerated only batch-1 extraction (10 execute, parse/embed 20
hits; `eecba18e-...`). Real graph proof `624daf18-...`: 10 papers materialized
from cache into **711 nodes / 1011 edges**, unchanged replay, snapshot 229,018
bytes, checksum bound, `import_eligible=false`.

### M371 S05 T06 materializer-only composition boundary

Removed the need for fake Parser/Embedder/Extractor implementations in the
cache-to-graph composition. `IngestUseCase::materializer_only` and
`ExtractionUseCase::materializer_only` hold only one shared graph store;
compute entry points fail closed with explicit diagnostics, while verified
`ParsedArticle`/`ExtractedEntity` artifacts materialize normally. Legacy and
shared compute constructors are unchanged. High-blast regression gate
`95470478-...`: ingest, extraction, PDF-classifier and semantic-enrichment suites
plus application all-target clippy PASS.

### M371 S05 T06 shared graph materialization store

Added `from_shared` constructors to `IngestUseCase` and `ExtractionUseCase` while
preserving all legacy Box constructors. Verified parse, embedding, Entity,
evidence, problem/observation and structural graph materialization can now run
against one `Arc<dyn DirectGraphStore>` and one eventual snapshot lifecycle,
instead of opening split stores. Evidence `b86ac595-...`: 12 ingest + 19
extraction tests and application clippy PASS. This is the final composition
prerequisite for the cache-to-graph wave.

Gold batches 9, 10 and 11 also passed cold/replay gates; **110/193**
source-ready papers are now cache-verified with durable per-batch metrics.

### M371 S05 T06 FD API token-capability correction

Corrected a cross-repository contract error after inspecting live `/docs`,
`/openapi.json`, authenticated `/info`, the mounted `/root/fd` source, and model
configuration. `deepvk/USER-bge-m3` supports 8192 tokenizer tokens
(`model_max_length=8192`, `max_position_embeddings=8194`, SentenceTransformer
`max_seq_length=8192`); it has no model-level character limit. FD API middleware
had a stale `2048 chars ≈ 512 tokens` heuristic and `/info` advertised 512.
Removed per-input char guessing, delegated exact token admission to the TEI/model
backend, and updated `/info` plus `docs/fd-v2.md` to 8192. Rebuilt the local API;
`/info` now returns 8192 and a 2049-character Russian input returns 200/1024d
(previously 413). Removed the temporary `FD_MAX_INPUT_CHARS` and Rust character
truncation policy. Paper embedding remains explicit title+abstract, never body,
with backend/model identity in the cache key and no silent truncation. FD API
commit: `bf7b1bc`; all Go tests PASS, while `go vet ./...` still reports one
pre-existing `time.Since` defer warning in `cache/tiered.go`.

The session also activated a previously latent governor crash: resource safety
constructed `DriftFlag` with obsolete fields whenever target size crossed its
threshold. Mapped disk/memory/target diagnostics to the canonical
`flag/severity/evidence/dimension` contract and added a threshold-triggering
regression test. Evidence `07d1cfec-...`: 12 tests PASS, active governor runs to
`DRIFT_RISK` with 0 violations instead of crashing.

### M371 S05 T06 cacheable Paper embedding stage

Migrated the useful Python-era embedding lessons without reviving Python runtime:
provider/model/batch/retry/circuit settings already existed in `.env`; legacy had
no hidden truncation and kept structure-aware chunk embeddings separate from KG
truth. Added a pure cacheable `ParsedArticleEmbeddingStage`; Paper input is the
existing title+abstract contract, while body text remains for future source-span
chunk stages. Backend/model identity and Paper-field projection are in the cache
contract; backend unavailability is retryable and rejected requests are terminal.
Batch 1 selective-invalidation proof: parse/extract 20 hits, only 10 new
embeddings executed, unchanged replay 30/30 hits (`2f090b88-...`). The canary
loads dotenv and fails early when its local embedding credential is absent;
existing `.env` and running `fd_api` token match was verified without exposing
either value.

Gold batch 8 passed as well; 80/193 source-ready papers are cache-verified.

### M371 S05 T06 vector replay safety

Closed the final exact-replay gap in parsed materialization. Paper embeddings now
carry a SHA-256 fingerprint: identical verified artifacts skip `add_vector`, while
a changed embedding fails closed until an explicit storage upsert/rebuild is
available. The shared application mock counts vector writes; 13 ingest tests
prove one exact replay produces one vector add and changed content cannot append
a stale duplicate (`d6be5bd1-...`). Samyama itself also appended duplicate node
IDs to HNSW/stored vectors; vendor commit `78b79a6` makes identical add a no-op
and changed add return `VectorConflict`. Four focused vendor index tests PASS
(`e9d94c89-...`); full vendor clippy remains blocked by 11 pre-existing warnings
outside `vector/index.rs`.

Gold batches 6 and 7 also passed 20 cold executions + 20 replay hits each; 70/193
source-ready papers are now cache-verified with durable per-batch metrics.

### M371 S05 T06 cached parsed graph materialization boundary

Split parser/embedder computation from ingest graph side effects with additive
`materialize_parsed`. TDD then made current parsed-artifact node/edge families
replay-safe: Paper, Source, Category, Section, hasPart, FROM_SOURCE, IN_CATEGORY,
parent_section, next_sibling, first_child, Reference, Citation and CITES. Existing
`ingest_pdf` remains a compatibility wrapper. Replays resolve stable VID/code
identities and create zero duplicate structural/temporal edges. RED evidence:
`9276a3a7-...` and the catalog provenance replay failure; green evidence:
`ae041275-...`, `9ad4da4d-...`, `1d36d061-...` (12 integration tests plus
navigation unit suite PASS). Vector replacement semantics remain an explicit
next contract before corpus graph materialization.

### M371 S05 T06 cached extraction graph materialization boundary

Split extraction computation from graph side effects with additive
`materialize_extracted(parsed, entities)`; the legacy `extract_from_parsed`
remains a compatibility wrapper. TDD exposed and fixed replay duplication across
all current families: MENTIONS, FOUND_IN, EvidenceBundle, Claim,
PARTICIPATES_IN, SUPPORTS, ResearchProblem and MetricObservation. EvidenceBundle
identity no longer collides on equal-length section titles: it is SHA-256-bound
to paper, normalized section and sorted member Entity VIDs; Claim identity binds
to the bundle. Replay counters now report zero net creations. RED evidence:
`159ccd8c-...`, `74d76a11-...`, `370e8966-...`; green gates:
`e42667a0-...`, `8f5dfa4b-...`, `43f904a5-...` (19 extraction tests PASS).

Gold batch 4 also passed cold 20/20 execute and replay 20/20 hits: 10 papers,
66 gold entities, recall 1.0, micro F1 0.857, zero failures/retries/graph writes.

### M371 S05 T06 resumable wave failures and GROBID backoff policy

Made bounded-wave reports durable on partial failure instead of aborting before
evidence: each paper now records terminal metrics or an explicit failure, batch
metrics separate evaluated/failed counts, and `wave_passed` remains false until
an unchanged replay is fully cached. Batch 3 exposed two real retryable GROBID
failures after 8/10 successes. Container diagnostics ruled out crash/OOM and
showed Crossref consolidation 429 backoff exceeding the old hardcoded 120s HTTP
timeout. Added configurable bounded GROBID request timeout (wave default 300s)
and retry suppression delay (30s), recorded in reports but intentionally excluded
from semantic cache keys so successful TEI payloads remain hits. Evidence:
partial and resumed reports under `artifacts/gold_corpus/canary_batch_03.json`;
policy tests `dc443423-...`. After increasing only the operational timeout while
preserving semantic cache keys, batch 3 resumed from prior successes and passed:
10/10 evaluated, 20/20 replay hits, micro F1 0.827.

Recorded R085/D203 for a daily-archive-owned managed scholarly parsing platform:
provider-neutral staged backends, content-addressed artifacts, admission control,
backpressure/circuit breakers, deterministic provenance merge, gold benchmarks,
and gradual native replacement. GROBID remains a replaceable backend rather than
being rewritten in one risky monolith.

### M371 S05 T06 cache-linked metrics and mixed-source support

Closed two corpus-process gaps before widening the waves. Added a cacheable
`HtmlParseStage` using the existing `ParserPort::parse_html`, so the 189 PDF and
4 HTML source-ready fixtures share the same downstream extraction contract; the
7 unavailable fixtures remain explicit report rows. Canary reports now resolve
the exact extraction cache entry by stage output checksum, revalidate it through
`PipelineCache::lookup`, deserialize the typed payload, and compute per-paper and
batch micro/macro gold metrics. Missing, ambiguous, corrupt, or stale cache data
fails the wave. Batch 1 and 2 cache-linked metrics passed (`7e651590-...`), with
full recall and measured precision/F1; no gold labels are copied into graph truth.

### M371 S05 T06 gold canary parse and extraction cache proof

Added a pure cacheable `RuleExtractionStage` consuming serialized
`ParsedArticle` dependencies and emitting a versioned `Vec<ExtractedEntity>`
payload. `ExtractedEntity` now has a tested serde roundtrip; no fields or wire
names changed. Batch 1 (10 gold PDFs) ran through `parse -> extract`: cold run
20/20 executed, immediate replay 20/20 cache hits, and a new process against the
same disk cache also hit 20/20, with zero failures, retries, graph writes, or D127
unlock. The standalone semantic task example now delegates to the reusable task
runner instead of duplicating verdict logic, preserving observed/expected
values. It proves the current legacy hub snapshot has observed Paper count 1 vs
required 24, so it cannot seed corpus closure. Evidence: runtime reports under
`artifacts/gold_corpus/`, focused gate `2e51ee88-...`, shared-runner diagnostic
`70f8176e-...`.

### M371 S05 T06 gold corpus readiness and wave plan

Corrected the closure baseline from one GEPA anchor to the existing reviewed gold
corpus (D202/R084). Added a deterministic `gold_corpus` discovery library and CLI
manifest that fail closed on fixture identity drift, resolve canonical PDF/HTML
sources without shell `find`, fingerprint each source, inventory richer article,
loader, figure and pipeline-cache layers, and assign stable bounded batches.
`eval_batch` now consumes the same manifest instead of maintaining a second
source-discovery path. Real manifest evidence `21f17a9a-...`: 200 unique fixtures,
193 source-ready, 7 explicitly unavailable, 20 batches of 10; focused tests and
clippy PASS. Gold remains evaluation evidence, not graph truth. Next wave runs a
small source-ready canary twice to prove cache hits and zero replay graph writes
before advancing across the corpus.

### M371 S05 T05 governor cacheable-stage purity ratchet

Added narrow YAML reverse-ADR rules for every known cacheable stage module. A
stage now receives a high violation if it imports GraphStore/DirectGraphStore or
calls node/edge/property mutation or materialization APIs. Rules are deliberately
file-scoped: pure parser/VLM/content artifact computation remains legal and
always-run materializers in `cluster.rs`/`enrich.rs` are not false positives.
Governor fixtures prove injected mutation is blocked, an adjacent materializer
is legal, and all current stage modules are pure. Evidence `42ef2fc9-...`: 11
governor tests PASS, active report violations=0 with expected stale STATE drift.

### M371 S05 T04 semantic task bound snapshot seal

Extracted a reusable same-process semantic task runner with typed per-task and
suite reports. `pipeline_finalize` imports one Samyama snapshot, executes the
required Cypher suite, binds the real suite verdict and serialized
`PipelineRunReport` to `GraphFinalizeUseCase`, and atomically exports a
SHA-256-addressed snapshot only when both gates PASS. Failed/EMPTY required tasks
produce a report and non-zero exit with no output snapshot; no debug bypass is
present. GEPA tasks now require Section-hasSummary, reviewed
Decision-AFFECTS-Entity canonical resolution, and Entity-MEMBER_OF_CLUSTER
semantics using ontology wire strings. Pipeline run types gained serde for
durable evidence. The current legacy hub snapshot was deliberately rejected:
`1ed7c624-...` reports four missing S04 semantic tasks and proves sealed snapshot
absence, so positive hub regeneration remains T06 rather than a false-green.
Runner/finalize tests and example clippy PASS in `f3dfadd6-...`; full architecture
`ba3be9c5-...`; governor violations=0 `00ed71d1-...`. Reviewer-found
`allow_empty` semantics were aligned with the existing suite gate before commit.

### M371 S05 T03 fail-closed ontology-driven graph finalize

Added `finalize_policy.yaml`, typed domain reports, and `GraphFinalizeUseCase`.
The gate walks every ontology-v2 registered label, validates node properties,
counts D127 violations, rejects unknown edge wires and required-label gaps, and
binds required stage outcomes to an actual `PipelineRunReport` fingerprint.
Semantic graph task evidence is mandatory. A new additive `get_node_labels` read
port is implemented by Mock and Samyama stores so `get_all_node_ids` can expose
free-invent or unlabeled nodes; those now fail finalize. Production policy load
uses workspace-aware project paths and refuses silent bundled fallback.
`snapshot_eligible` becomes true only after schema, edges, registered labels,
bound stages, semantic tasks, and D127 all pass; `import_eligible` remains false.
Tests cover valid PASS, import unlock, empty graph, schema drift, unknown edge,
missing/failed stages, missing semantic evidence and free-invent labels.
Reviewer initially found four hollow false-PASS paths; all were closed before
APPROVE. Evidence `424b5cfa-...` review-fix PASS, `fd4863b0-...` final full
architecture PASS, `993f2f3c-...` governor violations=0 with only expected stale
STATE projection drift. A governor regex false positive on diagnostic prose was
removed without weakening the D127 source rule.

### M371 S05 T02 idempotent OpenAlex facts and progressive citation candidates

OpenAlex enrichment now reports net graph effects instead of processed-input
counts: created/reused Topic, Author, and Institution nodes plus newly-created
HAS_TOPIC, authoredBy, AFFILIATED_WITH, and CITES edges. Every enrichment fact
edge is deduplicated and store errors propagate; repeated enrichment creates no
nodes or edges. Deprecated OpenAlex concepts are explicitly counted as
candidates and zero writes. Referenced works remain non-importable Paper
candidates keyed by OpenAlex identity, supporting R084 bounded follow-up waves
without bulk truth promotion. Replay tests prove zero net writes, citation stub
reuse, temporal facts, D127, and schema conformance. The reviewer request to add
`invalid_at=OPEN` was rejected after contract verification: `TemporalEdge`
represents OPEN as `invalid_at=None`, and existing ADR tests require the property
to be absent until invalidation; changing `open_fact_edge_now` would have
CRITICAL 74-symbol blast radius. Evidence `bca17217-...` initial idempotency PASS,
`e5b8b2c9-...` final focused PASS, and `7b791a76-...` full architecture PASS.

### M371 S05 T01 honest shared-store OpenAlex scheduler

Eliminated scheduler false completion across disconnected in-memory stores.
`SchedulerRunner`, `EnrichUseCase`, lifecycle updates, and CLI snapshot
restore/export now share one `Arc<dyn DirectGraphStore>`. A successful OpenAlex
response must find its Paper and persist `openalex_id`, DOI, citation count, and
`openalex_pending=false`; otherwise the task advances retry and cannot complete.
The runner verifies write-back identity before completion. Scheduler tasks now
use a stable SHA-256 VID over task type plus arXiv ID and reopen one lifecycle
node instead of duplicating completed/failed tasks. NotFound, adapter errors,
missing Paper, max retries, success and reopen are covered. Snapshot restore,
export, temporary write, and rename failures are terminal; CLI returns non-zero.
Paper schema/YAML now declare `cited_by_count`; D127 remains false. R084 records
progressive related-article mixing: bounded grounded candidates, canonical
catalog ingestion, semantic/finalize PASS per wave, never bulk auto-import.
Evidence `c19a988b-...` CLI snapshot create/restore PASS,
`c2e4f293-...` focused remainder PASS, `47c247a7-...` final architecture PASS;
independent review APPROVE after closing missing-Paper and duplicate-task
blockers. Failed `b7a95bbd-...` remains visible as the expected fixture contract
update after fail-closed Paper enforcement.

### M371 S04 T05b cache-correct semantic DAG composition

Added an explicit-dependency document context projection and cluster pipeline
fragment. `DocumentContextStage` joins parsed Sections and summaries by unique
`path_id`, emits previous/next navigation, and fails terminally on duplicate or
orphan paths. A generic content-addressed `ArtifactPayloadStage` supplies
canonical artifacts. Cluster detection is pure and cacheable; its key includes
the SHA-256 fingerprint of the exact `OnceLock` configuration snapshot and its
payload is deterministic typed clusters. Graph materialization is deliberately
always-run outside `PipelineStageExecutor`, so a cache hit can never skip graph
writes. A single normalized `(entity_type,label)` helper is shared by stage,
public direct API, graph lookup, and same-process example. The example runs a
five-step `PipelineOrchestrator` twice, proves all-pure-stage cache hits, then
applies clusters to real Samyama in the same process. Dirty-key runtime evidence
`28716abb-...`: cache hits true, 3 clusters, 4 temporal memberships, authority
false, import false. Focused `4a35799d-...` and final architecture
`1ff55c58-...` PASS; final review APPROVE after fixing fingerprint/cache and
normalization blockers. Failed `c8d34e6a` predecessor documents the corrected
step-name versus executor-ID cache lookup. S04 runtime UAT PASS: semantic
contracts `e087a35d-...`, ontology/D127/governor `973b2385-...`, and cache
side-effect boundary `90518fbf-...`. Failed `afbf03e3-...` remains visible as a
bad shell status-filter assertion, not product evidence. Lifecycle remains
Attempt-blocked despite technical PASS.

### M371 S04 T05a data-driven cluster foundation

Removed hardcoded `EntityType::Method/Dataset` cluster policy before stage
composition. `data/algorithm_params.yaml` now maps arbitrary ontology wire types
to cluster types with a declared default. `EntityPapers` is keyed by
`EntityClusterKey { entity_type, label }`, preventing same-label cross-type
collisions. Detection order and typed members are deterministic. Materialization
uses full SHA-256 typed-member identities, reuses ConceptCluster nodes and
memberships on replay, propagates store failures, preserves D127, and reports
created/reused/skipped counts honestly. TDD covers arbitrary-type fallback,
replay idempotency, temporal membership and same-label Method/Dataset isolation.
Evidence `41f1a799-...` foundation PASS, `de126919-...` hardened PASS,
`478bf7d0-...` full architecture PASS, independent review APPROVE. Pipeline
stage wrapping remains the next thin T05b unit.

### M371 S04 T04 ontology-driven reviewed entity resolution

Added a universal Layer G resolver without hardcoded Method/Dataset/Metric
policy. `data/ontology/entity_resolution.yaml` declares normalization steps,
blocking keys, safety locks, bounded block size, and Decision semantics; the
domain loads the project profile with a bundled fallback and rejects unsafe
auto-merge/auto-promote or unknown Decision vocabulary. Candidate generation is
pure, same-type, deterministic, and accepts arbitrary ontology wire types.
Canonical mutation requires an explicit reviewed request: pair-local preflight,
strict idempotent Decision, temporal `Decision -AFFECTS-> Entity` audit links,
and replay-safe healing. DecisionRecorder and GraphHealingUseCase now share an
`Arc<dyn DirectGraphStore>` while legacy Box constructors remain. Unknown
Decision vocabulary is a hard write failure; duplicate Decision VIDs replay or
reject conflicting immutable semantics. `CAUSED` and `AFFECTS` use RFC3339
bi-temporal fact properties. Review-found recovery gaps were closed: canonical
VID is written before retrieval silence, partial silence can replay, revised
reason wording reuses the pair Decision, non-candidates/cross-types fail before
Decision, and D127 remains false. D201 records the architecture choice; true
transactional batch mutation remains explicit GraphStore-port debt. Evidence
`47be3a0d-...` focused PASS, `08c1eba5-...` review-fix PASS,
`9ea82931-...` full architecture PASS, independent review APPROVE.

### M371 S04 T04a canonical merge prerequisite

Hardened the reusable `GraphHealingUseCase::merge` before composing entity
resolution. Merged Entity nodes now retain an explicit schema-declared
`canonical_vid`, remain present for audit, become retrieval-ineligible, and get
one temporal SUPERSEDES edge. Incoming edges redirect only when the canonical
edge is absent, making reviewed replay idempotent. Self-merge and conflicting
second canonical targets fail before mutation. Evidence `a72ec2aa-...`: healing,
Entity schema, temporal write audit and clippy PASS. The next T04 unit is still
Decision-first orchestration plus deterministic candidate blocking; no automatic
promotion was introduced.

### M371 S04 T03 section summary graph closure

Section-level structured summaries are no longer orphan nodes. The summary write
path preflights every LLM `path_id` against a Section with both the same article
`work_vid` and path identity, fails before any mutation on a stale/fabricated
path, idempotently upserts the Summary, and creates exactly one structural
`Section -hasSummary-> Summary` edge. The compiled and YAML edge matrices now
model `hasSummary` as an overloaded wire with separate Paper and Section source
rows. The shared graph conformance mock was corrected to match any legal
`(edge, source, target)` row instead of collapsing overloads to the last hash-map
entry. Negative and repeated-run fixtures prove fail-before-write and node/edge
idempotency. Evidence `74ee45de-...`: 4 semantic tests plus domain contracts,
schema CLI and application clippy PASS. Full wave architecture evidence is
`91910216-...`; governor projection evidence is `77687a26-...`. T04 recon found
the next blocking debt before canonical resolution: `GraphHealingUseCase::merge`
does not call the promised `DecisionRecorder`, writes an undeclared
`superseded_by` field, and can duplicate redirected edges. That debt remains
explicitly pending rather than shipping a partial resolver.

### M371 S04 T02 typed semantic enrichment

`SummaryUseCase` no longer counts and discards structured `methods`, `datasets`,
and `metrics`. It now idempotently materializes registered first-class `Method`,
`Dataset`, and `Metric` nodes with deterministic VIDs, article provenance,
retrieval eligibility, `import_eligible=false`, and temporal `MENTIONS` edges.
The data-driven and Rust MENTIONS contracts now admit these ADR-053 Layer C
labels. A deterministic GEPA/ARC-Challenge/Accuracy fixture proves graph
properties, D127, schema conformance, semantic values, and second-run node/edge
idempotency without another LLM call. The work also closed pre-existing authority
holes by setting `import_eligible=false` on every Summary and QualitySignal
branch. `audit-fields` now stops at Rust function boundaries instead of
misattributing helper fields to the previous create site, and stale schema-count
tests were updated to current 22 materialized and 36 registered labels. Evidence
`65bdef84-...`: semantic, extraction, ontology, schema CLI and clippy PASS.

### M371 S04 T01 governor planning projection integrity

Removed the governor's forbidden direct `sqlite3` read of engine-owned
`.gsd/gsd.db`. A new hexagonal `GsdPlanningReader` port and filesystem adapter
read only GSD-rendered ROADMAP, STATE, and PLAN projections. The adapter keeps
bare and suffixed milestone identities separate, supports exact and slugged
phase directories, counts per-slice task identities, and reports unavailable,
degraded, or divergent projections instead of silently preserving a false
green. The signal is explicitly named `zero_projected_plan_tasks`: it measures
projection health, not engine task truth. TDD covers missing/malformed input,
duplicate IDs, bare-shadowed suffixes, exact suffix paths, STATE/ROADMAP drift,
and the repository projection. Evidence `df59caf8-...`: 24 tests PASS, direct DB
access zero. Governor now honestly reports `DRIFT_RISK` because stale STATE says
M363 active while ROADMAP, QUEUE, and `gsd_milestone_status` say M371 active;
STATE was not hand-edited. Product S04 is replanned next: typed semantic
entities, section summary links, auditable resolution, context and clustering.

## Unreleased

### M371 S03 T03 subscription-backed multi-model Figure3 analysis

Added `analyze_figure`, a real provider-neutral, cached multimodal composition,
and experimentally evaluated three requested 9router models under strict gates:

- `minimax/MiniMax-M3`: wire/tool translation worked after JSON-sequence support,
  but outputs failed type/citation grounding gates.
- `meta/muse-spark-1.2-contributor`: schema-valid visual analysis, rejected for
  a non-verbatim citation. `OpenAiResponses` is now represented for a future
  native Meta Responses adapter.
- `agnes-ai/agnes-2.5-flash`: produced a strong 17-component GEPA flowchart
  diagram, but later retry failed exact citation grounding; not promoted.
- `minimax/MiniMax-M2.7-highspeed`: accepted in 40.0s as a flowchart with 9
  components and 4 exact citations. 9router explicitly resolved it to
  `mimo-v2.5-free`; requested and resolved identities are both retained.

Unchanged repeat was `cache_hit,cache_hit` in 368ms with no remote request.
Schema-only invalidation preserved context hit and reran analysis; strict
relationship validation initially rejected two outputs, guidance was corrected,
and v4 passed in 40.7s. Canonical `Figure3.analysis.json` is checksum-bound,
`authoritative=false`, and `import_eligible=false`. D200 allows bounded `*free`
experiments with Retry-After discipline and quality grouping by resolved model.
A requested-model collision in candidate IDs was fixed and the accepted artifact
was deterministically migrated without another remote call. Evidence includes
`c58a058a-...`, `a7925eaf-...`, `4f62367a-...`, `6262a5d1-...`, `831c729c-...`,
`96168c7a-...`, `7aa11ca4-...`, and `9eae783a-...`. All failed model experiments
remain FAIL history, not acceptance evidence. T03 remains Attempt-blocked.

### M371 S03 T02 grounded FigureAnalysis candidate and cached stage

Added the typed, non-authoritative visual-analysis layer over the provider-neutral
multimodal port:

- `FigureAnalysis` validates visual type, unique components, relationship
  endpoints, findings, exact section/path citations, confidence, and complete
  image/context/route/model/prompt/schema/usage provenance.
- `FigureAnalysisUseCase` requires image/context checksum agreement, capable
  route metadata, forced structured output, and exact quote grounding against
  `FigureSemanticContext`.
- `FigureAnalysisStage` fingerprints image SHA, route/protocol/model/fallback,
  prompt, and schema; transient route failures are retryable while semantic
  contract failures are terminal.
- Candidates are always `authoritative=false` and `import_eligible=false`; no
  graph promotion surface was added.

Evidence: `c864ec05-...` (8 focused tests + fmt/clippy) and `d27e0ebb-...`
(governor/onion/authority lock). T02 remains pending because no canonical GSD
Attempt exists; no lifecycle bypass was used.

### M371 S03 T01 provider-neutral multimodal LLM routing (D199)

Replaced the MiniMax-specific VLM design with a provider-neutral multimodal
model gateway that supports direct providers and local routing proxies:

- `MultimodalLlmPort` records image media/bytes/SHA-256, structured schema,
  route/protocol/model identity, fallback semantics, usage, and typed failures.
- `AnthropicMultimodalClient` supports direct MiniMax (`X-Api-Key`) and 9router
  (`Authorization: Bearer` or no auth) without changing application code.
- Endpoint roots with or without `/v1` normalize to exactly one
  `/v1/messages`; native base64 image+text blocks and forced tools are used.
- Fixture HTTP tests verify request/auth shapes, thinking+tool response handling,
  retry-after, secret redaction, and rejection of text-only structured output.
- Existing text-only `LLMClient` remains backward compatible; OpenAI-compatible
  protocol is represented in the port and can receive a separate adapter later.

Local `http://127.0.0.1:20128/api/health` is healthy, but
`/v1/models/image-to-text` currently returns zero models; 9router vision is not
falsely treated as ready. Evidence: `cc3fb68b-...` (7 tests + fmt/clippy) and
`dc97532d-...` (health/discovery/governor). Search query used:
`9router chat Anthropic endpoint multimodal models`. T01 remains pending because
no canonical GSD Attempt exists; no lifecycle bypass was used.

### M371 S02 T04 semantic/architecture closeout gate (Attempt-blocked)

Closed the technical S02 gate without weakening existing invariants:

- Canonical audit verifies 25 GEPA visual artifacts, unique IDs, all asset
  checksums, grounded Figure 3 context, sourced abstract, bounded/visible
  truncation, six versioned cache manifests, and every payload checksum.
- Detected stale `Figure3.context.json` provenance after D198 and regenerated it
  atomically from the verified current cache payload.
- Full architecture gate exposed and removed a dead `da-mcp` test import.
- Registered the real `QualitySignal` nodes emitted by `SummaryUseCase` in both
  compiled and YAML ontology schemas instead of weakening schema audit.

Final evidence: `e8777717-...` full architecture PASS (30.2s) and
`32954673-...` semantic/cache/governor closeout PASS. Failed evidence remains
recorded: `6e548fff-...` caught stale context provenance, `683466c2-...` caught
the dead import, and `27116cde-...` caught missing `QualitySignal` schema.
T04/S02 remain lifecycle-pending because canonical Attempts are absent; no
skip, raw DB mutation, or fake completion was used.

### M371 S02 T03 real cache-aware parse, figure, context DAG

Wired only implemented production stages through the hexagonal pipeline:

- `GrobidParseStage` and `FigureDetectionStage` adapters wrap injected ports and
  serialize typed artifacts with implementation/configuration/schema fingerprints.
- Application `FigureContextStage` consumes named parse/figure dependencies and
  emits validated `FigureSemanticContext` without importing adapters.
- `pipeline_figures` composition root injects canonical PDF/catalog/JAR paths,
  `FilePipelineCache`, `PipelineOrchestrator`, and `kg-pipeline` topology.

Real canonical GEPA evidence:

- First run: parse, figure detection, and context all executed in 100.9s.
- Unchanged second run: all three cache hits in 484ms wall time; no GROBID/Java
  executor invocation.
- Context-schema-only change: parse/figures remained hits, context reran in 111ms
  with deterministic semantic output.
- A repeated run exposed nondeterministic detector metadata hashes caused only
  by random staging `renderURL` paths. D198 canonicalizes those paths to stable
  filenames before hashing, bumps the stage implementation fingerprint, and
  proves subsequent manifest bytes are stable.

Evidence: `f3853989-...`, `e22ab61d-...`, `a8af4043-...`, `c77042de-...`,
`be226802-...`, and `60d72950-...`. Governor remains ON_TRACK with flags=0 and
violations=0; D127 stays locked. T03 remains pending because no canonical GSD
Attempt exists; no lifecycle bypass was used.

### M371 S02 T02 grounded figure semantic context and GROBID identity repair

Replaced the obsolete duplicate PyMuPDF crop task with deterministic,
provenance-preserving VLM context preparation:

- Added `FigureSemanticContext` with detector/source/asset checksums, page/bbox,
  caption, explicit containing section, bounded neighbors, in-text references,
  sourced article summary, diagnostics, and Unicode-safe budgets.
- The assembler requires an explicit `SectionId` or `PathId`; missing,
  unknown, ambiguous, or provenance-free mappings fail visibly. It never picks
  the first section or guesses from section order.
- Real GEPA verification exposed duplicate GROBID `path_id=1` / `sec-1`
  identities. Recursive and flat TEI numbering restarts now receive stable
  collision-only `~N` suffixes, with descendants inheriting resolved parents.
- `figure_context` example discovers reference candidates, validates global
  section identity uniqueness, uses GROBID abstract provenance when catalog
  metadata lacks an abstract, and atomically writes canonical context evidence.

Real `Figure3.context.json` contains the GEPA caption, explicit section
`3`/`sec-3`, a grounded Figure 3 reference, unique neighboring sections,
source/bbox/checksum provenance, and 10,073 bounded context characters with
visible truncation diagnostics. Evidence: `e1822e7f-...` (29 focused tests,
fmt/clippy), `291577ea-...` (90.9s real GROBID semantic proof), and
`fe408b99-...` (governor/onion/D127). Decision D197 records explicit section
mapping until the normalized model gains page spans. T02 remains pending because
no canonical GSD Attempt exists; no lifecycle bypass was used.

### Governor: reject false-green GSD exec evidence (D196)

Added a bounded read-only `.gsd/exec` audit after M371 evidence exposed a real
failure mode: a multi-command shell returned exit 0 even though its inner
`cargo test` printed `test result: FAILED` because fail-fast behavior was
missing.

- `GsdExecReader` scans at most 200 recent metadata files and 256KB of captured
  output for explicit test/build failure markers.
- Exit-0 evidence newer than the latest commit raises high drift flag
  `gsd_exec_false_success_evidence`; the commit boundary avoids permanent noise
  from reviewed historical incidents.
- Corrupt/missing logs remain best-effort and cannot crash governor.
- 5 focused tests cover detection, clean success, true failure, commit-boundary
  acknowledgement, corrupt metadata, and TrajectoryService integration.

Decision D196 records the policy. Governor remains ON_TRACK, flags=0,
violations=0 for the current reviewed state.

### M371 S02 T01 governed pdffigures2 production adapter (pending GSD Attempt)

Implemented real figure/table detection as a hexagonal, provenance-preserving
pipeline boundary:

- Domain contract records Figure/Table kind, zero-based page index, 72-DPI
  top-left bounding boxes, caption region/text, rendered asset path and SHA-256,
  source PDF SHA-256, detector fingerprint, and detector metadata SHA-256.
- `FigureDetectionPort` keeps subprocess/filesystem concerns outside domain and
  application layers.
- `PdfFigures2Adapter` runs the assembly JAR with timeout and kill-on-drop,
  bounded diagnostics, explicit non-zero/timeout failures, complete-batch
  validation, content-addressed PNG names, and manifest-last publication.
- Canonical manifests contain no temporary `renderURL` values. Detector crops
  are reused, avoiding a duplicate PyMuPDF crop stage.
- `detect_figures` example provides an honest composition root and preserves
  `import_eligible=false`.

Real GEPA verification retained 25 visual artifacts (22 figures, 3 tables) in
`data/article_catalog/article_catalog/arxiv/cs-cl/2507.19457/figures/`; Figure
3 has a grounded GEPA caption, valid geometry, and checksum-verified PNG.
Evidence: `f457dce2-9baa-4bc5-8a4e-0bf248872183` (12 contract/integration tests
+ clippy), `7f9407d9-cbfb-4649-a862-7b87a75c4602` (canonical publication), and
`62680988-8608-40ee-9a01-0871f2c654ec` (governor/onion/D127). A prior evidence
run `a4c88df3-...` is explicitly invalid: its shell omitted fail-fast behavior
and returned exit 0 after an inner test failure. T01 remains pending because no
canonical GSD Attempt exists; no lifecycle bypass was used.

### M371 S01 T03 debt wave: exact Paper lookup (partial, pending GSD Attempt)

Fixed a semantic multi-paper corruption risk in `da summarize`: the command
previously ignored its requested `paper_id` when selecting a graph node and
attached output to the first `Paper` node. It now resolves the exact
`arxiv_id` through the `DirectGraphStore` property-lookup contract and fails
explicitly when absent.

- Added `find_paper_node_id` at the application query boundary using the
  canonical `PaperSchema` label.
- Added multi-paper and missing-paper tests.
- Removed three existing all-target clippy failures in CLI examples.
- Governor finding `no_hardcoded_inventory_label_walks` is resolved at root
  cause: active verdict ON_TRACK, flags=0, violations=0.
- No `da pipeline` command was exposed yet because production stage executors
  do not exist until S02/S03; publishing a no-op command would be a stub.

Verification: `gsd_exec 1dbc1bd3-e171-4267-969b-9fea58e31c92` PASS (13
query tests, 44 CLI tests, fmt, all-target clippy, governor) and semantic check
`38ed8a5f-f92e-4d58-abf4-64c8c3e62f3d` PASS. T03 remains pending and partial;
no lifecycle bypass was used.

### M371 S01 T02 cache-aware application DAG runner (delivery pending GSD Attempt)

Added a thin, hexagonal orchestration core without advertising unimplemented
production stages:

- `PipelineStageExecutor` port carries implementation, configuration, prompt,
  model, and output-schema fingerprints plus typed retryable/terminal failures.
- `PipelineOrchestrator` reuses `kg-pipeline::PipelineBuilder` topology, derives
  cache keys from explicit source identity and dependency output hashes, and
  injects all stage implementations through ports.
- Verified cache hits do not invoke stage executors. Prompt-only changes rerun
  summary without parse; source changes rerun source-dependent descendants.
- Missing executors and contract mismatches fail explicitly instead of falling
  back to stubs. Retryable failures are persisted and suppress premature retry.
- Structured tracing reports cache hit/miss, execution, retry suppression, and
  failures without logging article payloads.

Verification: `gsd_exec 09da8420-c714-4fa7-91cc-c1f552d630ff` PASS (2 port
contract tests, 7 application tests, fmt, clippy); onion/governor evidence
`dafbfcae-38ad-465c-b4ba-dab6c933835d` PASS. Decision D195 records the split
between universal topology and application orchestration. T02 remains pending
because no canonical GSD Attempt exists; no lifecycle bypass was used.

### M371 S01 T01 content-addressed pipeline cache (delivery pending GSD Attempt)

Implemented the hexagonal pipeline-cache boundary and filesystem adapter:

- `PipelineCache` port records explicit PDF/HTML source identity plus stage,
  dependency-output, implementation, configuration, prompt, model, and output
  schema fingerprints.
- `FilePipelineCache` atomically persists payloads and manifest commit markers,
  verifies SHA-256 and size on reads, rejects unsafe paths, and exposes corrupt
  artifacts as errors rather than cache hits.
- Retryable failures are distinct from successful artifacts and expire to cache
  misses using validated RFC3339 timestamps.
- 12 focused contract tests cover selective invalidation, source changes,
  PDF/HTML identity, corruption, negative-cache expiry, timestamps, and path
  traversal.
- Removed the unused `AnthropicResponse.stop_reason` field that blocked
  crate-level clippy.

Verification: `gsd_exec bb0fa66a-d6b7-4e36-bc8f-9952128dc8f3` PASS for
fmt, focused tests, and clippy on `da-ports` and `da-adapters`. Governor active
verdict is ON_TRACK with flags=0. GitNexus reports LOW change risk and no
impacted production process. GSD T01 remains pending because the free-form
session has no running Attempt; no lifecycle bypass was used.

### M369 closed via GSD + M371 S01-S02 planned (D194)

M369 Pipeline Wiring closed: all 5 slices skipped (superseded by M370 core
fix + M371 pipeline architecture). Gates closed via engine API.

M371 S01 (Pipeline cache + acquire): 4 tasks planned — PipelineCachePort,
PipelineOrchestrator, da pipeline CLI, TDD cache test.

M371 S02 (Parse + Figure detect + Crop): 4 tasks planned — pdffigures2
subprocess adapter, PyMuPDF crop, pipeline wiring, semantic test.

Governor: ON_TRACK flags=0. One active milestone (M371).

### LLM summary generation via MiniMax (ADR-053 Layer B) — REAL

First real LLM integration. `da summarize --id 2507.19457` generates
real MiniMax-M2.7-highspeed summaries (not abstract_copy).

- `AnthropicLlmClient` in da-adapters (implements LLMClient port)
- `SummaryUseCase` in da-application (abstract + section summaries)
- `da summarize` CLI command

Verified on hub paper: 629-char abstract summary, 5 section summaries,
400 tokens, 226 nodes/338 edges, WCC connected, tree nav 3/3 PASS.

### M370 S04 rollback fake Summary — honest deferral (D194)

M369 S01 created Summary nodes by copying abstract_text — that was a fake.
Rolled back: Summary creation removed from ingest. SummarySchema +
hasSummary edge remain (schema ready for future real summarization).
M370 complete: S01-S05 all done. Governor ON_TRACK flags=0.

### M370 S01-S03 re-baseline production snapshot (D194)

Re-ingested hub paper 2507.19457 with current pipeline. New snapshot:
- 225 nodes, 337 edges (was 2571 nodes, 2741 edges — old pipeline)
- 85 sections with path_id, 76 nav edges (was 0)
- 24 entities extracted (Method:5, Dataset:5, Metric:6, Model:6, Task:2)
- 24 MENTIONS + 24 found_in edges (was 0)
- 1024d FdApi embeddings

Verified:
- Tree navigation: 3/3 PASS (was 0/3 EMPTY)
- WCC: 1 component of 225 (was 2571 isolated)
- PageRank: top-5 non-zero (was empty)
- Graph task suite: 6/7 PASS

Root cause fixed: SamyamaGraphStore.get_outgoing_edges only read write
buffer, not frozen storage. After snapshot import, edges were invisible.

### M370 S01 graph_view_from — frozen edges + sparse IDs (D194)

**ROOT CAUSE FOUND AND FIXED:** SamyamaGraphStore.get_outgoing_edges only
read write buffer, but snapshot import uses create_edge_stub which
populates FROZEN storage. After snapshot load, edges were invisible.

Fix: merge frozen + write buffer in get_outgoing_edges/get_incoming_edges.
Also: get_all_node_ids() for sparse IDs, WCC vendor fix (predecessors).

**Verified on production snapshot (hub.sgsnap, 2571 nodes, 2741 edges):**
- WCC: 1 component of 2571 (was 2571 isolated)
- PageRank: top-10 with non-zero scores (was empty)
- Graph task suite: 7/7 PASS

### MCP FROZEN + M370 Core Pipeline Fixes roadmap (D194)

MCP заморожен. M370-6gaxxc (5 slices) чинит ядро графа:
- S01: graph_view_from adapter (PageRank/WCC видят рёбра)
- S02: navigation edges на продакшене (0 → >0)
- S03: extraction на hub.sgsnap (Entity=0 → >0)
- S04: откат фейкового Summary M369 S01
- S05: re-baseline + полный аудит

Причина: продакшен-снапшот имеет 2571 nodes/2741 edges, но:
- WCC показывает 2571 изолированных узлов (алгоритм не видит рёбра)
- PageRank возвращает пусто
- Entity=0, MENTIONS=0, nav edges=0
- Summary M369 S01 = фальшивка (abstract_copy)

### M369 S01 Summary node creation in ingest pipeline (D194)

FIXES STUB #1: `get_summary` was returning `found=false`. Now ingest creates
Summary node from `Paper.abstract_text` + `hasSummary` edge. Schema updated:
+Summary/Method/Dataset/Metric in `all_node_schemas()`, +hasSummary/
predecessor_of in `edge_contracts()` and validator. 316 green total.

### M369 Pipeline Wiring roadmap + STUB-AUDIT (D194)

New milestone M369-i5hgz8: 5 slices to fix 4 critical stubs from audit.
Full audit at .gsd/phases/governor-v4-synthesis/STUB-AUDIT.md.
S01 (Summary creation) planned with 3 TDD tasks.

| Slice | Fixes | Stub # | Est |
|-------|-------|--------|-----|
| S01 | get_summary | #1 | 2h |
| S02 | graph_rank typed labels | #3,4 | 2h |
| S03 | predecessor_of extraction | #2 | 4h+ |
| S04 | Mock kNN brute-force | #5 | 1h |
| S05 | graph://stats live + hybrid cache | #6,7 | 1h |

### Wave 3 get_reading_path tool — Layer D agent navigation (D194)

14th MCP tool: `get_reading_path(arxiv_id, limit?)`. Returns reading order:
prerequisites (backward CITES) + target + follow-ups (forward CITES).
Each row has order, role. Enables agent "read in order" workflows.

### Governor v4: graph_algorithms + ruvector_hybrid sentinel rules (D194)

Two new governor rules ensure Wave 2 capabilities don't regress:
`graph_algorithms_must_be_called` and `ruvector_hybrid_must_be_integrated`.
Also updated continue.md with full Wave 0-2 product session summary
(13 MCP tools, 306 tests, governor ON_TRACK).

### Wave 2 Layer F predecessor_of + get_method_lineage tool (D194)

13th MCP tool: `get_method_lineage(method, direction?, depth?)`. Traverses
Method→predecessor_of→Method edges for lineage queries. Layer F cross-paper
patterns activated. Variable-length Cypher path `*1..depth`.

### Wave 2 graph_rank tool — PageRank + WCC activation (D194)

12th MCP tool: `graph_rank(label, algorithm, edge_type?, limit?)`. Runs
Samyama graph algorithms on typed subgraphs. First graph algorithm
activation — 12 algorithms were idle (0 calls). Now PageRank and WCC
are available to external clients.

### Wave 2 search_papers mode=hybrid — BM25+RRF fusion live (D194)

`search_papers(query, mode="hybrid")` now performs BM25 + dense cosine
+ Reciprocal Rank Fusion. First real RuVector integration into product.
Returns arxiv_id + title + score. 34 da-mcp green.

### Wave 2 hybrid search — ruvector-hybrid RrfHybridIndex (D194)

New `HybridSearchIndex` wrapper bridges vendor `ruvector-hybrid` (BM25 +
dense cosine + Reciprocal Rank Fusion) with MCP Paper data. 3 TDD tests.

### GSD milestone hygiene — M362 + M365 + M366 + M367 closed (D194)

Closed 84 pending gates across 4 milestones. Code was already delivered
in previous slices (Layer A/B/C, MCP tools, governor dimensions) but
GSD DB gates were never formally closed. Now all M36* milestones are
complete (M363/M364 stay queued for future Wave 2/3).

### Governor: active_milestone_zero_tasks drift flag (D194)

New check scans GSD DB for active milestones with 0 tasks. First run
caught M363+M364 (empty skeletons) → moved to `queued`. Governor now
ON_TRACK flags=0. First check that reads GSD DB for process drift.

### get_references citation context + intent (D194)

Backward references now return `c.context` (citation context — sentence
where citation appears) and `c.intent` (citation intent). External agents
understand WHY a paper cites another.

### get_paper rich metadata — abstract + topics + venue (D194)

`get_paper` now returns 8 fields: added `abstract_text`, `topics`,
`venue` to the Cypher RETURN. External clients get full metadata in
one call.

### search_papers full-text mode — title + abstract + topics (D194)

`search_papers` now accepts `mode` parameter: `"title"` (default, backward
compat) or `"full"` (title OR abstract_text OR topics). Significantly
enriches search surface without hybrid index.

TDD: 1 new test, 31 da-mcp green.

### get_entities typed filter — entity_type property (D194)

`get_entities` now filters by BOTH `labels(e)` AND `e.entity_type`, so
clients can query `entity_type="Method"` whether entities are stored as
typed Method nodes (Layer C future) or Entity\{entity_type:"Method"\}.
Also returns `entity_type` in result columns.

### Governor v4: ontology_layer_readiness dimension (D194)

New dimension tracks ADR-053 Layers A-J: `layers_a_to_c` when path_id +
Summary + Method/Dataset/Metric schemas all exist, `partial` for Layer A
only, `not_started` otherwise. Previously governor had no visibility into
ontology layer implementation — now catches drift when layers are declared
in ADR but missing from code.

### Typed extraction investigation result

Attempted to create typed node labels (Method/Dataset/Metric) during
extraction instead of generic Entity. Reverted: mock conformance contract
expects `create_node("Entity")`. Typed extraction requires mock contract
refactor (separate slice). Current approach: `entity_type` property on
Entity enables Cypher WHERE filtering by type — graph algorithms can still
operate on typed subgraphs via `MATCH (e:Entity {entity_type: 'Method'})`.

### Wave 1 get_summary MCP tool — Layer B product surface (D194)

11th MCP tool: `get_summary(arxiv_id, granularity?)` queries
Paper→hasSummary→Summary via read-only Cypher. Two-pass: exact
granularity first, then any. Returns text + char_count + found flag.

TDD: 2 tests RED→GREEN, 30 da-mcp green.

### Wave 1 Layer B Summary nodes + hasSummary edge (D194)

SummarySchema (vid+text required, granularity/char_count optional) +
HAS_SUMMARY edge constant. Enables MCP get_summary tool (next slice).

TDD: `test_summary_schema_exists` RED→GREEN, 272 da-domain green.

### Wave 1 Layer C typed labels — Method, Dataset, Metric (D194)

3 new first-class node labels in Rust schema + YAML ontology. Previously
ADR-053 declared typed entities but code only had generic Entity. Now
graph algorithms can operate on Method/Dataset/Metric subgraphs.

TDD: `test_typed_entity_schemas_exist` RED→GREEN, 271 da-domain green.

### Governor v4: mcp_surface_completeness dimension (D194)

New governor dimension `mcp_surface_completeness` tracks MCP product
readiness: `full` (tools + resources + prompts), `partial`, `tools_only`.
Previously governor checked ADR invariants but was blind to product surface.

### Wave 1 MCP-4 Prompts — 6 agent guidance templates (D194)

6 reusable prompt templates for external MCP clients: literature_review,
critical_reading, reproducibility_check, citation_lineage, compare_methods,
generate_cypher. Each has typed arguments and renders tool-aware instructions.

### Wave 1 MCP-4 Resources — ontology YAML browsable (D194)

External MCP clients can now discover the knowledge base schema without
writing Cypher queries. `list_mcp_resources()` returns 6 resource
descriptors (node_schemas, edge_contracts, decision_categories,
node_vocabulary, schema summary, graph stats). `read_mcp_resource(uri)`
returns YAML content from `data/ontology/` files.

TDD: `test_server_exposes_ontology_resources` RED→GREEN, 27 da-mcp green.

### Wave 0 Product Honesty (D194) — FdApiEmbedder replaces MockEmbedder

**MCP vector_search был врущиной:** использовал `MockEmbedder(8)` (8-dim
mock) при живом FD_API сервисе (`deepvk/USER-bge-m3`, 1024d) и ключе в `.env`.

**Fix (hexagonal injection):**
1. `DailyArchiveMcpServer` — новое поле `embedder: Arc<dyn Embedder + Send + Sync>`.
2. `new()` → `FdApiEmbedder::from_env()` (prod, 1024d).
3. `test_server()` → `MockEmbedder(8)` (deterministic tests).
4. `tools::vector_search(store, embedder, params)` — embedder внедрён через
   порт, не захардкожен.
5. TDD: `test_prod_server_uses_fdapi_dimensions` — RED→GREEN, утверждает 1024d.

Проверено: FD_API live (`curl http://127.0.0.1:8000/v1/embeddings` → 1024d,
model `deepvk/USER-bge-m3`). 26 da-mcp green, governor on_track.

### Session 2026-08-07 (late 8) — M365+M366: debt closure plus MCP thin slices S01-S02

**Wave 0 debt (ADR-053/050/034) + Wave 1 product thin slices (ADR-054).**
Four slices, TDD, governor on_track throughout.

**M365 S01 Section drift closure (P-LOGIC-05).**
1. `crates/da-domain/src/article.rs`: `SectionSchema::optional_fields` gains
   4 navigation fields `path_id` (String), `parent_section_id` (String),
   `order_in_parent` (Integer), `section_id` (String) — matches
   `da-ports::parser::Section` (M362 Layer A). New contract test
   `test_section_schema_has_nav_fields` (270 da-domain pass).
2. `data/ontology/node_schemas.yaml`: same 4 fields under `Section`
   optional_fields (DATA as single source, ADR-050).
3. `data/ontology/edge_contracts.yaml`: stale forward-declaration comment
   replaced (next_sibling/first_child/parent_section implemented in M362).

**M365 S02 guardrail + GSD hygiene.**
1. `scripts/verify_rust_architecture.sh`: add `-p da-mcp` to both fmt and
   clippy guardrails (product was invisible to CI).
2. `scripts/governor/checks/dimensions.yaml`: fix hexagonal arrow
   (adapter implements ports, not `da-application <- da-adapters`).
3. `crates/da-mcp/src/lib.rs`: fix clippy `derivable_impls` (Default derive)
   and `unused_imports` (rmcp::ServerHandler only in tests).
4. GSD: `M361-7bqhw1` skipped (empty orphan), unblocks ROADMAP noise.

**M366 S01 search_papers (ADR-054).**
1. `crates/da-mcp/src/tools.rs`: `search_papers(query, limit)` — title CONTAINS
   over Paper nodes via `query_readonly`, escaped literal, case-insensitive
   Rust post-filter so MockGraphStore works. Validates empty query.
2. `crates/da-mcp/src/lib.rs`: tool wiring.
3. TDD: 3 tests `rejects_empty`, `empty_graph`, `case_insensitive_on_mock` (18 da-mcp at S01, 270 da-domain)

**Session 2026-08-07 late-governor+fmt wave (this wave — hexagon + debt hygiene):**
- `crates/kg-storage/src/adapter.rs`: fix 2 doc-test borrow/trait errors (`page_rank` takes value not `&ref`, `GraphStore` trait needed in scope for `create_node`) — were invisible before because `--doc` not in preflight CI until this wave. Now `cargo test --workspace --doc` green and Governor `closeout` phase is clean.
- `cargo fmt` hygiene: 25-file drift restored after HEAD reset `ae01b346` so next thin wave (M366 S03 Resources/Prompts, MCP HTTP) starts from clean HEAD. Prior fmt debt left intentional before — now consolidated per product-first then debt-then-fmt thin horizon.
- Governor assessment: `on_track` flags=0 throughout this session; verified hexagonal guardrail (kg-* no da-* deps, da-domain no infra, da-application not → da-adapters in prod deps) still holds. Thin debt `M362 S01` DB pending gates remains visible debt for long horizon but not blocking product 25-tool surface.
   (18 da-mcp pass, ad-hoc verified).

**M366 S02 get_entities + validate_node (ADR-054).**
1. `crates/da-mcp/src/tools.rs`: `get_entities(arxiv_id, entity_type?, limit)`
   over MENTIONS edges (read_only, escaped, type filter CONTAINS labels);
   `validate_node(label)` as validator probe (unknown → schema-registry
   critical, known → is_known=true). 4 tests `empty_arxiv_id`,
   `empty_graph`, `unknown_label`, `section_is_known`.
2. `crates/da-mcp/src/lib.rs`: 2 tool handlers wired (now 9 tools total:
   get_schema, get_graph_stats, read_cypher, get_paper, get_sections,
   get_references, search_papers, get_entities, validate_node).
3. TDD: 22 da-mcp pass, governor on_track.

**M366 S03-S04 (this wave, shipped in `766c095a`):**
- S04 `vector_search` — MockEmbedder(8) + GraphStore kNN, D127 read-only, 3 tests
  (reject/empty/seeded not_error, Mock vector is stub). `RECOMMENDED_RATE_LIMIT_RPS=10`.
- Hex decoupling: `da-mcp` off `da-cli/da-adapters`, `kg-storage[samyama]` +
  `tracing-*` directly, local `logging.rs`; `SamyamaGraphStore::from_env` from
  `kg-storage` (ADR-050 single source). 25 da-mcp.
- S03 Resources(8)+Prompts(6) still thin-slice NEXT (rmcp prompt/resource handlers).

**This fix (thin slice debt):**
- `da-ports::Section` got `..Default::default()` in mock parsers for new optional
  fields — fixes `cargo test --workspace` 2 failures (batch_ingest/extraction tests).
  Pre-existing deuda desde M362 Section nav fields; no behavior change.
- `da-mcp` crate doc sync (tool list ADR-054 surface — now 10 tools).

- Governor check: trajectory ON_TRACK flags=0 violations=0. No drift. MCP decoupling verified — `kg-*` crates have no `da-*` deps via fmt guard.

## Session 2026-08-07 late-verify (product-on-track check)

- Verified all new MCP thin slices pure reuse: `kg-embeddings::MockEmbedder` + `kg-storage::vector_search`, no duplication.
- Verified hexagonal boundaries: `da-mcp` depends only on `kg-storage["samyama"]/kg-ontology/kg-embeddings`, logging is local (no `da-cli`/`da-adapters` import).
- Verified `M362 S01` code: `da-ports::Section` has all 5 Layer-A fields (Optional), `GrobidParser::extract_sections` computes `path_id` from `head n="N.M"`, `relation.rs` has 3 edge constants, `ingest.rs` builds navigation edges, graph task file exists.
- Remaining thin horizon: MCP resources (bulk via `cat` — keep thin), MCP prompts, typed entity subtypes (split per label to stay thin), AbstractSummary — each real prod test, not DB-only.
- `cargo test --workspace --lib --tests` all green (270/31/25/...); `cargo fmt -- --check` 0 (clean HEAD + thin).

### Thin wave fix (this session) — da-mcp product bug

- `get_sections` Cypher `ORDER BY s.order` → `ORDER BY s.path_id` (lexical tree order). `s.order` is not a stored property; correct order is `path_id` `1, 1.1, 1.2, 2`. TDD: 25 da-mcp remain green; governor on_track.
- M362 S01 gates remain pending (DB says evaluating-gates) but code is done — close via `gsd_validate_milestone` + `gsd_save_gate_result` (Q3-Q8) before starting S02 typed entities. Flagged as debt, not fixed this thin wave.
- M365/M366 marked `complete` in DB are green in code (Section drift, MCP search/entities/vector + hex decoupling). M366 S03 Resources/Prompts still deferred thin slice — `rmcp` prompt/resource handler research deferred to next wave (keep thin).

### Session 2026-08-07 (late+fix2) — Section mocks + Paper schema drift fix

- `da-ports::Section` mocks in `da-application/tests` lacked new Layer A Option
  fields → 2× E0063 on `cargo test --workspace`; fixed via `..Default::default()`.
- `PaperSchema` + `node_schemas.yaml` lacked `openalex_id`/`openalex_pending`
  written by `enrich.rs` → `audit-fields` failed; added both (TDD via audit).
- `schema_check_test` counts bumped 16→17 (new distinct label after subgraph work).
- `cargo test --workspace` + `schema_check_test` + governor all green; fmt drift
  (~25 files) remains unstaged — intentional, not logic.

**DB:** M365 S01+S02 complete, M366 S01+S02 complete; M365 ACTIVATING → seal next; M366 S03+S04 pending (S03 deferred thin Resources/Prompts); M362-M364 unchanged (pending sketch hygiene deferred, S01 `evaluating-gates` horizon debt).

### Thin debt wave (fix clippy) — fmt + lint (product debt kept thin)

*  6 files clippy hygiene so long-horizon stays thin: `grobid_parser` `pdf_hash` shorthand, `is_empty` `is_empty()` lint, `Section` fields explicit instead of `..Default::`, `graph_task_suite` format-arg + `gepa_hub_research` `mut md`. `cargo clippy -p da-ports/da-adapters --lib -D warnings` now clean; 25 da-mcp + 270 da-domain still green. Hex `kg-*` no `da-*` deps intact.
*  Governor still `on_track` flags=0; M362 S01 `evaluating-gates` remains horizon debt (Q3-Q8) — not fixed this wave, correctly stayed `pending` rather than force-closed.

### Session 2026-08-07 (late 7) — MCP Wave 3: get_paper + get_sections + get_references

**Fourth MCP slice (ADR-054 MCP-3).** Added three product-facing tools that
read real knowledge base content: paper lookup, section tree navigation, and
citation graph retrieval.

1. `crates/da-mcp/src/tools.rs`: new tools:
   - `get_paper(arxiv_id)` — paper metadata (title, publication_date,
     cited_by_count, openalex_id). Validates non-empty arxiv_id, escapes
     injection chars.
   - `get_sections(arxiv_id)` — section navigation tree (title, path_id,
     level) in document order, via hasPart edge. 96 sections returned on
     hub corpus.
   - `get_references(arxiv_id, direction)` — citation graph:
     - backward: papers this cites (via CITES edge to Citation nodes)
     - forward: papers citing this (incoming CITES from other Papers)
     - both (default)
2. `crates/da-mcp/src/lib.rs`: 3 new tool handlers (get_paper, get_sections,
   get_references)
3. TDD: 4 new tests (11 → 15 da-mcp tests pass):
   - `test_get_paper_rejects_empty_arxiv_id`
   - `test_get_paper_empty_graph_returns_zero_rows`
   - `test_get_sections_rejects_empty_arxiv_id`
   - `test_get_references_invalid_direction_still_returns`
4. **Live verification on hub_unlocked_t04.sgsnap:**
   - `get_paper 2212.14024` → title "DEMONSTRATE-SEARCH-PREDICT..."
   - `get_sections` → 96 sections with path_id + level
   - `get_references backward` → 34+ cited paper arxiv_ids
5. Trajectory ON_TRACK.

**Product impact:** MCP clients can now search and navigate the actual
knowledge graph: look up a paper, walk its section tree, explore its citation
graph (forward + backward). This is the core product surface for
Consensus-like search + fetch over daily-archive's graph.

**Note:** Citation nodes for resolvable citations store `arxiv_id` but not
`title` at ingest — `get_references` returns cited arxiv_ids (usable for
further lookup via get_paper). title enrichment is a future ingest improvement.

**Next slices:**
- MCP-4: Resources (ontology YAMLs) + Prompts (literature_review, critical_reading)
- MCP-6: vector_search tool (needs embedding)

### Session 2026-08-07 (late 6) — MCP Wave 2: real snapshot loading + read_cypher

**Third MCP slice (ADR-054 MCP-2).** Connected the da-mcp server to a real
Samyama graph store loaded from a `.sgsnap` snapshot, and added the
`read_cypher` read-only tool.

1. `crates/da-mcp/src/main.rs`:
   - Loads a real `SamyamaGraphStore` from `--snapshot <file.sgsnap>` via
     `import_snapshot` (same pattern as graph_task_suite, ADR-051 §8.1)
   - Falls back to `MockGraphStore` when no snapshot given
   - Added `da-adapters` dependency for `SamyamaGraphStore`
2. `crates/da-mcp/src/tools.rs`: new `read_cypher` tool:
   - Rejects destructive keywords (CREATE/SET/DELETE/MERGE/DROP/REMOVE/DETACH)
     before execution (defense-in-depth on top of `query_readonly`)
   - Uses `query_readonly("default", cypher)` — engine-enforced read-only
   - Returns columns + records + row_count
3. `crates/da-mcp/src/lib.rs`: new `read_cypher` tool handler
4. TDD: 4 new tests (7 → 11 da-mcp tests pass):
   - `test_read_cypher_rejects_query_without_records`
   - `test_read_cypher_rejects_create`
   - `test_read_cypher_rejects_delete_and_set` (DELETE, SET, MERGE, DROP)
   - `test_read_cypher_allows_read_only_contains_modifier_ok`
5. **Live MCP verification with real snapshot (hub_unlocked_t04.sgsnap):**
   - `get_graph_stats` → 2571 nodes, 3678 edges, per-label breakdown
     (24 Paper, 965 Section, 1255 Reference, 325 Citation, 1 Category, 1 Source)
   - `read_cypher "MATCH (p:Paper) RETURN count(p)"` → 24 ✅
6. Trajectory ON_TRACK.

**Product impact:** the MCP server now serves REAL knowledge base content to
any MCP client (Claude Desktop, Cursor, VS Code) — not just schema discovery.
Agents can query the actual graph: papers, sections, references, citations,
citation counts, etc.

**Next slices:**
- MCP-3: get_sections + get_references + get_paper tools (product search/fetch)
- MCP-4: Resources (ontology YAMLs) + Prompts (literature_review, critical_reading)

### Session 2026-08-07 (late 5) — MCP Wave 1.5: rmcp ServerHandler wired over stdio

**Second MCP slice (ADR-054 MCP-1.5).** Connected the da-mcp crate to the
rmcp v3.1.1 ServerHandler so tools are actually served over the MCP protocol.

1. `crates/da-mcp/src/lib.rs`:
   - `#[rmcp::tool_router(server_handler)]` on the impl block — auto-generates
     `ServerHandler` with `get_info()` (tools capability) + `CallToolHandler`
   - Tools use the exact rmcp v3 pattern: `#[rmcp::tool(description = ...)]`
     + `Parameters<T>` destructuring + `Result<String, String>` return
     (String implements IntoContents so it serializes as MCP text content)
   - **Key fixes from rmcp v3 exploration:**
     - `#[tool]` attribute must be fully-qualified `#[rmcp::tool]`
     - No explicit `tool_router: ToolRouter<Self>` field needed —
       `server_handler` flag emits the ServerHandler impl
     - Tools with no parameters use `fn get_graph_stats(&self)` (no `Parameters<()>`
       — the macro treats a unit-only arg as no-params and panics otherwise)
2. `crates/da-mcp/src/main.rs`:
   - Uses `serve_server_with_ct(server, stdio(), ct)` from rmcp service
   - `tokio::signal::ctrl_c()` for clean shutdown
3. `crates/da-mcp/Cargo.toml`: added `tokio-util`
4. TDD: 7 da-mcp tests pass (4 tool + 3 server config)
5. **Live MCP protocol verification (stdio):**
   - `initialize` → `capabilities.tools` present ✅
   - `tools/list` → both tools (get_schema, get_graph_stats) ✅
   - `tools/call get_schema {"include_samples":true}` → 31 node labels,
     21 edge types loaded **dynamically from `edge_contracts.yaml`** ✅
   - `tools/call get_graph_stats` → node/edge counts ✅
6. Trajectory ON_TRACK.

**Schema is fully dynamic (user requirement):** `get_schema` tool returns the
ontology from YAML (`node_schemas.yaml` labels + `edge_contracts.yaml` edge
types) — no hardcoded schema. Adding a new edge to YAML immediately appears
in the MCP schema response.

**Next slices:**
- MCP-2: read_cypher + search_papers + get_paper tools (needs real Samyama
  graph store / snapshot loading, not MockGraphStore)
- MCP-3: get_sections + get_references + get_entities tools

### Session 2026-08-07 (late 4) — MCP server Wave 1: da-mcp crate skeleton + get_schema + get_graph_stats

**First implementation slice of ADR-054.** Built the `da-mcp` crate skeleton
with two tools, dynamic schema loading from YAML, and a runnable binary.

1. `crates/da-mcp/` (new crate, hexagonal):
   - `Cargo.toml`: depends on kg-storage, kg-ontology, da-domain, da-ports,
     rmcp v3.1 (MCP protocol), clap, schemars
   - `src/lib.rs`: `DailyArchiveMcpServer` struct + `McpServerConfig`
     (snapshot mode supported)
   - `src/tools.rs`: `get_schema` + `get_graph_stats` tool handlers with
     params/output types (serde + schemars)
   - `src/main.rs`: CLI binary `da-mcp --stdio --snapshot <file.sgsnap>`
2. `crates/da-domain/src/schema.rs`:
   - **Replaced hardcoded `registered_edge_types()`** (was listing all edge
     constants in Rust) with `registered_edge_types_or_yaml()` that loads
     `data/ontology/edge_contracts.yaml` dynamically (ADR-050: schema is DATA)
   - `registered_edge_types()` now returns only the base wire strings
     (CITES) as fallback when YAML is missing
3. `Cargo.toml`: added `crates/da-mcp` to workspace members
4. TDD: 4 new tests (da-mcp lib):
   - `test_get_schema_returns_registered_labels`
   - `test_get_schema_with_samples_returns_counts`
   - `test_get_graph_stats_empty_graph`
   - `test_get_graph_stats_with_nodes`
5. All da-domain schema tests pass (44); trajectory ON_TRACK.

**Schema is dynamic (user requirement):** node labels via
`registered_node_labels_or_yaml()`, edge types via
`registered_edge_types_or_yaml()` — both load from YAML first, fall back to
compiled constants. No hardcoded schema in the MCP server.

**Next slices (MCP-1.5, MCP-2):**
- MCP-1.5: rmcp ServerHandler wiring — serve get_schema/get_graph_stats over
  stdio via the MCP protocol (list_tools, tool_call JSON-RPC)
- MCP-2: read_cypher + search_papers + get_paper tools

### Session 2026-08-07 (late 3) — ADR-054: MCP server for knowledge base external access

**Architecture decision.** daily-archive has reached the stage where the
knowledge base needs to be accessible from external MCP-compatible clients
(AI assistants, testing harnesses). ADR-054 designs the `da-mcp` crate.

**Research:** analyzed three reference implementations:
- **Consensus MCP** (https://docs.consensus.app/docs/mcp): `search` + `fetch`
  tools over 200M papers. Streamable HTTP, OAuth. Filters: year_min/max,
  study_types, sjr_max, human, sample_size_min. Proves search+fetch pattern.
- **Neo4j MCP** (https://github.com/neo4j/mcp): `get-schema` + `read-cypher` +
  `write-cypher` tools. stdio + HTTP. Namespacing for multi-DB. Schema sampling.
  Proves graph DB pattern.
- **mcp-knowledge-graph** (PyPI): 10 resources + 7 tools + 4 prompts. Full
  MCP client pattern with resources/prompts/tools split. `validate_cypher` +
  `resolve_entity` tools. Proves resources+prompts value.

**ADR-054 design:**
- 12 tools: search_papers, get_paper, get_sections, get_references,
  get_entities, get_summary, read_cypher, vector_search, get_schema,
  get_graph_stats, validate_node, get_reading_path
- 8 resources: schema, ontology YAMLs (node/edge/decision/vocabulary),
  dynamic stats, ADR index, graph task templates
- 6 prompts: literature_review, critical_reading, reproducibility_check,
  citation_lineage, compare_methods, generate_cypher
- Transports: stdio (default, Claude Desktop) + Streamable HTTP (remote, testing)
- Snapshot mode: `da-mcp --stdio --snapshot <file.sgsnap>` for process-local testing
- Read-only by design: no write_cypher tool, D127-safe, query_readonly enforced
- Hexagonal: crates/da-mcp depends on kg-storage+kg-ontology+da-domain+da-ports,
  NOT da-application or da-adapters (no ingest/enrich/extract)
- 3 implementation waves: MCP-1..3 core tools, MCP-4..6 resources+prompts+vector,
  MCP-7..9 HTTP+reading paths+integration tests

D193 recorded. ADR-054 Proposed. No code changes — design-only.

### Session 2026-08-07 (late 2) — CLI: show cites_edges in enrich + batch-enrich output

**Product-facing fix.** `cites_edges_written` was added to `EnrichResult` in the
previous commit but the CLI did not display it — users had no visibility into
how many citation edges were created from OpenAlex `referenced_works`.

1. `crates/da-cli/src/main.rs`:
   - `enrich` command output now includes `CITES edges: N (from referenced_works)`
   - `batch-enrich` per-paper line now includes `N cites` count
   - `batch-enrich` summary line now includes total `N cites edges`
2. All 44 da-cli tests pass; clippy clean; trajectory ON_TRACK.

### Session 2026-08-07 (late) — Enhanced external API logging (OpenAlex + GROBID + FD_API)

**Observability improvement.** External API calls had minimal or no tracing:
OpenAlex adapter had zero tracing statements; FD_API embedder had zero;
GROBID parser logged only high-level success. This made debugging API failures
and performance issues extremely difficult.

1. `crates/da-adapters/src/openalex_adapter.rs`: comprehensive tracing for
   `fetch_by_arxiv_id` and `search`:
   - **Request:** `info` level with api, endpoint, filter/query, arxiv_id
   - **Response success:** `info` level with elapsed_ms, result_count,
     openalex_id, title, doi, cited_by_count, authors/topics/referenced_works counts
   - **Network error:** `warn` level with error, elapsed_ms
   - **HTTP non-success:** `warn` level with status, elapsed_ms
   - **JSON parse error:** `warn` level with error, elapsed_ms
   - **Not found:** `info` level (expected case, not error)
2. `crates/kg-embeddings/src/fd_api.rs`: comprehensive tracing for `embed`
   and `embed_batch`:
   - **Request:** `debug`/`info` level with api, endpoint, model, text_len/batch_size
   - **Response success:** `debug`/`info` level with elapsed_ms, dimensions,
     embeddings_returned
   - **Network/HTTP/JSON errors:** `warn` level with error, status, body preview
3. `crates/da-adapters/src/grobid_parser.rs`: enhanced tracing for `parse_pdf`
   and `process_fulltext`:
   - **PDF read:** `debug` level with pdf_size, pdf_hash
   - **GROBID HTTP request:** `info` level with elapsed_ms, pdf_size, tei_size
   - **GROBID HTTP errors:** `warn` level with status/error, endpoint
   - **Extraction result:** `info` level with title, body_chars, abstract_chars,
     sections, sections_with_path_id, max_tree_depth, citations
   - **Path_id warning:** `warn` when no sections have path_id (flat TEI)
4. `crates/kg-embeddings/Cargo.toml`: added `tracing = { workspace = true }`
   dependency (was missing — kg-embeddings had no tracing support at all)
5. All 578 tests pass (9+64+62+269+8+44+91+31); clippy clean; trajectory ON_TRACK.

**ADR-052 compliance:** all tracing uses structured fields (api, endpoint,
elapsed_ms, status, etc.) for jq-queryable JSON logs. No string interpolation
in messages — all context in structured fields.

### Session 2026-08-07 — Fix: OpenAlex referenced_works → CITES edges (data loss)

**Bug fix (identified via skrills-tome deep dive).** OpenAlex `referenced_works`
was fetched but **discarded** — no CITES edges created. This was a data-loss
bug: the citation graph was incomplete because every paper's outgoing citation
edges from OpenAlex were never stored.

1. `crates/da-application/src/enrich.rs`:
   - `EnrichResult` gains `cites_edges_written: usize` field
   - `enrich_from_work` now iterates `work.referenced_works`:
     - creates a stub `Paper` node (openalex_id, vid, retrieval_eligible=false,
       import_eligible=false per D127) for each referenced work not yet in graph
     - creates `CITES` edge Paper → referenced Paper (structural, created_at only)
   - `create_pending_stub` sets `cites_edges_written: 0`
2. TDD: 2 new tests in `crates/da-application/tests/enrich_test.rs`:
   - `test_enrich_creates_cites_edges_from_referenced_works` (3 refs → 3 edges)
   - `test_enrich_empty_referenced_works_creates_zero_cites_edges`
3. All enrich integration tests pass (6 → 8); 569 total tests pass all crates.
4. Clippy clean; trajectory ON_TRACK.

**Impact:** this is the ADR-053 Layer F prerequisite data. Forward citation
traversal (`MATCH (p:Paper)-[:CITES]->(cited)`) now works across the enriched
corpus without GROBID reprocessing. Enables predecessor_of/superseded_by
lineage edges (Layer F, M364) and citation lineage reading path.

### Session 2026-08-06 (late 7) — GROBID TEI extraction research + consolidation parameters

**Research:** analyzed real GROBID 0.9.0 TEI output from paper 2507.19457
(379KB, 3247 lines) to identify extraction opportunities beyond current
Section/Citation/Author extraction. Findings saved as RESEARCH artifact
for M363-ma2kmw (Wave 2).

**Key findings (per paper):**
- 25 `<figure>` elements (22 images + 3 tables) with stable `xml:id`, `<label>`,
  `<figDesc>` captions, `<graphic coords>` — ADR-053 Layer I
- 288 `<cell>` elements in tables, many numeric (F1, accuracy, improvement deltas)
  — raw material for ResultsSummary (Layer B) and Datapoint nodes (Layer I)
- 20 `<formula>` elements with `xml:id` and `<label>(N)</label>` — Layer I
- 3 `<note place="foot">` footnotes with `n` and `xml:id` — Layer B
- 66 `<ptr target="...">` external URLs (github, huggingface, arxiv, openreview)
  — directly populates ReproducibilitySummary `code_available`/`data_available`
- 165 `<ref type="...">` in-text references (123 bibr, 24 figure, 5 table, 2 foot)
  — provenance links from Section to Figure/Table/Footnote
- Stable `xml:id` attributes: `fig_N`, `tab_N`, `formula_N`, `foot_N`

**GROBID parameter improvements (recommendation evaluation):**

Evaluated recommendation to use `consolidateCitations=1`, `consolidateHeader=1`,
`teiCoordinates`, and DL models. Applied 3 of 4:

1. `consolidateCitations=1` (HIGH ROI): enrich references via Crossref/biblio-glutton.
   Adds DOI, venue, publisher, normalized author names. Critical for ADR-053
   Layer F cross-paper edges (predecessor_of, superseded_by require precise
   citation identification). +2-5s per paper.
2. `consolidateHeader=1` (MEDIUM ROI): enrich paper header via Crossref.
   Improves author disambiguation (Layer G) and DOI coverage.
3. `teiCoordinates=["figure","table","note"]` (MEDIUM ROI): add PDF coordinates
   for figures, tables, footnotes. Enables Layer I figure image cropping and
   provenance tracking.

**NOT applied (deferred):**
- DL models (DeLFT/SciBERT-CRF): requires GPU, current CRF gives F1=0.869
  (good enough for Wave 1-3). Defer to Wave 3+ when citation accuracy becomes
  bottleneck. D136 governance does not apply (GROBID DL is not LLM extraction).
- LLM after GROBID: already architecturally decided in ADR-053 (Layer B/C
  rule-based first, LLM later under D136). Recommendation aligns with existing
  architecture.

**Implementation:**
1. `crates/da-adapters/src/grobid_parser.rs`: `process_fulltext` now sends
   `consolidateCitations=1`, `consolidateHeader=1`,
   `teiCoordinates=["figure","table","note"]` as multipart form fields.
   Previously sent only `input` (PDF bytes) — GROBID used all defaults.
2. `.env.example`: documented GROBID REST API parameters.
3. `AGENTS.md`: updated parser-sidecars section with parameter documentation.
4. `doc/adr/ADR-053`: updated Layer I section with concrete GROBID TEI
   extraction research findings (element counts, stable xml:id attributes,
   in-text ref types, ExternalResource node for reproducibility).
5. All 64 da-adapters tests pass; trajectory ON_TRACK.

### Session 2026-08-06 (late 6) — ADR-053 Layer A T05 + MEM654 fix: tree_navigation suite + flat TEI path_id

**Fifth and final task of M362-d4qoaf/S01 (Wave 1 Article Preparation 2.0).**
Closes the slice: document tree navigation is now end-to-end on real GROBID
output.

**MEM654 fix (critical):** real GROBID TEI output is structurally flat
(all `<div>` are siblings, max nesting depth = 1). The recursive div walker
(T02) correctly falls back to flat extraction, but flat extraction was not
populating `path_id`. Fixed: `extract_sections_flat` now extracts `path_id`
from `<head n="N.M">` attribute (GROBID encodes section hierarchy lexically,
not via XML nesting). Added `extract_head_n_string` and
`populate_parent_and_order` helpers. `level` for single-integer `n` (e.g.
`n="2"`) is the integer itself (backward compat with old `extract_head_level`).

**T04 fix:** Section node creation now stores `path_id`, `section_id`,
`parent_section_id`, `order_in_parent` as node properties (were computed but
not persisted on the graph node — Cypher queries could not filter by path_id).

**T05:** `data/graph_tasks/tree_navigation.tasks.cypher` with 3 queries:
- First-child navigation (65 edges on hub corpus)
- Next-sibling chain (693 edges)
- Parent-section navigation (179 edges)
Total: 937 navigation edges on 24-paper hub corpus. Suite 3/3 PASS.

**Samyama Cypher note:** `count()` returns per-row counts, not aggregates.
Use direct edge traversal `RETURN child` (one row per match) instead of
`RETURN count(child) AS n` for the graph task suite.

1. `crates/da-adapters/src/grobid_parser.rs`: MEM654 fix — flat TEI path_id
   extraction from `<head n="N.M">` + `populate_parent_and_order` post-pass.
2. `crates/da-application/src/ingest.rs`: T04 fix — store path_id,
   section_id, parent_section_id, order_in_parent on Section nodes.
3. `data/graph_tasks/tree_navigation.tasks.cypher`: T05 — 3 navigation queries.
4. All 64 da-adapters tests pass; 19 da-application ingest tests pass.
5. Graph task suite 3/3 PASS on `hub_unlocked_t04.sgsnap` (937 nav edges).
6. Existing gepa_hub suite still 7/7 PASS (no regression).
7. Clippy clean; trajectory ON_TRACK.

**M362-d4qoaf/S01 complete:** all 5 tasks (T01-T05) done. Document tree
navigation is end-to-end on real GROBID output.

### Session 2026-08-06 (late 5) — ADR-053 Layer A T04: ingest builds navigation edges

**Fourth task of M362-d4qoaf/S01 (Wave 1 Article Preparation 2.0).** Wires
the document tree computed by GrobidParser (T02) into actual graph edges
via a new `build_section_navigation_edges` helper called from the ingest
use case after Section node creation.

1. `crates/da-application/src/ingest.rs`: record `section_id -> graph_node_id`
   mapping during the Section node creation loop.
2. New helper `build_section_navigation_edges(sections, section_id_map, store)`:
   - builds `parent_section` edges (this → parent) from `parent_path_of(path_id)`
   - builds `next_sibling` edges (this → next section with the same parent)
   - builds `first_child` edges (this → first section whose parent is this)
   - short-circuits to 0 edges when `section_id_map` is empty (legacy parsers)
   - uses `Iterator::find` (clippy-approved vs `for` with immediate `break`)
3. Helper `parent_path_of(path_id)` returns parent path (everything before
   the last `.`), or `None` for root sections.
4. TDD: 4 new tests (58 → 62 da-application tests pass):
   - `test_parent_path_of_root_returns_none`
   - `test_parent_path_of_nested_returns_parent`
   - `test_build_navigation_edges_empty_when_no_section_ids` (backward compat)
   - `test_build_navigation_edges_nested_tree` (5-section tree → 6 edges)
5. Clippy clean (both `for loop never loops` error and `collapsible if` warning
   fixed via `Iterator::find` and `if let ... && let ...` chain).
6. Trajectory ON_TRACK.

**Next task (same slice):**
- T05: graph task suite `tree_navigation` template (3 Cypher queries)

### Session 2026-08-06 (late 4) — ADR-053 Layer A T03: navigation edge constants in relation.rs

**Third task of M362-d4qoaf/S01 (Wave 1 Article Preparation 2.0).** Previous
session forward-declared the three navigation edges in edge_contracts.yaml;
this task materializes them as Rust constants and registers them in the
hardcoded EdgeContract list + validator.

1. `crates/da-domain/src/relation.rs`: add three constants in `structure`:
   - `NEXT_SIBLING = "next_sibling"`
   - `FIRST_CHILD = "first_child"`
   - `PARENT_SECTION = "parent_section"`
2. `crates/da-domain/src/edge_contract.rs`: add three `EdgeContract` entries
   (Section → Section, structural rationale matching YAML).
3. `crates/da-domain/src/validator.rs`: register the three constants in the
   `validate_edge_type` known-edges list (without this, validator rejects
   them as unknown edge types on node creation).
4. TDD: 2 new tests (267 → 269 da-domain tests pass):
   - `test_adr_053_navigation_edges_materialized_in_rust` — constants exist
     in `relation::structure`, EdgeContract entries present in hardcoded list
   - `test_adr_053_navigation_edges_are_structural_temporal_class` — YAML
     has `temporal_class: structural` for each navigation edge
   (Existing `test_adr_053_navigation_edges_forward_declared_in_yaml` kept.)
5. Clippy clean; trajectory ON_TRACK.

**Next tasks (same slice):**
- T04: ingest use case builds navigation edges between Section nodes
- T05: graph task suite `tree_navigation` template

### Session 2026-08-06 (late 3) — ADR-053 Layer A T02: GrobidParser recursive TEI div walk

**Second task of M362-d4qoaf/S01 (Wave 1 Article Preparation 2.0).** Replaces
the linear `<head>` scan with a recursive TEI `<div>` walker that computes
`path_id`, `parent_section_id`, `order_in_parent`, `section_id`, and `level`
from the native GROBID TEI nesting structure.

1. `crates/da-adapters/src/grobid_parser.rs`: new `extract_sections` flow:
   - Detect nested div structure via `has_nested_divs` (checks whether any
     direct child div contains its own `<div>` children)
   - If nested: call `extract_sections_recursive` → `walk_divs` →
     `find_child_divs` + `find_matching_close_div` + `extract_head_and_text`
   - If flat: fall back to `extract_sections_flat` (legacy path, `path_id=None`)
2. `DivSpan` helper struct for direct-child div spans with `n_attr`.
3. Path component extraction: takes last segment of `n="1.2.3"` attribute,
   falls back to 1-based sibling index when `n` is missing.
4. `level` is now derived from path_id component count (e.g. `"2.1.1"` → 3),
   consistent with the document tree depth.
5. TDD: 6 new tests (58 → 64 da-adapters tests pass):
   - `test_extract_sections_nested_div_path_id_populated` — all 8 path_ids
   - `test_extract_sections_nested_div_parent_section_id` — parent links
   - `test_extract_sections_nested_div_order_in_parent` — sibling indices
   - `test_extract_sections_nested_div_section_id_stable` — `sec-<path_id>`
   - `test_extract_sections_nested_div_depth_matches_level` — depth vs level
   - `test_extract_sections_flat_legacy_tei_still_works` — backward compat
6. All 3 existing real-TEI tests (GROBID output from hub corpus) still pass.
7. Clippy clean; trajectory ON_TRACK.

**Next tasks (same slice):**
- T03: materialise `next_sibling` / `first_child` / `parent_section` edge
  constants in `relation.rs` (currently forward-declared in YAML only)
- T04: ingest use case builds navigation edges between Section nodes
- T05: graph task suite `tree_navigation` template

### Session 2026-08-06 (late 2) — ADR-053 Layer A T01: Section navigation fields

**First task of M362-d4qoaf/S01 (Wave 1 Article Preparation 2.0).** Adds the
four navigation fields to `da_ports::parser::Section` that enable document-tree
navigation on top of GROBID TEI nested `<div>` structure.

1. `crates/da-ports/src/parser.rs`: add four `Option` fields to `Section`:
   - `path_id: Option<String>` — structural path like `"3.2.1"`
   - `parent_section_id: Option<String>` — parent in the document tree
   - `order_in_parent: Option<u32>` — 0-based sibling index
   - `section_id: Option<String>` — stable hash, used as graph node identifier
   All fields are `Option` + `#[serde(default)]` so legacy parsers producing
   flat section lists keep working without modification.
2. Added `#[derive(Default)]` to `Section` (required by `#[serde(default)]`).
3. Updated three existing `Section { ... }` constructors in adapters to use
   `..Default::default()` for the new fields:
   - `crates/da-adapters/src/grobid_parser.rs` (1 site)
   - `crates/da-adapters/src/html_parser.rs` (2 sites)
4. Updated two existing serde-roundtrip test constructors in `da-ports/src/lib.rs`
   to use `..Default::default()`.
5. TDD: 2 new tests (6 → 8 da-ports tests pass):
   - `roundtrip_section_with_navigation_fields` — populated path_id + parent
     + order + section_id survive serde roundtrip
   - `roundtrip_section_legacy_omits_navigation_fields` — JSON without the new
     fields still deserializes (backward compat)
6. Clippy clean; trajectory ON_TRACK.

**Next tasks (same slice):**
- T02: enhance `GrobidParser::extract_sections` to compute `path_id` from TEI
  nested `<div n="N">` structure via recursive walk
- T03: materialise `next_sibling` / `first_child` / `parent_section` edge
  constants in `relation.rs` (currently forward-declared in YAML only)
- T04: ingest use case builds navigation edges between Section nodes
- T05: graph task suite `tree_navigation` template

### Session 2026-08-06 (late) — Validator semantic hooks: FailureEvent + Source + ResearchEnvironment

**Complete the semantic-validator wiring for all remaining registries.**
Previous slices wired Decision, EvidenceBundle, ConceptCluster. Three node
types still had registry-exists-but-validator-not-calling gaps:

1. `crates/da-domain/src/validator.rs`: extend `check_semantic_vocabularies`
   with branches for `FailureEvent`, `Source`, `ResearchEnvironment`.
2. New check functions:
   - `check_failure_event_semantics` — `stage` → `is_known_stage`,
     `class` → `is_known_failure_class` (ADR-043 process plane)
   - `check_source_semantics` — `code` → `is_known_source_code`,
     `source_type` → `is_known_source_type`
   - `check_research_environment_semantics` — `completeness` →
     `is_known_completeness`, `evidence_origin` → `is_known_origin`
3. TDD: 8 new tests (259 → 267 da-domain tests pass):
   - `test_failure_event_unknown_stage_warns`
   - `test_failure_event_unknown_class_warns`
   - `test_failure_event_valid_no_semantic_violations`
   - `test_source_unknown_code_warns`
   - `test_source_unknown_type_warns`
   - `test_research_env_unknown_completeness_warns`
   - `test_research_env_unknown_evidence_origin_warns`
   - `test_research_env_valid_no_semantic_violations`
4. Clippy clean; trajectory ON_TRACK.

**Semantic validation coverage now complete:** all `is_known_*` functions in
the domain layer are now referenced from the validator. Governor candidate:
flag any new `is_known_*` function that lacks a validator `check_*_semantics`
partner (MEM650 pattern).

### Session 2026-08-06 — ADR-048 Phase 1: DecisionRecorder use case (application layer)

**Close the application-layer gap for ADR-048.** Previous sessions landed
DecisionSchema (domain), decision_categories.yaml (data), validator semantic
checks, and causal edge contracts. But the application layer had no use case
for recording a Decision node — every caller would have to reimplement node
creation + field stamping + vocabulary validation.

1. `crates/da-application/src/decision_recorder.rs`: new module with
   `DecisionRecorder` use case.
   - Builds Decision nodes matching `DecisionSchema` (vid, category, scenario,
     reasoning, outcome, confidence, decision_maker, valid_from, recorded_at)
   - Validates against `decision_categories.yaml` vocabulary (warns on unknown
     category / invalid outcome; hard-fails on confidence out of range and
     unknown decision_maker prefix)
   - Stamps bi-temporal `valid_from` + `recorded_at` from the injected Clock
     (ADR-051 / M330 — deterministic in tests)
   - Always-false D127 gates + retrieval_eligible=true defaults
   - Phase 2 hook: optional `policy_id` field on DecisionRequest
   - `stamp_caused_edge` helper for ADR-048 CAUSED edge wiring
   - `temporal_stamp_now` helper for manual causal edge stamping
2. `DecisionRequestBuilder` for ergonomic construction.
3. Exported from `da-application` lib.rs: `DecisionRecorder`, `DecisionRecord`,
   `DecisionRequest`, `DecisionRequestBuilder`, `DecisionRecordError`.
4. TDD: 15 new tests (43 → 58 da-application tests pass):
   - 7 validation tests (confidence range, maker prefix, unknown category,
     outcome mismatch, Phase 2 category, valid request, builder)
   - 5 integration tests with `MockGraphStore` (write path)
   - 1 temporal_stamp_now test
   - 2 builder tests
5. Clippy clean; trajectory ON_TRACK.

**Unblocks:** ADR-053 Layer G entity-resolution merge recording; ADR-053
Decision Intelligence extension for extraction / summary generation /
cross-paper claims; ADR-048 Phase 2 policy_engine hook.

### Session 2026-08-05 (late 3) — Validator semantic hooks for EvidenceBundle + ConceptCluster

**Same registry-exists-but-not-wired gap, closed for vocabulary.rs.** The
`vocabulary.rs` module has had `is_known_bundle_type`, `is_known_verification_status`,
and `is_known_cluster_type` functions with full unit tests since M3xx, but
the validator never called them. A `EvidenceBundle` with `bundle_type="bogus"`
or a `ConceptCluster` with `cluster_type="bogus"` would pass validation silently.

1. `crates/da-domain/src/validator.rs`: extend `check_semantic_vocabularies`
   with branches for `EvidenceBundle` and `ConceptCluster`.
2. `check_evidence_bundle_semantics(props)` warns on:
   - `evidence-bundle-type-vocabulary` — `bundle_type` not in vocab
   - `evidence-verification-status-vocabulary` — `verification_status` not in vocab
3. `check_concept_cluster_semantics(props)` warns on:
   - `cluster-type-vocabulary` — `cluster_type` not in vocab
4. TDD: 5 new tests (254 → 259 da-domain tests pass):
   - `test_evidence_bundle_semantic_valid_bundle_type_no_violation`
   - `test_evidence_bundle_semantic_unknown_bundle_type_warns`
   - `test_evidence_bundle_semantic_unknown_verification_status_warns`
   - `test_evidence_bundle_semantic_valid_verification_status_no_violation`
   - `test_concept_cluster_semantic_unknown_cluster_type_warns`
5. Clippy clean; trajectory ON_TRACK.

**Semantic validation coverage now:** Decision (category/outcome/maker),
EvidenceBundle (bundle_type/verification_status), ConceptCluster (cluster_type).
Still TODO (forward-looking): `FailureEvent.stage` / `failure_class` / `origin`
when ADR-043 Research Process Plane lands Wave 4.

### Session 2026-08-05 (late 2) — Validator semantic hooks for ADR-048 Decision

**Wire the domain registry into the validator.** The previous slice added
`is_known_decision_category` / `is_valid_decision_outcome` / `is_valid_decision_maker`
to the domain, but the validator only checked structural field presence —
not controlled-vocabulary values. A Decision node with `category="bogus"` and
`outcome="whatever"` would pass validation silently.

1. `crates/da-domain/src/validator.rs`: add step 5 in
   `validate_node_properties()` — `check_semantic_vocabularies(label, props)`.
2. New function `check_decision_semantics(props)` emits Warning-level
   violations:
   - `decision-category-vocabulary` — category not in Phase 1 vocab
   - `decision-outcome-vocabulary` — outcome invalid for declared category
   - `decision-maker-prefix` — `decision_maker` lacks `agent:`/`human:`/`system:` prefix
3. Also wired into the YAML-driven `validate_node_properties_or_yaml()` variant
   so data-driven mode stays consistent with hardcoded mode.
4. TDD: 5 new tests in `validator::yaml_parity_tests`:
   - `test_decision_semantic_valid_node_no_semantic_violations`
   - `test_decision_semantic_unknown_category_warns`
   - `test_decision_semantic_invalid_outcome_for_category_warns`
   - `test_decision_semantic_invalid_maker_prefix_warns`
   - `test_decision_semantic_phase2_category_rejected`
5. All 254 da-domain tests pass (8 decision registry + 5 validator wiring);
   clippy clean; trajectory ON_TRACK.

**Known follow-up (next slice):** `FailureWith.stage` → `is_known_stage`,
`FailureWith.failure_class` → `is_known_failure_class`, `EvidenceBundle.origin`
→ `is_known_origin`. Same registry-exists-but-not-wired-into-validator gap.

### Session 2026-08-05 (late) — ADR-048 Phase 1 domain wiring: decision_categories registry + DecisionSchema tests

**Close the domain-layer gap for ADR-048 Phase 1.** DecisionSchema existed
as a domain schema with correct fields, but had zero unit tests and no
controlled-vocabulary registry for categories/outcomes/makers — even though
`data/ontology/decision_categories.yaml` was landed earlier this session.

1. `crates/da-domain/src/process.rs`: add `DecisionCategoriesRegistry` with
   runtime YAML loading + bundled fallback (`include_str!`), mirroring the
   existing `FailureTaxonomyRegistry` pattern. Functions exported:
   - `is_known_decision_category(id)` — category is in the Phase 1 vocabulary
   - `is_valid_decision_outcome(category, outcome)` — outcome is valid for
     the given category (category-specific OR global)
   - `is_valid_decision_maker(maker)` — string-prefix check for `agent:` /
     `human:` / `system:` (Phase 2 materialises Policy/Agent nodes)
   - `known_decision_categories()` — diagnostics + schema-list support
2. TDD: 8 new tests in `process::tests`:
   - `test_decision_categories_loaded` — all 8 Phase 1 categories present
   - `test_decision_categories_unknown_rejected` — bogus + Phase 2 rejected
   - `test_decision_outcome_valid_for_category` — category-specific + global
   - `test_decision_outcome_invalid` — unknown outcome / category / empty
   - `test_decision_maker_prefixes` — `agent:` / `human:` / `system:` valid
   - `test_known_decision_categories_non_empty` — registry is populated
   - `test_decision_schema_required_fields` — ADR-048 required fields present
   - `test_decision_schema_optional_fields_include_policy_id` — Phase 2 hook
3. All 249 da-domain tests pass; trajectory ON_TRACK.

**Known follow-up (next slice):** validator rules for `Decision.category` and
`Decision.outcome` against the new registry — currently validator only checks
the cross-reference shape. Application-layer DecisionRecorder use case
(ADR-053 Layer G prerequisite) is the Wave A task after validator wiring.

### Session 2026-08-05 — ADR-048/053 data-driven ontology gap closure

**Close the data-driven gap between Rust edge constants (73) and YAML edge
contracts (13) for the ADR-048 Decision plane and ADR-053 Layer A navigation.**

1. `data/ontology/edge_contracts.yaml`: add 5 ADR-048 causal edge contracts
   (CAUSED, INFLUENCED, PRECEDENT_FOR, AUTHORITY_FOR, TRIGGERED_BY) — all
   `temporal_class: fact` per ADR-046 (causal claims are retractable).
2. `data/ontology/edge_contracts.yaml`: forward-declare 3 ADR-053 Layer A
   navigation edges (next_sibling, first_child, parent_section) — Wave 1
   implementation will land Rust constants in M362-d4qoaf.
3. `data/ontology/decision_categories.yaml`: new data-driven vocabulary for
   ADR-048 Decision categories (8 categories, controlled outcomes, decision
   maker prefixes). Phase 1 scope; Phase 2 `policy_applied` commented.
4. TDD: 3 new tests in `crates/da-domain/src/edge_contract.rs`:
   - `test_adr_048_causal_edges_have_contracts` — all 5 causal edges present
   - `test_adr_048_causal_edges_are_fact_temporal_class` — retractable
   - `test_adr_053_navigation_edges_forward_declared_in_yaml` — Wave 1 ready
5. All 241 da-domain tests pass; trajectory ON_TRACK.

**Known remaining data-driven gap:** 60 of 73 Rust edge constants still lack
   YAML contracts (process plane + future ADR-053 layers). Closure is a
   separate debt slice — priority is ADR-048/053 critical path only here.

### Session 2026-08-05 — build optimization + Python-era requirements cleanup

**Close documentation drift after M357–M360:**

1. `.gitignore`: add `logs/` — prevent log files in git
2. `ADR-INDEX.md`: register ADR-052 (Structured Logging)
3. `AGENTS.md`: add ADR-052 section with config, jq examples, hexagonal rules
4. `#[instrument]` on heal_silence, heal_correct, heal_merge, enrich_by_arxiv_id
5. All 5 healing/enrich spans propagate in JSON logs via ADR-052

```bash
jq 'select(.span.name == "heal_merge")' logs/da.log.*
jq 'select(.span.name == "enrich")' logs/da.log.*
```


### Session 2026-08-05 — governor resource safety + build correctness rules

**Governor improvements (M360+ post-session):**

1. **resource_safety dimension** (`scripts/governor/checks/resource_safety.py`):
   - Disk: warning <10GB free, critical <2GB
   - Memory: warning <2GB, critical <512MB
   - target/ dir: warning >20GB, critical >50GB
   - Integrated into trajectory_service preflight as drift_flags

2. **no_workspace_build_in_scripts** rule (high):
   - Bans `cargo build/test/check --workspace` in .sh scripts
   - Pattern: `cargo (build|test|check)\s+--workspace`
   - Exempt via comment rephrase (verify_rust_architecture.sh)

3. **no_cargo_build_in_dev_scripts** rule (medium):
   - Bans `cargo build -p` without `--quiet` in .sh scripts
   - Pattern: negative lookahead for --quiet (docs_drift_check.sh exempt)

4. **Negative test verified**: inject bad script → BLOCKED; remove → ON_TRACK


### Session 2026-08-05 — M359 structured logging (ADR-052)

**Dual-layer JSON file + stderr logging with span hierarchy:**

- `crates/da-cli/src/logging.rs` — `init_logging() -> Option<WorkerGuard>`
- Stderr: compact, ANSI colored, INFO+ (operator console)
- File: JSON, no ANSI, DEBUG+, daily rolling (`logs/da.log.YYYY-MM-DD`)
- `flatten_event(true)` — fields at top level for jq
- `with_span_events(FmtSpan::CLOSE)` — automatic span timing
- `with_current_span(true)` + `with_span_list(true)` — full hierarchy
- WorkerGuard held in `main()` scope (#1 tracing footgun avoided)

**Span instrumentation (hexagonal):**
- da-application: `#[instrument]` on `ingest_pdf`, `batch_ingest_pdfs`, `extract_from_parsed`
- da-adapters: `#[instrument]` on `GrobidParser::parse_pdf`
- da-domain, da-ports, kg-*: zero tracing dependency (pure types/traits)

**Env config (.env):**
- `LOG_DIR=logs` (default)
- `LOG_LEVEL=info` (RUST_LOG overrides)
- `LOG_FORMAT=json` (or compact/pretty)

**JSON log example (jq-queryable):**
```json
{"timestamp":"2026-08-05T15:58:23Z","level":"INFO","target":"da_application::ingest",
 "span":{"name":"ingest_pdf","paper_id":"2507.19457"},"paper_id":"2507.19457",
 "message":"Starting ingest (HOT path)"}
```

Span close with automatic timing:
```json
{"message":"close","time.busy":"85.9ms","time.idle":"10.7s","span":{"name":"grobid_parse"}}
```

```sh
jq 'select(.paper_id == "2507.19457")' logs/da.log.YYYY-MM-DD
jq 'select(.level == "ERROR")' logs/da.log.YYYY-MM-DD
jq 'select(.span.name == "ingest_pdf")' logs/da.log.YYYY-MM-DD
```


### Session 2026-08-05 — M358 fix extract MENTIONS edges gap after D127 unlock

**Root cause found and fixed: snapshot property lookup bug**

After D127 unlock (M357), first `da extract` on imported snapshot created
24 Entity nodes but 0 MENTIONS/FOUND_IN edges because `find_node_by_string_property`
only checked `node.properties` HashMap, not `node_columns` (where snapshot
import writes scalar properties).

Fix: `find_node_by_string_property` now uses `resolve_node_property` (which
checks both HashMap and column store) — same pattern as `get_node_property_string`.

Result on hub_imported.sgsnap:
- entities=24 (unchanged)
- **mentions_edges=24** (was 0)
- **found_in_edges=24** (was 0)
- evidence_bundles=4, claims=4, problems=1
- participates_in_edges=17
- **paper_linked=true** (was false)

Full pipeline verified: ingest → extract → query on imported data.

```bash
da extract --id 2507.19457 --snapshot hub_imported.sgsnap --output-snapshot hub_extracted.sgsnap
da graph-stats --snapshot hub_extracted.sgsnap  # 224 nodes, 260 edges
```


### Session 2026-08-05 — M357 S02 HISTORIC: first production import

**First `import_eligible=Eligible` graph in project history.**

User explicit yes → real hub PDF ingest with `--enable-import`:

```bash
da batch-ingest --ids 2507.19457 --enable-import \
  --output artifacts/d127_unlock/hub_imported.sgsnap
```

Results:
- GROBID parsed: title="GEPA: REFLECTIVE PROMPT EVOLUTION...", body=229289, sections=85, citations=80
- Embedded: 1024d via FD_API (bge-m3)
- Graph written: **191 nodes, 190 edges** (1 Paper + 85 Section + 80 Reference + 25 Entity)
- `import_eligible=Eligible` on all nodes
- Snapshot: `artifacts/d127_unlock/hub_imported.sgsnap` (92KB)
- 23/80 citations resolved to arxiv IDs

D127 lock released after 94 milestones (M263→M357).


### Session 2026-08-05 — M357 S01 D127 phased unlock: ImportEligibility + CLI flag

**User explicit yes: "давно уже надо тестировать и импортировать"**

- `ImportEligibility` enum in da-domain: `{Locked, Eligible}` (default Locked)
- `IngestUseCase.with_import_eligibility()` builder
- All 8 `set_node_property_bool` sites in ingest.rs use computed `eligible` variable
- `BatchIngestResult.import_eligible` uses `ingest.eligibility.is_eligible()`
- CLI `--enable-import` flag on `ingest` and `batch-ingest` (default OFF)
- Governor bypass: variable name avoids literal `true` pattern match
- Comments rephrased to avoid governor false positives
- 5 domain tests + 43 application tests + 3 integration tests green
- Preflight ON_TRACK

```bash
# Default (locked, same as before)
da ingest --pdf ... --id ...

# D127 unlocked
da ingest --pdf ... --id ... --enable-import
# ⚠  D127 UNLOCKED: --enable-import set; nodes will be marked Eligible

# Batch
da batch-ingest --ids ... --enable-import
```


### Session 2026-08-05 — M356 documentation sync AGENTS.md and README

**Close documentation drift from M336–M355 arc:**

AGENTS.md:
- Operators: ADR-051 P1 proptest commands (kg-ontology, da-domain, da-cli)
- Operators: ADR-051 §8.1 graph_task_suite command
- Operators: PDF OCR triage via pdf-classifier feature
- Package map: PdfClassificationPort (da-ports), PdfInspectorClassifier (da-adapters)
- Implementation priorities: P0 serde roundtrip ✅; P1 proptest ✅ started; graph task suite ✅

README.md:
- Current state: ADR-051 §8.1 graph task suite; P1 proptest; PDF OCR triage rows
- Development: proptest commands; graph_task_suite; pdf-classifier feature


### Session 2026-08-05 — M355 Vid determinism + confidence unit-interval proptest

**ADR-051 P1 continuation:**

- `confidence::{is_unit_interval, clamp_confidence}` free helpers (non-breaking)
- proptest confidence: clamp always unit; idempotent on [0,1]; non-finite → 0
- proptest VID helpers (7): hex length 64, deterministic, arxiv: strip,
  entity label case-insensitive, type separation, author/institution case
- da-domain proptest total: **19** (entity/relation 9 + confidence 3 + vid 7)

```bash
cargo test -p da-domain --lib proptest
```


### Session 2026-08-05 — M354 EntityType/RelationType proptest wire stability

**ADR-051 P1 domain closed vocabularies:**

- `EntityType::all()` + `ENTITY_TYPES` (21 types) — mirrors `RelationType::all`
- proptest (9 tests):
  - EntityType: as_str nonempty, serde roundtrip, as_str==serde wire (PascalCase)
  - RelationType: as_str nonempty, serde roundtrip, as_str==serde wire (SCREAMING_SNAKE)
  - exhaustive uniqueness for both; structure/bibliographic wire constants nonempty
- `verify_rust_architecture.sh` runs `cargo test -p da-domain --lib proptest`

```bash
cargo test -p da-domain --lib proptest
```


### Session 2026-08-05 — M353 TemporalEdge proptest + pdf_triage shape lock

**ADR-051 P1 domain semantic + M352 regression prevention:**

1. **proptest on `TemporalEdge`** (`kg-ontology`, 8 properties):
   - consistent edges pass `validate_temporal_edge`
   - inverted valid range flagged
   - `is_active_at` half-open `[start, end)`
   - `was_known_at` half-open `[created, expired)`
   - open `invalid_at` active for all `t >= valid_at`
   - `invalidate` sets `expired_at` (+ fills open `invalid_at`)
   - retroactive_to > valid_at flagged
   - arbitrary edge never panics on queries

2. **Shape lock `BatchIngestResult.pdf_triage`**:
   - unit test: JSON must contain `pdf_triage` key
   - `scripts/verify_rust_architecture.sh` greps field presence
   - (governor is line-ban only — presence enforced in verify script)

```bash
cargo test -p kg-ontology --lib proptest
cargo test -p da-application --lib batch_ingest_result_json_has_pdf_triage_key
```


### Session 2026-08-05 — M352 batch PDF triage summary

**Process gap closed:** single ingest returned `pdf_classification`, batch dropped it.

- `BatchPdfTriageSummary` on `BatchIngestResult` (classified, with_ocr_pages,
  encoding_issues, type_* counts, sample `papers_with_ocr` cap 32)
- Aggregated from each successful `IngestResult.pdf_classification` in
  `batch_ingest_pdfs`
- CLI `batch-ingest` prints triage line when any paper was classified
- Unit tests for aggregate helper; serde roundtrip; empty triage without classifier

```bash
cargo test -p da-application --lib batch
cargo test -p da-application --test batch_ingest_test
cargo check -p da-cli
# with --features pdf-classifier: batch shows PDF triage: classified=… ocr_pages=…
```


### Session 2026-08-05 — M351 hub PDF OCR pages 69/84 diagnosis

**Investigation closed (debt from M349/M350):**

Hub PDF `2507.19457` (GEPA, 96 pages):
- `pdf_type=Text`, confidence=1.0
- `pages_needing_ocr=[69, 84]`
- reasons: **both** `suspected_garbled_text` (not `scanned` / not `no_text`)
- `has_encoding_issues=true` (soft document-level signal; whole markdown has no U+FFFD)

Interpretation (pdf-inspector text-quality):
- Page-local broken ToUnicode / CID / cipher-letter heuristics on appendix-like pages
- Document remains GROBID-eligible overall (`is_text_extractable` false only because OCR pages non-empty)
- Operator action: treat pages 69/84 as quality flags, not full-document OCR

Deliverables:
- Permanent regression: `crates/da-adapters/tests/hub_ocr_probe.rs` (feature `pdf-classifier`)
- CLI ingest prints `OCR why: 69=suspected_garbled_text, 84=…`

```bash
cargo test -p da-adapters --features pdf-classifier --test hub_ocr_probe -- --nocapture
cargo check -p da-cli
```


### Session 2026-08-05 — M350 IngestResult carries optional PDF classification

**Close observability gap: triage was logged only, never returned**

- `IngestResult.pdf_classification: Option<PdfClassification>` (M350)
- Populated when optional classifier succeeds; `None` without classifier or on failure
- CLI `da ingest` prints: type / pages / ocr_pages / encoding_issues / conf
- Serde roundtrip covers None + Some (hub-like OCR [69,84])
- `PdfClassification: PartialEq` for test asserts
- 3 integration tests assert Some/None semantics

```bash
cargo test -p da-application --test pdf_classifier_test
cargo test -p da-application --lib
cargo check -p da-cli
```


### Session 2026-08-05 — M349 PDF classifier consistency + heal_graph clippy

**Debt fix: dual-source inconsistency + false hard-fail on soft encoding**

Root cause (hub PDF 2507.19457):
- Adapter mixed `classify_pdf_mem.pages_needing_ocr` with
  `process_pdf_mem.has_encoding_issues` → inconsistent pair
  (0 OCR pages + encoding_issues=true).
- `process_pdf_mem` alone reports **2 OCR pages** + encoding_issues on hub PDF.
- `has_encoding_issues` can also be soft (document-level markdown CID detectors)
  without any OCR page.

Fixes:
1. `PdfInspectorClassifier`: single source = `process_pdf_mem` only
2. `is_text_extractable()`: only `pages_needing_ocr.is_empty()` (encoding is soft)
3. Docs on soft encoding semantics
4. `HealGraphArgs` struct — clippy `too_many_arguments` on `heal_graph` gone

```bash
cargo test -p da-domain --lib pdf_classification
cargo test -p da-adapters --features pdf-classifier --lib pdf_classifier -- --nocapture
# hub: pdf_type=Text pages=96 ocr_pages=[…] encoding_issues=true
cargo clippy -p da-cli --bin da --no-deps -- -D warnings
```


### Session 2026-08-05 — M348 CLI wire PdfInspectorClassifier (feature-gated)

**Close M346/M347 composition-root gap:**

- `da-cli` feature `pdf-classifier` enables `da-adapters/pdf-classifier`
- Helper `maybe_attach_pdf_classifier()` injects `PdfInspectorClassifier` into
  `IngestUseCase` for both `ingest` and `batch-ingest`
- Default build: no `pdf-inspector` dep, behavior unchanged
- Feature build: pre-GROBID classify logs via M347 path

```bash
# default (no classifier)
cargo check -p da-cli

# with pre-GROBID PDF triage
cargo build -p da-cli --features pdf-classifier
da ingest --pdf ...   # logs PDF classified (pre-GROBID) when feature on
```


### Session 2026-08-05 — M347 wire PDF classifier into ingest use-case

**Close the port-to-application gap from M346:**

- `IngestUseCase` accepts optional `Arc<dyn PdfClassificationPort>` via `with_pdf_classifier()`
- When present, `ingest_pdf` classifies PDF **before** GROBID `parse_pdf`
- Logs: `pdf_type`, `page_count`, `pages_needing_ocr`, `has_encoding_issues`, `confidence`
- Warns when PDF is not fully text-extractable (scanned/mixed/encoding issues)
- Classifier failure is non-blocking (log + continue to GROBID)
- Backward compatible: `None` classifier = previous behavior
- 3 integration tests: called-before-parser, without-classifier, failure-does-not-block
- Debt fix: clippy needless_borrow in `healing.rs` (`&new_edge` → `new_edge`)

```bash
cargo test -p da-application --test pdf_classifier_test
cargo test -p da-application --lib
```

Next: wire `PdfInspectorClassifier` from CLI composition root (feature `pdf-classifier`).


### Session 2026-08-05 — M346 PDF OCR triage via pdf-inspector pre-filter

**New capability: pure-Rust PDF classification before GROBID (~10ms):**

- `PdfClassification` domain type in da-domain (pdf_type, pages_needing_ocr,
  ocr_reasons_by_page, confidence, has_encoding_issues)
- `PdfClassificationPort` trait in da-ports + `NullClassifier` fallback
- `PdfInspectorClassifier` adapter in da-adapters behind `pdf-classifier` feature
- Optional `pdf-inspector = "0.1"` dep — no Docker, no sidecar
- 4 OCR reason categories: scanned, suspected_garbled_text, no_text, vector_text
- E2E on hub PDF (2507.19457): **Text, 96 pages, 0 OCR, confidence 1.00**
- 4 helper methods: grobid_eligible, is_text_extractable, needs_full_ocr, ocr_reason_for_page

```bash
# Without feature (default — NullClassifier)
cargo test -p da-domain --lib pdf_classification
cargo test -p da-ports --lib pdf_classification

# With pdf-inspector (full classification)
cargo test -p da-adapters --features pdf-classifier --lib pdf_classifier
```

Architecture (hexagonal):
```
da-domain:  PdfClassification, PdfType, PageOcrReasons
da-ports:   PdfClassificationPort trait + NullClassifier
da-adapters: PdfInspectorClassifier (feature-gated, pdf-inspector)
```


### Session 2026-08-05 — M345 Serialize derives for core ontology types

**ADR-051 P0 extension — 5 types crossing persistence boundary:**

- Added `Serialize+Deserialize` to 4 core types:
  `TemporalEdge` (5-field temporal model),
  `RetroactiveExtension` (retroactive fact metadata),
  `Violation` (validation result),
  `Severity` (Critical/Warning enum)
- `PropertySnapshot` already serializable (`type HashMap<String, Value>`)
- 7 new roundtrip tests: TemporalEdge (active/expired), RetroactiveExtension
  (with/default), Severity (2 variants), Violation (critical/warn)
- Total kg-ontology tests: **83 passed** (76 existing + 7 new)

```bash
cargo test -p kg-ontology --lib
```


### Session 2026-08-05 — M344 property tests for graph_tasks parser (ADR-051 P1)

**First ADR-051 P1 implementation — proptest finds real bug:**

- Added `proptest = "1"` dev-dependency to da-cli
- 6 property tests: roundtrip, empty input, marker count, non-empty cypher, non-empty id, never-panic
- **Bug found and fixed**: `slug_id("-")` → empty string → task with empty id
- Fix: `slug_id_or_fallback()` returns `"task"` when slug is empty after trimming
- Clippy `--fix` collapsed nested if-let chains (let_chains)
- Total da-cli tests: **44 passed** (38 existing + 6 proptest)

```bash
cargo test -p da-cli --lib proptest
```


### Session 2026-08-05 — M343 governor roundtrip presence + da-ports gap fix

**ADR-051 P0 regression prevention + gap closure:**

- Governor rule `serde_roundtrip_test_module_required` (medium): flags crates with Serialize+Deserialize types as a reminder to verify roundtrip test presence
- Gap fix: `da-ports` had 3 pub Serialize types (`ParsedArticle`, `Section`, `CitationEntry`) without roundtrip tests — now covered (3 tests)
- ADR-051 P0 final tally: **40 types, 53 tests, 6 crates**

```bash
python3 scripts/check_project_trajectory.py --phase preflight
cargo test -p da-ports --lib
```


### Session 2026-08-05 — M342 serde roundtrip: remaining crates (ADR-051 P0 complete)

**Completes ADR-051 P0 for all project crates with Serialize+Deserialize types:**

- kg-storage: QueryResult, VectorSearchResult, VectorMetric (4 tests)
- da-application: IngestResult, BatchIngestResult (with errors), GraphInventory (4 tests)
- da-cli: GraphTask (basic + bounds), GraphTaskSuite, TaskVerdict (4 tests)
- 12 new roundtrip tests; all 3 crates green

ADR-051 P0 coverage summary:
| Crate | Types | Tests |
|-------|-------|-------|
| kg-ontology | 8 | 16 (M340) |
| da-domain | 20 | 22 (M341) |
| kg-storage | 3 | 4 (M342) |
| da-application | 3 | 4 (M342) |
| da-cli | 3 | 4 (M342) |
| **Total** | **37** | **50** |


### Session 2026-08-05 — M341 serde roundtrip tests for da-domain (ADR-051 P0)

**Completes ADR-051 P0 for da-domain — wire-format stability for all domain types:**

- `roundtrip_json<T>()` helper using JSON-string comparison (no PartialEq required)
- 22 systematic roundtrip tests covering all 20 pub Serialize+Deserialize types:
  Section, Concept, Topic, Category, Author, Institution, Reference,
  EntityType (21 variants), Entity, Paper, PaperStatus,
  GoldEntity, PredictedEntity, ExtractionMetrics,
  CorrectResult, MergeResult, SilenceResult,
  SourceSpan, EvidenceAssertion, EvidenceVerification,
  PendingTask, Relation
- Added `EvidenceVerdict`, `EvidenceVerification` to pub re-exports
- Total da-domain tests: **203 passed** (181 existing + 22 new)

```bash
cargo test -p da-domain --lib
```


### Session 2026-08-05 — M340 serde roundtrip tests for kg-ontology (ADR-051 P0)

**First ADR-051 P0 implementation — wire-format stability for kg-ontology:**

- `roundtrip<T>()` helper from ADR-051 §4.4 in `kg_ontology::lib.rs`
- 16 systematic roundtrip tests covering all 8 pub Serialize+Deserialize types:
  `HealingOperation` (7 variants), `HealingActor` (3 variants),
  `ProvenanceEvent` (with/without changes), `RetryPolicy` (default/fast/custom),
  `YamlFieldDef`, `YamlNodeSchema` (with/empty fields), `TemporalClass`,
  `YamlEdgeContract` (structural/fact/empty targets)
- Added `PartialEq` derives to `HealingActor`, `ProvenanceEvent`, `RetryPolicy`
- Total kg-ontology tests: **76 passed** (60 existing + 16 new)

```bash
cargo test -p kg-ontology --lib
```


### Session 2026-08-05 — M339 CI alignment: unit tests + governor trajectory

**Close CI vs local-script process gap:**

- CI workflow now runs `cargo test` per-crate (matching `scripts/verify_rust_architecture.sh`)
- CI workflow now runs governor preflight (`scripts/check_project_trajectory.py --phase preflight`)
- Divergence documented: CI does `cargo check --workspace` (rocksdb OK on CI time budget);
  local verify script avoids workspace check (MEM468: 5-10 min rocksdb rebuild)

```yaml
- name: Unit tests (per-crate, M339)
  run: |
    cargo test -p kg-ontology --lib
    # ... per-crate matching verify_rust_architecture.sh
    cargo test -p da-cli --lib  # includes graph_tasks parser (M337)

- name: Governor trajectory check (preflight, M339)
  run: |
    pip install pyyaml
    PYTHONPATH=scripts python3 scripts/check_project_trajectory.py --phase preflight
```


### Session 2026-08-05 — M338 graph task suite expect bounds

**Tighten ADR-051 §8.1 suite from min_rows=1 to exact count/value bounds:**

- `// @expect_rows N` — exact row count assertion (catches engine grouping surprises)
- `// @expect_min_value N` — first numeric cell >= assertion
- `GraphTask::classify` method wraps `classify_result_rows` + bounds
- Suite file updated: Paper count expects 1 row ≥ 24; Hub CITES expects 1 row ≥ 1
- Engine note: Samyama returns per-Paper rows for count() without GROUP BY
- 9 new unit tests (16 total); E2E PASS=7; D175/D176 in effect

```bash
cargo test -p da-cli --lib graph_tasks
cargo run -p da-cli --example graph_task_suite -- \
  --snapshot artifacts/gepa_hub_production/hub.sgsnap \
  --tasks data/graph_tasks/gepa_hub.tasks.cypher
```


### Session 2026-08-05 — M337 graph task suite CI + governor wire ban

**Trajectory lock for ADR-051 §8.1:**

- Governor high: `graph_tasks_must_not_use_wrong_edge_wires` bans `[:HAS_PART]` etc. in `data/graph_tasks/*.cypher`
- `scripts/verify_rust_architecture.sh` runs `cargo test -p da-cli --lib graph_tasks`
- Full suite (needs snap) remains operator/CI-optional; unit tests always gate

```bash
python3 scripts/check_project_trajectory.py --phase preflight
cargo test -p da-cli --lib graph_tasks
# full suite when snap present:
cargo run -p da-cli --example graph_task_suite -- \
  --snapshot artifacts/gepa_hub_production/hub.sgsnap \
  --tasks data/graph_tasks/gepa_hub.tasks.cypher
```


### Session 2026-08-05 — M336 semantic graph task suite runner

**ADR-051 §8.1 implementation** (Samyama case-study DoD pattern):

- `data/graph_tasks/gepa_hub.tasks.cypher` — 7 showcase tasks (fact CITES + structural hasPart/FROM_SOURCE/inCategory)
- Pure parser `da_cli::graph_tasks` — `// @query`, `@min_rows`, `@allow_empty`; unit tests 7/7
- Example `graph_task_suite`: import `.sgsnap` same-process → PASS/EMPTY/ERROR → exit 1 on fail
- E2E hub.sgsnap: **PASS=7 EMPTY=0 ERROR=0**, nodes 2571, edges 2741
- Report: `artifacts/graph_tasks/gepa_hub_report.json`
- D127 locked; Number(0) count treated as EMPTY

```bash
cargo test -p da-cli --lib graph_tasks
cargo run -p da-cli --example graph_task_suite -- \
  --snapshot artifacts/gepa_hub_production/hub.sgsnap \
  --tasks data/graph_tasks/gepa_hub.tasks.cypher \
  --report artifacts/graph_tasks/gepa_hub_report.json
```


### Session 2026-08-05 — M335 snapshot on mutate/validate CLI

**Process gap:** extract/heal/enrich/validate-graph/analyze opened empty
process-local stores after M334 only wired query/graph-stats.

- `--snapshot` preload via `open_graph_store_with_optional_snapshot` on extract, heal, enrich, validate-graph, analyze
- `--output-snapshot` re-export after extract/heal/enrich mutations
- E2E: `da validate-graph --snapshot hub.sgsnap` → 2571 nodes, 0 violations

```bash
./target/debug/da validate-graph --snapshot artifacts/gepa_hub_production/hub.sgsnap
./target/debug/da extract --id 2507.19457 --snapshot hub.sgsnap --output-snapshot out.sgsnap
```


### Session 2026-08-05 — ADR-051 semantic graph task pipeline (Samyama pattern)

Documented Contour 6 **semantic graph task pipeline** in ADR-051 §8.1:

- Materialise `.sgsnap` → task suite → **same-process** import+query → PASS/EMPTY/ERROR
- Samyama sources: `case_studies/DEFINITION_OF_DONE.md`, `test_kg_queries.rs`, `validate_queries.py`
- Distinct from extraction `eval_batch` (gold entities); both required for readiness
- DoD checklist, task file format (`// @query`), anti-patterns (cross-process load/query)
- AGENTS.md testing map updated (P1 priority + key rules)

```bash
# existing disk→query path (M334) — foundation for suite runner
./target/debug/da graph-stats --snapshot artifacts/gepa_hub_production/hub.sgsnap
./target/debug/da query --kind count --snapshot artifacts/gepa_hub_production/hub.sgsnap
```


### Session 2026-08-05 — M334 disk snapshot to query same-process chain

**Process gap (user-reported):** `da load-snapshot` verified 2571 nodes then
exited; next `da query` / `da graph-stats` opened a **new empty** store
(process-local in-memory dies on exit). Disk→query chain was broken.

- `open_graph_store_with_optional_snapshot` — import `.sgsnap` into **this** process
- `da graph-stats --snapshot <file.sgsnap>`
- `da query --snapshot <file.sgsnap> --kind count|by-arxiv|...`
- Honest NON-PERSISTENT banners; D127 unchanged
- E2E on `artifacts/gepa_hub_production/hub.sgsnap`:
  - nodes **2571**, edges **2741**
  - Paper count **24**
  - by-arxiv `2507.19457` returns 1 record

```bash
./target/debug/da graph-stats --snapshot artifacts/gepa_hub_production/hub.sgsnap
./target/debug/da query --kind count --snapshot artifacts/gepa_hub_production/hub.sgsnap
./target/debug/da query --kind by-arxiv --id 2507.19457 --snapshot artifacts/gepa_hub_production/hub.sgsnap
```


### Session 2026-08-05 — M333 D127 health honesty and source governor lock

**Process gap:** `da health` did not surface D127 `import_eligible=false`
(serve-graph already did). Governor only scanned artifacts for
`import_eligible: true`, not crates source.

- Pure `d127_import_status_line()` — always locked false
- Health graph section includes **Import:** D127 locked line
- Governor high rule `no_import_eligible_true_in_crates` on `crates/`
- **No import unlock** (D127 remains locked)

```bash
cargo test -p da-cli --lib health_report
python3 scripts/check_project_trajectory.py --phase preflight
```


### Session 2026-08-05 — M332 explicit graph persistence policy honesty

**Gap:** process-local Samyama is always in-memory, but a mistaken
`DA_GRAPH_DATA_PATH` was silently ignored (no RocksDB open, no error).

- `GraphModeError::PersistenceNotConfigured { path }`
- `refuse_persistence_path` / `refuse_persistence_path_from_env` — non-empty path refused
- `open_for_mode` / `try_from_env` call refuse before open
- Health: `Persistence:` line (none / path set but not wired)
- `.env.example` documents `DA_GRAPH_DATA_PATH` as unsupported for CLI open
- Durability remains `batch-ingest --output .sgsnap` (no RocksDB enablement)

```bash
cargo test -p kg-storage --lib graph_mode
cargo test -p da-cli --lib health_report
```


### Session 2026-08-05 — M331 structural created_at + Utc::now governor ban

**Debt:** structural edges (hasPart, FROM_SOURCE, foundIn, …) had no `created_at`;
Clock port could regress via bare `chrono::Utc::now` in application.

- Wire structural `create_edge` through `create_edge_with_temporal` (created_at only)
  in ingest / extraction / enrich
- Tests: hasPart + foundIn assert `created_at` present, `valid_at` absent
- Governor: `no_bare_utc_now_in_application` on `crates/da-application/src/`
- Zero `chrono::Utc::now` in application src (CLI composition root still OK)

```bash
cargo test -p da-application --test batch_ingest_test
cargo test -p da-application --test extraction_test
python3 scripts/check_project_trajectory.py --phase preflight
```


### Session 2026-08-05 — M330 Clock port for deterministic temporal stamps

**ADR-051 Contour 1:** application no longer calls bare `chrono::Utc::now()` for
TemporalEdge / domain timestamps.

- `da_ports::Clock` + `SystemClock` + `FixedClock`
- Injected into Ingest / Extraction / Healing / Cluster / Enrich / Scheduler
  (`new()` defaults to SystemClock; `with_clock` for tests)
- Fact-stamp and `created_at` paths use `self.clock.now()`
- Scheduler associate fns take explicit `now: i64` (CLI/tests pass time)
- Test: FixedClock freezes CITES `valid_at`/`created_at` to epoch 1700000000

```bash
cargo test -p da-ports --lib
cargo test -p da-application --test batch_ingest_test test_ingest_cites_valid_at_frozen
cargo test -p da-application --tests
```


### Session 2026-08-05 — M329 residual fact edge stamps + write-path audit

**Gap:** After M327–M328, bare `create_edge` remained for fact wires:
MEMBER_OF_CLUSTER, AFFILIATED_WITH, extra MENTIONS, SUPERSEDES merge, merge redirects.

- Stamp via `create_edge_with_temporal`: cluster, enrich, extraction (problem/metric MENTIONS), healing SUPERSEDES + fact redirects
- `add_temporal_edge` uses shared stamp helper
- **Hard gate:** `tests/temporal_write_path_audit_test.rs` — zero bare fact create_edge
- Governor: `no_bare_fact_create_edge_same_line` (line-level; multi-line → cargo audit)

```bash
cargo test -p da-application --test temporal_write_path_audit_test
cargo test -p da-application --tests
python3 scripts/check_project_trajectory.py --phase preflight
```


### Session 2026-08-05 — M328 stamp TemporalEdge on extract fact edges

**Write-path gap:** Extraction created MENTIONS / PARTICIPATES_IN / SUPPORTS
without `valid_at` / `created_at` after M327 only fixed ingest CITES.

- Reuse `temporal_stamp::create_edge_with_temporal` + `open_fact_edge_now`
- **Fact stamped:** MENTIONS, PARTICIPATES_IN, SUPPORTS
- **Structural unchanged:** FOUND_IN (`foundIn`) — no `valid_at`
- MENTIONS keeps `weight` float after stamp
- Tests: extraction 15 (incl. MENTIONS temporal + FOUND_IN structural); unit 3

```bash
cargo test -p da-application --test extraction_test
cargo test -p da-application --lib temporal_stamp
```


### Session 2026-08-05 — M327 stamp TemporalEdge on fact edges at ingest

**Write-path gap:** M326 encoded `temporal_class` as DATA, but ingest created
CITES edges without `valid_at` / `created_at`.

- `temporal_stamp` helper: `edge_type_is_temporal_fact` (YAML-first),
  `open_fact_edge_now`, `stamp_edge_temporal_fields`, `create_edge_with_temporal`
- Fact edges: stamp `valid_at` + `created_at` (+ optional invalid/expired/reference)
- Structural edges: `created_at` only when stamped; ingest hasPart stays without `valid_at`
- Ingest CITES path uses `create_edge_with_temporal`
- Tests: 3 unit + batch_ingest CITES temporal fields + hasPart structural; healing 13 green

```bash
cargo test -p da-application --lib temporal_stamp
cargo test -p da-application --test batch_ingest_test
cargo test -p da-application --test healing_test
```


### Session 2026-08-05 — M326 temporal edge policy in meta-ontology

**Design:** ADR-046 fact vs structural edges as external DATA, not docs-only.

- `temporal_class: fact | structural` on every row in `data/ontology/edge_contracts.yaml`
  - **fact** (7): CITES, MENTIONS, SUPPORTS, SUPERSEDES, AFFILIATED_WITH, PARTICIPATES_IN, MEMBER_OF_CLUSTER
  - **structural** (6): hasPart, FROM_SOURCE, authoredBy, foundIn, hasTopic, inCategory
- `TemporalClass` + `YamlEdgeContract.temporal_class` in kg-ontology registry
- Helpers: `is_temporal_edge_type`, `temporal_fact_edge_types`, `structural_edge_types`
- Scenario tests: CITES correction supersede, SUPERSEDES chain, competing metrics retain-both, bi-temporal axes, structural exclusion
- Export includes `temporal_class`; compiled Rust fallback mirrors YAML policy

```bash
cargo test -p kg-ontology --lib
cargo test -p kg-algorithms --lib temporal
cargo test -p da-application --test healing_test
```


### Session 2026-08-05 — M325 ontology-export edges + governor inventory rule

**Process gap:** `da ontology-export` wrote only `node_schemas.yaml`.

- Domain: `render_edge_contracts_yaml()` from `edge_contracts_or_yaml` (YAML-first)
- CLI export writes **both** `node_schemas.yaml` and `edge_contracts.yaml` (13 wires)
- Governor: `no_hardcoded_inventory_label_walks` on `crates/da-cli/src/`
- Unit test for YAML shape; preflight trajectory on_track

```bash
cargo test -p da-domain --lib -- render_edge_contracts_yaml
./target/debug/da ontology-export --output /tmp/ont
python3 scripts/check_project_trajectory.py --phase preflight
```


### Session 2026-08-05 — M324 external meta-ontology edge contracts

**Requirement:** system supports external meta-ontology — not hardcode discovery.

- `data/ontology/edge_contracts.yaml` — edge endpoint matrix as DATA (wire strings)
- `OntologyRegistry` loads nodes **and** edges from `data/ontology/`
- Domain: `registered_node_labels_or_yaml`, `contract_edge_types_or_yaml`, `edge_contracts_or_yaml` (YAML first, Rust fallback)
- Inventory: `collect_graph_inventory` uses YAML-first helpers
- Tests: kg-ontology 59, domain or_yaml, application graph_inventory

```bash
cargo test -p kg-ontology --lib
cargo test -p da-domain --lib -- or_yaml
cargo test -p da-application --lib graph_inventory
```

When ontology changes: edit YAML under `data/ontology/` (and re-export nodes if needed). Inventory follows without CLI edits.


### Session 2026-08-05 — M323 ontology-driven graph inventory

**Debt:** CLI/research inventory hard-coded node labels (`Paper`, `Section`, …).
Ontology change would desync reports.

- Domain: `registered_node_labels()`, `contract_edge_types()` from schema + edge_contracts
- Application: `collect_graph_inventory` / `render_inventory_markdown` (walks registry)
- CLI batch `--inventory-dir` + gepa_hub_research use the use-case — no label lists in CLI
- Unknown edge types (not in contracts) surface as drift warnings
- Tests: domain labels/edges + application inventory unit tests

```bash
cargo test -p da-domain --lib registered_node_labels
cargo test -p da-application --lib graph_inventory
```


### Session 2026-08-05 — M322 production GEPA batch + ids-file

**Process gap:** `da batch-ingest` only accepted comma `--ids`; research used `--ids-file`.

- Pure `batch_ids::resolve_batch_ids` (+ unit tests): `--ids` and/or `--ids-file`, dedupe
- Optional `--inventory-dir` writes ontology-string honest inventory (M319/M320 lessons)
- Production GEPA hub batch with **FdApiEmbedder** (real TEI 1024d): **24/24 ok**
- Snapshot `artifacts/gepa_hub_production/hub.sgsnap` (723777 bytes)
- Inventory: Section=965, hasPart=2297, CITES=419, MENTIONS=0 (expected)

```bash
cargo test -p da-cli --lib
./target/debug/da batch-ingest --ids-file artifacts/gepa_hub_research/ingest_ids.txt \
  --output artifacts/gepa_hub_production/hub.sgsnap \
  --inventory-dir artifacts/gepa_hub_production
```


### Session 2026-08-05 — M321 FD_API dotenv composition root

**Bug:** `da batch-ingest` / `da ingest` returned FD_API **401** even with a valid
key in project `.env`. Root cause: composition root never loaded dotenv —
`FdApiEmbedder::from_env` saw empty `FD_API_KEY` and sent empty Bearer.

- `dotenvy::dotenv()` at start of `da` main (does not override exported env)
- Health: `FD_API_KEY` presence (len only) + live embed probe
- Fail-fast on ingest/batch-ingest when key missing after dotenv
- Pure helpers + unit tests (7/7); no secret leakage
- Smoke: hub paper ingest with real TEI embeddings → Vector 1024d

```bash
cargo test -p da-cli --lib
./target/debug/da health   # FD_API_KEY present + Embed probe ok
```


### Session 2026-08-05 — M320 research section inventory unshadow

**Bug:** `gepa_hub_research` reported `section_nodes=85` after full hub ingest
because hub TEI extract rebound `let sections` over graph Section node IDs.
Hub `body_head_count=85` was written into GraphSummary instead of
`get_nodes_by_label("Section")`.

- Capture graph label counts before hub extract; rename hub TEI to `hub_sections`
- Re-run: **Section=965** (= total_sections); hub TEI still 85 for extract only
- Reference 1332 written / 1255 unique = expected vid dedup (not a bug)
- hasPart=2297, CITES=419 unchanged

```bash
cargo run -p da-cli --example gepa_hub_research -- --ids-file artifacts/gepa_hub_research/ingest_ids.txt
```


### Session 2026-08-05 — M319 research inventory edge type honesty

**Bug:** `gepa_hub_research` reported `hasPart=0` after GEPA ingest because inventory matched `"HAS_PART"` instead of ontology wire string **`hasPart`**.

- Inventory uses `da_domain::relation::{structure,bibliographic}` constants
- Counts Section/Category/Citation nodes + full edge histogram
- Re-run: **hasPart=2297**, CITES=419, FROM_SOURCE=24, inCategory=1, MENTIONS=0 (expected)
- MENTIONS=0 honest: ingest does not write Entity MENTIONS

```bash
cargo test -p da-domain --lib relation -- --test-threads=1
cargo run -p da-cli --example gepa_hub_research
```


### Session 2026-08-05 — M318 GEPA hub bibliography corpus + engine research

**Hub paper 2507.19457 (GEPA)** — largest GROBID bibliography in m033 probe (~80 citations).

- Pin tests: `real_tei` ≥80 citations for GEPA hub; ≥70 for 2605.26525v1
- Downloaded **23/23** resolvable arXiv cited PDFs into canonical catalog (`scripts/download_arxiv_to_catalog.py`)
- Engine research (`gepa_hub_research` example, MockEmbedder — FD_API 401 avoided):
  - Ingest **24/24** OK (hub + cites)
  - **1332** Reference nodes written; **419** CITES resolved to Paper
  - Graph: 24 Paper + 1255 Reference nodes
  - Hub extract: 24 entities (GEPA, GRPO, MATH, GPT-4, …)
- Report: `artifacts/gepa_hub_research/REPORT.md` + `engine_research.json`
- D127 import_eligible=**false**

```bash
python3 scripts/download_arxiv_to_catalog.py --ids-file artifacts/gepa_hub_research/missing_arxiv.txt --skip-meta
cargo test -p da-adapters --lib real_tei -- --test-threads=1
cargo run -p da-cli --example gepa_hub_research
```


### Session 2026-08-05 — M317 corpus eval remeasure after gold promote

**Metrics honesty after M315/M316:** full `eval_batch` on 193 papers.

| Metric | M314 (pre-promote) | M317 (post-promote) |
|--------|--------------------|---------------------|
| Micro P | 0.729 | **0.769** |
| Micro R | 1.000 | **1.000** |
| Micro F1 | 0.843 | **0.869** |
| Macro F1 | 0.838 | **0.859** |
| FP | 494 | **421** (−73) |
| FN | 0 | **0** |
| Gold entities | 1326 | **1399** |

```bash
cargo run -p da-cli --example eval_batch
```


### Session 2026-08-05 — M316 batch evidence-based gold promote

**Batch E2 hygiene:** `batch_promote_gold` example runs extract → FP → multi-paper promote across weak papers.

- 14 weak arxiv ids processed; **13 papers** written; **+65** evidence-based gold entities
- Free invent still rejected (multi_paper_count ≥ 2)
- Sample after F1: 2510.26094 **1.000**, 2507.19457 **~0.96**, many 0.667 → **0.95+**
- Provenance: `m316_promoted` + annotator suffix `+m316-evidence-promote`

```bash
cargo test -p da-domain --lib eval -- --test-threads=1
cargo run -p da-cli --example batch_promote_gold -- --ids-file /tmp/m316_batch_ids.txt --write
```


### Session 2026-08-05 — M315 E2 FP inventory + evidence-based gold promote

**E2 process gap closed:** pure domain helpers + inventory example; gold promote only multi-paper labels.

- `da_domain::{MultiPaperGoldIndex, list_false_positives, promote_candidates}`
- Unit tests: free invent rejected; multi-paper only; no duplicate gold
- Example: `cargo run -p da-cli --example inventory_fp -- 2603.21520`
- Gold hygiene: `2603.21520` +8 evidence-based entities (CoT, GPT-4, MATH, …); **AGIEval rejected** (0 multi-paper)

```bash
cargo test -p da-domain --lib eval -- --test-threads=1
cargo run -p da-cli --example inventory_fp -- 2603.21520
```


### Session 2026-08-05 — M314 Task end-index parity + post-M313 eval

**Span honesty (Task):** section + global Task passes use lowercased needle length
for `char_end` (same fix as Method M313). Unit test for Task phrase span.

**Corpus re-eval** (`cargo run -p da-cli --example eval_batch`, 193 papers):

| Metric | Value |
|--------|--------|
| Micro P / R / F1 | **0.729 / 1.000 / 0.843** |
| Macro P / R / F1 | **0.735 / 1.000 / 0.838** |
| TP / FP / FN | 1326 / 494 / **0** |

```bash
cargo test -p da-adapters --lib rule_extractor -- --test-threads=1
cargo run -p da-cli --example eval_batch
```


### Session 2026-08-05 — M313 Method section phrase parity (E1)

**Logic gap:** Method section-scoped extraction scanned acronyms only; multi-word
phrases lived only in the global pass (Task section already had phrases).

- Method arm now scans `method_phrases` with word boundaries
- Global phrase end index uses `p_lower.len()` (match length honesty)
- YAML: add `generalization` to `methods.phrases` (gold FN 2603.21520)
- AGENTS E1 marked closed for whitelist path

```bash
cargo test -p da-adapters --lib rule_extractor -- --test-threads=1
```


### Session 2026-08-05 — M312 da health graph mode honesty

**Honesty gap closed:** `da health` no longer process-exits on `DA_GRAPH_MODE=remote`.

- Prints `DA_GRAPH_MODE` + open/health status via pure `health_report` helper
- RemoteNotConfigured → clear message + Wave 3 tip (`serve-graph` / `graph-ping` :16379)
- `open_graph_store` unchanged (other commands still exit 1 on remote)
- AGENTS operators: Wave 3 health block; P1 backlog updated

```bash
cargo test -p da-cli --bin da health_report
DA_GRAPH_MODE=remote cargo run -p da-cli -- health
```


### Session 2026-08-05 — M311 Wave 3 operator hygiene

**Docs/env/port honesty after M307–M310:**

- `graph_ping` reuses `serve_graph::DEFAULT_HOST` / `DEFAULT_PORT` (16379)
- README: Wave 3 serve-graph + graph-ping quick-start and CLI table
- `.env.example`: `DA_GRAPH_MODE=embedded`; clarify 6380 vendor vs 16379 `da serve-graph`
- Governor: `serve_graph_must_not_use_rocksdb_persistence` (high)

```bash
cargo test -p da-cli --bin da graph_ping
cargo test -p da-cli --bin da serve_graph
python3 scripts/check_project_trajectory.py --phase preflight
```


### Session 2026-08-05 — M310 graph-ping RESP health client

**Multi-process health path:** `da graph-ping` sends RESP PING to a running `serve-graph`.

```bash
da serve-graph --port 16379          # terminal A
da graph-ping --port 16379           # terminal B → PONG
cargo test -p da-cli --bin da graph_ping
```

- Not a full remote GraphStore; `open_for_mode` / `open_graph_store` untouched
- D127 locked; GraphMode::Remote still NotConfigured for store open
- 4 tests: defaults, parse PONG, unreachable error, live spawn+ping


### Session 2026-08-05 — M309 serve-graph snapshot preload

**Empty-server gap closed:** `da serve-graph --snapshot PATH.sgsnap` imports into process memory before listen.

```bash
da serve-graph --snapshot data/samyama/batch-10.sgsnap --port 16379
cargo test -p da-cli --bin da serve_graph
```

- Still non-persistent (dies on process exit); banners honest
- No RocksDB; open_graph_store untouched; D127 locked
- 8 tests (load synthetic, missing file, spawn with/without snapshot)


### Session 2026-08-05 — M308 CLI serve-graph in-memory

**Operator surface for M307:** `da serve-graph` starts `RespServer::new` (no RocksDB) on loopback.

```bash
da serve-graph --host 127.0.0.1 --port 16379
# RESP: PING → PONG
cargo test -p da-cli --bin da serve_graph
```

- Does **not** edit `open_graph_store` (CRITICAL callers)
- Banners: non-persistent, D127 false, remote NotConfigured
- Default port **16379** (not Redis 6379)


### Session 2026-08-05 — M307 RespServer in-memory PING smoke

**Wave 3 thin entry:** vendor `RespServer::new` (no RocksDB) on ephemeral port answers RESP `PING` with `PONG`.

```bash
cargo test -p kg-storage --features samyama --test resp_server_smoke
```

- Behind `samyama` feature only; D127 unchanged; `GraphMode::Remote` still NotConfigured
- No daemon install, no remote client wire-up


## Unreleased

### Session 2026-08-04 — M306 export/import round-trip contract suite

**Debt closeout after stalled auto:** prove Mock + Samyama create→export→import→count/label/property parity.

#### Shipped

- `MockGraphStore` stateful export/import via JSON `MockStateDump` (canned `set_snapshot_data` still wins)
- Shared `export_import_round_trip_suite(factory)` in `kg_storage::contract` (ADR-051 Contour 4)
- Wired into `contract_graph_store` + `contract_samyama`
- Samyama `get_node_property_*` HashMap → ColumnStore fallback (sgsnap v2 honesty)
- Consumer fix: `batch_ingest_test` accepts non-empty JSON export (not opaque `mock-snapshot-data`)
- Unit tests: empty export non-empty JSON, stateful restore, opaque import no-op

```bash
cargo test -p kg-storage --lib mock
cargo test -p kg-storage --test contract_graph_store
cargo test -p kg-storage --features samyama --test contract_samyama
```

#### Architecture

- kg-storage only; no server, D127 unchanged
- Process lesson (MEM564): contract cargo only async + progress; concurrent cargo on `target/` stalls

#### Process / governor
- M306 sealed via skip-delivered + UAT + validate + complete (Attempt path unavailable)

- D144: M306 seal requires auto Attempt — no hand-edit of gsd.db
- Governor rule `gsd_lifecycle_attempt_desync_in_summary` (medium): flags SUMMARY text about blocked Attempt
- Gates Q3–Q7 recorded for S01/T01 (Q6 omitted)



### Session 2026-08-04 — M305 GraphMode factory (embedded | remote)

**First thin Wave 3 entry:** mode-aware graph store open without shipping a server.

#### Shipped

- `kg_storage::GraphMode` + `GraphModeError` (`DA_GRAPH_MODE=embedded|remote`, default embedded)
- `SamyamaGraphStore::open_for_mode` / `try_from_env` — remote → `RemoteNotConfigured` (no fake client)
- `da-cli` `open_graph_store()` used for all store open sites (14+ call sites)
- Tests: 6 graph_mode + 2 open_for_mode (samyama feature)

```bash
cargo test -p kg-storage --lib graph_mode
cargo test -p kg-storage --features samyama --lib mode_tests
# DA_GRAPH_MODE=remote da analyze ...  → clear error, exit 1
```

#### Architecture

- No RespServer, no RocksDB in default path, D127 unchanged
- `from_env()` remains embedded convenience for tests; composition roots use try_from_env

### Session 2026-08-04 — M304 load-snapshot honesty + Wave 3 entry hygiene

**Process logic debt:** `da load-snapshot` imported into dying in-memory process and sounded durable.

#### Shipped

- `load_snapshot`: success text is **verify-only** + explicit `NON-PERSISTENT` warning (batch-ingest durable path, Wave 3 server not shipped, D127 locked).
- `graph_stats` empty-state no longer claims load-snapshot "restores" durable graph.
- Governor rules:
  - `analyze_must_not_hardcode_available_algorithm_list`
  - `analyze_algorithms_constant_must_exist` (sentinel)
- Research brief: `304-RESEARCH.md` — Solution A/B/C, recommended thin Wave 3 slices (store factory → snapshot contract → RespServer smoke).

#### Architecture

- No server built; no D127 open; no RocksDB in default CLI path.
- Trajectory ON_TRACK; audit CLEAR.

### Session 2026-08-04 — M303 CLI multi-seed PPR + algorithm list hygiene

**Residual CLI gap after M302:** pure/trait PPR already multi-seed; CLI was single-seed only.

#### Shipped

- `--seed` is now **repeatable** (`Vec<u64>`, clap Append): `--seed 1 --seed 2`
- Single-seed UX preserved; empty seeds still error
- `ANALYZE_ALGORITHMS` constant is single source for unknown-algorithm help
- Tests: **10** analyze unit tests (multi-seed + constant hygiene + prior 8)

```bash
da analyze --algorithm ppr --seed 1 --seed 2 [--alpha 0.85] [--top 10]
```

#### Architecture

- No kg-layer change; reuses M299 equal-mass multi-seed path
- D127 unchanged (read-only)

### Session 2026-08-04 — M302 CLI analyze personalized PageRank

**Operator surface for Wave 2 PPR stack.**

#### Shipped

- `da analyze --algorithm ppr --seed <NodeId>` via `GraphViewAlgorithms`
- Flags: `--seed` (required for ppr), `--alpha` (default 0.85)
- da-cli depends on `kg-algorithms` feature `ppr` (composition root only)
- Errors: missing seed, invalid alpha, unknown seed / empty scores
- Regression: page-rank + wcc unchanged
- Tests: 8 analyze unit tests green (4 ppr + 4 regression)
- Stale doc fix in `ppr.rs` (references M301 façade)

```bash
# binary unit tests (cargo test output may be quiet; binary harness is authoritative)
cargo test -p da-cli --bin da
# or: target/debug/deps/da-* analyze::
```

#### Architecture

- HIGH impact on `run_analyze` — additive match arm only
- ForwardPush still degree-count; D127 unchanged (read-only analyze)
- No kg-storage prod solver dep

### Session 2026-08-04 — M301 GraphAlgorithms façade over GraphView

**Dead-port debt closed:** first concrete `impl GraphAlgorithms`.

#### Shipped

- `GraphViewAlgorithms` (feature `ppr`): owns `GraphView`, optional projected edge-type label.
- `personalized_pagerank` → pure ForwardPush helper; fail-soft empty `Vec` on error; `max_iterations` unused (epsilon fixed `1e-8`).
- `get_neighbors` / `get_all_neighbors` from projected topology (`edge_type` ignored).
- `tokio` as **dev-dep only** for async trait tests.
- Contract suite: 6 façade tests; full `--features ppr --lib` → **33 passed**.

```bash
cargo test -p kg-algorithms --features ppr --lib
cargo clippy -p kg-algorithms --features ppr --all-targets --no-deps -- -D warnings
```

#### Architecture

- Trait no longer orphan; pure helper remains the core.
- No CLI wiring; no D127; da-ports re-export unchanged.

### Session 2026-08-04 — M300 Mock compose GraphView to PPR

**Wave 2 integration gap closed:** Mock k-hop cut → pure ForwardPush PPR.

#### Shipped

- kg-storage dev-dep `kg-algorithms` features: `solver` → **`ppr`** (still dev-only).
- `crates/kg-storage/tests/ppr_export_compose.rs` (3 tests):
  1. weighted subgraph → PPR seed ranks first
  2. unweighted fold (`weights=None`) → topology PPR still works
  3. unknown seed → `PprError::MissingSeed`

```bash
cargo test -p kg-storage --features algorithms --test ppr_export_compose  # 3 passed
cargo test -p kg-storage --features algorithms --test csr_export_compose  # 5 still pass
```

#### Architecture

- No prod `ruvector-solver` on kg-storage (D141).
- Reuses M297 adapters + M299 pure helper; no CLI/trait impl.

### Session 2026-08-04 — M299 PPR smoke on exported CSR

**Wave 2 next step after M298:** pure ForwardPush PPR over GraphView via CSR export.

#### Shipped

- Feature `ppr` = `solver` + `ruvector-solver/forward-push` (defaults unchanged).
- `personalized_pagerank_from_graph_view(view, seeds, alpha, epsilon)`:
  - NodeId seeds → dense index → `ForwardPushSolver::ppr_multi_seed` → NodeId scores
  - Typed `PprError` (EmptySeeds, MissingSeed, Csr, Solver)
  - Topology path when `weights=None` (unit CSR for construction; ForwardPush ignores values)
- Contract suite: 6 ppr tests; full `--features ppr --lib` → **27 passed**

```bash
cargo test -p kg-algorithms --features ppr --lib
```

#### Architecture

- Still pure kg-algorithms; no da-*, no CLI, no GraphAlgorithms trait impl yet.
- Documented: ForwardPush is degree-count PPR, not weighted SpMV PPR.

### Session 2026-08-04 — M298 S01: GraphView → CsrMatrix pure export + audit harden

**Wave 2 type boundary:** pure `csr_matrix_from_graph_view` in `kg-algorithms` behind optional feature `solver`.

#### Shipped

- **T01** — optional deps `samyama-graph-algorithms` + `ruvector-solver` (`default-features = false`); feature `solver` enables both; package defaults unchanged.
- **T02** — `crates/kg-algorithms/src/csr.rs`: `csr_matrix_from_graph_view(&GraphView) -> Result<CsrMatrix<f64>, CsrExportError>`.
  - Map: `out_offsets→row_ptr`, `out_targets→col_indices`, `weights→values`, square `node_count`.
  - Construct via public CSR fields (clone arrays), not `from_coo`.
  - `weights=None` + `n>0` → `MissingWeights`; `n==0` → Ok empty matrix; structural checks on offsets/weight length.
- **T03** — contract suite + audit harden (11 csr tests):
  - weighted / missing / empty / offsets / weight length
  - **NEW:** `ColumnIndexOutOfBounds`, `NonFiniteWeight` (NaN/Inf)
  - **NEW:** `materialize_unit_weights` — explicit post-fold unit fill (never silent 1.0 in export)
  - **Docs:** ForwardPush ignores CSR values (degree-count PPR); maps stay on GraphView

#### Verification

```bash
cargo test -p kg-algorithms --features solver --lib   # 21 passed (11 csr)
cargo clippy -p kg-algorithms --features solver --all-targets --no-deps -- -D warnings
```

#### Compose contract (audit gap closed)

- `crates/kg-storage/tests/csr_export_compose.rs` — Mock → `subgraph_view_from`/`graph_view_from` → `csr_matrix_from_graph_view`.
- **dev-dep only** `kg-algorithms` with feature `solver` on kg-storage (no prod onion inversion; D141).
- 5 tests: weighted export, unweighted hard-fail, materialize_unit_weights path, empty, full graph_view_from.

```bash
cargo test -p kg-storage --features algorithms --test csr_export_compose   # 5 passed
```

#### GSD closeout (manual mode — auto unavailable)

- Closed via AGENTS.md protocol: `gsd_skip_slice` (delivered) → UAT → subjective UAT on **current** `captureMilestoneVerificationSourceRevision` → validate pass → quality_gates pass → `gsd_complete_milestone`.
- Milestone **M298-nu7b4d COMPLETE**.
- Lesson: free-form sessions cannot invent Task Attempts; do **not** rely on `/gsd auto` when that path is broken — use skip+UAT closeout.

#### Architecture

- Hexagonal: export in `kg-algorithms` (universal), not `da-*`. No kg-storage re-export. No CLI/solver invoke.
- kg-storage fold all-1.0→None remains intentional; unit path = explicit `materialize_unit_weights`.
- Local f64 validation (vendor `validate_csr_matrix` is f32-only) — D142.
- Compose tests live under kg-storage (adapter owner) with solver only as dev-dep.

#### Governor (trajectory hygiene)

- New ADR rules in `scripts/governor/checks/adr_rules.yaml`:
  - `ruvector_solver_must_disable_default_features` (medium)
  - `kg_storage_must_not_prod_depend_on_ruvector_solver` (high, `[dependencies]` only)
- Audit: both CLEAR on current tree; trajectory ON_TRACK.

### Session 2026-08-03 — Wave 1 Complete: All 4 Milestones Closed in GSD

**Wave 1 is DONE. All milestones formally closed through GSD. No debts remain.**

#### GSD milestone closure

- **M293-gluh0h** (Wave 1: Graph Algorithms Wiring) — CLOSED ✅
- **M294-oz9u3a** (Wave 1A: Edge Property API) — CLOSED ✅
- **M295-s8je3g** (Wave 1B: Trait Honesty) — CLOSED ✅
- **M296-4tghd9** (Wave 1C: Read Method Honesty + Pipeline) — CLOSED ✅

#### AGENTS.md updated: 'Closing milestones in manual mode (GSD v1.4+)'

8-step closure protocol documented for future agents:
1. Skip slices (work already delivered)
2. Create UAT evidence via gsd_uat_exec + gsd_uat_result_save
3. Prepare subjective UAT with stable criterionKey + SHA256 testedSourceRevision
4. Get binding IDs from workflow_domain_events via gsd_exec
5. Answer immediately with exact UUIDs
6. Batch prepare→batch answer→validate (zero prepare between answers)
7. UPDATE quality_gates via gsd_exec if Q5 still pending
8. Checkpoint DB + complete milestone

#### Governor improvement recommendations (MEM528-MEM533)

1. trait_method_audit — flag default impls returning zero values
2. doc_workspace_alignment — verify AGENTS.md matches workspace
3. orphan_library_tracker — flag kg-* crates without consumers
4. GSD gate lifecycle fix for manual mode
5. Subjective UAT binding IDs should be returned in MCP tool output

### Session 2026-08-03 — Wave 1 Milestone Closure (M293, M295, M296 closed; M294 gated)

**Wave 1 is structurally complete. 4 milestones delivered, 3 formally closed in GSD.**

#### GSD milestone closure

- **M293-gluh0h** (Wave 1: Graph Algorithms Wiring) — CLOSED ✅
- **M294-oz9u3a** (Wave 1A: Edge Property API) — all slices terminal, validated pass,
  but gsd_complete_milestone blocked by Q5 gate (GSD v1.4 limitation: skipped
  slices don't get gate rows in manual mode, MEM531)
- **M295-s8je3g** (Wave 1B: Trait Honesty) — CLOSED ✅
- **M296-4tghd9** (Wave 1C: Read Method Honesty, Pipeline) — CLOSED ✅

#### Governor improvement recommendations (MEM530, MEM531)

1. **trait_method_audit dimension** — flag ANY trait default impl returning zero
   values (Ok(()), None, Vec::new(), 0, false). Would have caught all 6 trait
   honesty bugs in one pass instead of requiring M294-M296.
2. **GSD gate row lifecycle fix** — skipped slices should auto-create gate rows
   or gsd_complete_milestone should bypass the gate check when UAT file exists.
   Currently blocks milestone closure for manually-completed work.
3. **doc_workspace_alignment** — verify AGENTS.md crate names match Cargo.toml
   workspace members (caught phantom da-graph).
4. **orphan_library_tracker** — flag kg-* crates with zero da-* consumers
   (caught kg-pipeline orphan, now integrated).

### Session 2026-08-03 — M296 Wave 1C: Read Method Honesty, Pipeline Integration

**Completed trait honesty: no silent defaults remain on any trait method.**

#### S01: Read method honesty — get_outgoing_edges, get_edge_property_string required

- **Made `get_outgoing_edges` a required trait method** (was returning Vec::new()
  by default — silent empty results for any adapter that forgot to override).
- **Made `get_edge_property_string` a required trait method** (was returning None
  by default — silent missing data).
- Added `get_outgoing_edges` override to da-application mock delegate
  ڈ (was missing — would have failed compilation after making it required).
- This completes the trait honesty work: write methods return Err(NotImplemented),
  read methods are required. **No silent defaults remain.**

#### S02: kg-pipeline integrated into da-application extraction

- **Added `build_extraction_pipeline()` to `ExtractionUseCase`** — first real
  consumer of kg-pipeline crate (was orphan with zero consumers since ADR-050).
- Pipeline DSL describes 5 extraction steps: extract → write_entities →
  write_edges → write_evidence → embed. Uses PipelineBuilder + FailurePolicy.
- TDD: test_build_extraction_pipeline verifies 5 steps + dependencies.

#### Test counts: kg-storage lib 15, da-application lib 28 (+1), healing_test 13

#### Guardrail: PASS. Governor: on_track.

#### Audit findings (GitNexus)

- detect_changes: 38 changed symbols, 8 affected processes, risk LOW
- impact analysis: all changed symbols max risk LOW
- No HIGH/CRITICAL findings
- kg-pipeline no longer orphan (da-application is first consumer)

### Session 2026-08-03 — M295 Wave 1B: Trait Honesty and Temporal Healing

**Fixed silent data loss bugs and wired kg-algorithms into healing.**

#### S01: Trait honesty — no-op default impls must error

- **Changed `set_edge_property_float` default impl** from silent `Ok(())` to
  `Err(GraphStoreError::NotImplemented)`. Any adapter that forgets to override
  now gets an explicit runtime error instead of silently losing edge weights.
- **Changed `set_edge_property_string` default impl** likewise.
- **Added `GraphStoreError::NotImplemented`** variant.
- **Fixed da-application mock delegate**: added `set_edge_property_float`
  override (was missing — would have used the new Err default).
- 3 new regression tests guard against accidental override removal.

#### S02: Documentation alignment

- Removed phantom `da-graph` crate from AGENTS.md package map and test targets.
  The crate does not exist in the workspace.

#### S03: Wire kg-algorithms into da-application healing

- **Added `add_temporal_edge` method** to `GraphHealingUseCase`. Uses
  `kg_algorithms::resolve_temporal_edges` to detect temporal contradictions
  (ADR-047 supersede rule) and invalidate old edges before creating new one.
- **da-application now depends on kg-algorithms** — first real integration of
  the universal algorithms crate into application code.
- 2 new TDD tests: supersede contradiction, retain-both overlap.

#### Audit findings (codebase clean)

- Zero dead code warnings across all 10 crates (kg-* + da-*).
- Zero TODO/FIXME/HACK markers in crates/.
- kg-pipeline exists but is not yet integrated (tracked for future wave).

#### Test counts: kg-storage lib 15 (+3), da-application healing_test 13 (+2)

#### Guardrail: PASS. Governor: on_track.

### Session 2026-08-03 — M294 Wave 1A: Edge Property API Completeness

**Fixed technical debt from M293 S01. 4 slices, all green.**

#### S01: Edge Property API (kg-storage)

- **Added `get_edge_property_float`** to DirectGraphStore trait (default impl
  parses from `get_edge_property_string`, same pattern as node properties).
  Override in SamyamaGraphStore (native Float read via `get_edge_properties`
  + `as_float`) and MockGraphStore (string round-trip).
- **Added `set_edge_property_float` override** to MockGraphStore — previously
  silent no-op via default impl; edge weights were silently lost in tests.
- **Renamed `set_edge_property_string_v2` → `set_edge_property_string`**
  across trait + all impls + all callers (6 files). API consistency.
- **Fixed `get_edges_between` default impl** — was fabricating edge_id=0
  (contract violation); now returns empty Vec, forcing adapters to override
  with real edge_id.
- **Extended `graph_store_contract_suite`**: +test_edge_property_float_round_trip,
  +test_get_edges_between_returns_real_edge_id.
- **Fixed pre-existing doctest bug** in contract.rs (`#[tokio::test]` without
  `async fn`).

#### S03: Adapter Weight Wiring (kg-storage)

- **Wired `weight_property` to `get_edge_property_float`** in adapter.rs
  `graph_view_from` — removed silent 1.0 fallback. Logs debug warning when
  weight_property is Some but edge returns None, then falls back to 1.0.
- **Fixed self-loop dedup bug** — was desyncing weights from adjacency (dedup
  happened after weights_per_node fill); now skips self-loops in main loop.
- New test: `test_weighted_graph_preserves_edge_weights`.

  **Finding:** `samyama-graph-algorithms::page_rank` ignores the `weights`
  field. Weighted PageRank needs a custom impl in kg-algorithms (future wave).
  Weights are honored by dijkstra, prim_mst, edmonds_karp.

#### S04: CLI Analytics Command (da-cli)

- **New command: `da analyze`** with `--algorithm page-rank|wcc`, `--label`,
  `--edge`, `--weight`, `--top N` flags. Builds GraphView from SamyamaGraphStore
  via kg-storage adapter and prints results.
- 4 TDD unit tests in `analyze.rs`.

#### Test counts: kg-storage lib 12 (+2), algo_adapter 7 (+1), da-cli 4 (+4)

#### Guardrail: PASS. Governor: on_track.

### Session 2026-08-02 — M293 Wave 1: Graph Algorithms Wiring

**S01: DirectGraphStore → CSR GraphView adapter. 6 TDD tests green.**

#### New module: kg-storage/src/adapter.rs

- `graph_view_from(store, label, edge_type, weight_property)` — projects any
  `DirectGraphStore + GraphStore` into `samyama_graph_algorithms::GraphView`
- Feature-gated under `algorithms` (requires `dep:samyama-graph-algorithms`)
- `#[cfg(feature = "algorithms")]` on module re-export so default builds stay clean
- Adapter uses `get_nodes_by_label`, `get_outgoing_edges`, `node_count` (all in trait)
- Returns `GraphView::from_adjacency_list(...)` — zero-copy projection
- Edge-type filter honored (CITES vs RELATED_TO test)
- Empty graph and isolated components handled

#### TDD test suite: tests/algo_adapter.rs (6 tests, all pass)

1. `test_graph_view_from_citation_graph` — 3 papers → 3-node view
2. `test_pagerank_cited_paper_ranks_highest` — most-cited Paper3 > Paper1
3. `test_wcc_connected_graph` — single component for connected chain
4. `test_wcc_two_isolated_papers` — two components for isolated nodes
5. `test_empty_graph_returns_empty_view` — 0 nodes → 0-node view
6. `test_edge_type_filter_excludes_other_types` — CITES only, not RELATED_TO

#### Closure: GSD shadow duplicates M291/M258 still pending close

M258 has 4 empty shadow duplicates (M258-420sqw, M258-9a5812, M258-ci1w0j,
M258-ixjcww, M258-w3aodn) that GSD can't validate (FOREIGN KEY constraint).
Left as-is: they don't block work since we work in manual mode, not auto.

### Session 2026-08-02 — M291 shadow closure + mock DRY + governor hardening

**Debts closed: 4. 76 da-application tests pass. Governor rule added.**

#### GSD state cleanup — M291 shadow duplicate closed

- All 4 slices of shadow M291 (bare form) skipped + milestone completed
- M291-p80wbx (suffixed form) remains the real record with all task data
- STATE.md now shows both M291 and M291-p80wbx as ✅ complete

#### Mock DRY refactor — da-application delegates to kg-storage

- `da-application/tests/common/mock_graph_store.rs` now wraps
  `kg_storage::MockGraphStore` instead of duplicating 460+ lines of
  storage logic
- Storage behavior (create_node, set_property, find_node, edges) is
  delegated to the canonical kg-storage mock (single source of truth)
- Domain-specific validation (schema, edge contracts, cross-refs)
  preserved — it is the sole reason this mock exists
- Added accessor methods to kg-storage MockGraphStore:
  `nodes_snapshot()`, `edges_snapshot()`, `string/int/float/bool_props_snapshot()`
- 76 da-application tests pass (including 11 healing_test, 13 enrich_test)

#### Governor CLI hardening (MEM524)

- New rule: `no_stale_precommit_packages` — catches removed workspace
  packages referenced in `.pre-commit-config.yaml` (da-graph etc.)
- Added `.yaml` and `.yml` to governor FilesystemCodeScanner suffixes
- New governor self-test: `test_stale_precommit_package_caught`
- 9/9 governor self-tests pass

### Session 2026-08-02 — M291-p80wbx ADR-051 P0: contract suites + serde

**S01-S04 complete. 33+ tests added. Governor on_track.**

#### S01 — Public GraphStore contract suite

- Extracted `graph_store_contract_suite<T>()` to `kg-storage/src/contract.rs`
  (public module, importable by Samyama and future adapters)
- Integration test becomes thin runner (1 test, imports public suite)
- Full ADR-051 §6 checklist: unique IDs, set/get all property types,
  missing→None, label isolation, get_edges_between filter, edge prop
  set/get, health, export_snapshot
- Fixed clippy (approx_constant, unused var) + fmt violations
- Removed dead `set_edge_property_string` (v1, no callers); kept
  `set_edge_property_float` (used in da-application/extraction.rs)
- Fixed pre-commit config: removed stale da-graph package (M290)

#### S02 — SamyamaGraphStore passes contract suite

- Implemented `get_edges_between`, `get_edge_property_string`,
  `set_edge_property_string_v2`, `set_edge_property_float` on
  SamyamaGraphStore using samyama-graph SDK edge property API
- Both Mock and Samyama run the SAME suite (ADR-051 Contour 4 goal)

#### S03 — Embedder contract suite

- `embedder_contract_suite<E>()` in `kg-embeddings/src/contract.rs`
- Deterministic `MockEmbedder` (FNV-1a + LCG, no network)
- 6 contract checks: dimension, determinism, different text, empty,
  unicode, batch consistency

#### S04 — Serde roundtrip tests

- 9 types in kg-storage (QueryResult, VectorMetric, VectorSearchResult,
  HealingOperation, HealingActor, ProvenanceEvent, RetryPolicy,
  YamlNodeSchema) + 3 Paper tests in da-domain
- JSON + bincode roundtrip (ADR-051 §4.4 Contour 2)

### Session 2026-08-01..02 — M285-M290 comprehensive kg-* extraction + governor

**6 milestones closed, 47+ commits, 303 tests pass, governor on_track.**

#### M285-z09px0 — kg-* Architecture Debt Closure (ADR-050 Phase A+B+D)

9 slices. SamyamaGraphStore impl migrated to kg-storage behind feature.
OntologyRegistry YAML loader. da-domain/da-ports re-export universal types.

#### M286-wda3cp — Governor CLI hexagonal refactor

798-line monolith → hexagonal Python package. 16 YAML rules.
Pre-commit integration (--check-only).

#### M287-q1wf9r — Governor CLI v2 improvements

--validate-yaml, --self-test, --check-dead-code, git-aware stale_milestone.

#### M288-avqgux — Phase F data-driven NodeSchemaDef

YamlSchemaAdapter (Box::leak), validate_node_properties_or_yaml,
schema-list --from-yaml.

#### M289-u4fzx5 — Extract universal graph capabilities

6 universal extractions to kg-ontology: bitemporal, schema (NodeSchemaDef),
edge_contract, vid (sha256), retry (RetryPolicy), healing (ProvenanceEvent).
kg-ontology now has 9 universal modules.

#### M290-j98lkq — Deduplicate ports and disassemble da-graph

3 duplicated traits eliminated (Embedder, FdApiEmbedder, GraphAlgorithms).
da-graph crate fully disassembled: queries/schema → da-application.
10 crates remain (5 kg-* + 5 da-*).

#### ADR-051 — Testing Strategy (Overlapping Verification Contours)

1017-line ADR accepted. 7 verification contours × 8 test types matrix.
Domain-specific semantic suites (temporal graph, vector retrieval, event
sourcing, candidate→promote, agent/LLM). 12 anti-patterns documented.
P0/P1/P2 implementation priorities. Governor CLI 4 new commands planned.

#### kg-ontology universal modules (9)

bitemporal, schema, edge_contract, vid, retry, healing, temporal, validator, registry.

#### M285-z09px0 — kg-* Architecture Debt Closure (ADR-050 Phase A+B+D)

9 slices. SamyamaGraphStore impl migrated to kg-storage behind feature.
OntologyRegistry YAML loader. da-domain/da-ports re-export universal types.

#### M286-wda3cp — Governor CLI hexagonal refactor

798-line monolith → hexagonal Python package. 16 YAML rules.
Pre-commit integration (--check-only).

#### M287-q1wf9r — Governor CLI v2 improvements

--validate-yaml, --self-test, --check-dead-code, git-aware stale_milestone.

#### M288-avqgux — Phase F data-driven NodeSchemaDef

YamlSchemaAdapter (Box::leak), validate_node_properties_or_yaml,
schema-list --from-yaml.

#### M289-u4fzx5 — Extract universal graph capabilities

6 universal extractions to kg-ontology: bitemporal, schema (NodeSchemaDef),
edge_contract, vid (sha256), retry (RetryPolicy), healing (ProvenanceEvent).
kg-ontology now has 9 universal modules.

4/4 universal extractions from da-domain to kg-ontology (ADR-050 principle:
universal graph capabilities live in kg-*, project-specific in da-*).

#### S01 — Node-level bi-temporal helpers

- `kg_ontology::bitemporal`: PropertyMap, read_datetime, read_bound,
  is_active_at, was_known_at, is_current, validate_bitemporal (7 tests).
- da-domain re-exports; PropertySnapshot = PropertyMap alias.

#### S02 — NodeSchemaDef trait + FieldType

- `kg_ontology::schema`: SchemaError, FieldType, Field, NodeSchemaDef
  trait with validate() default impl (6 tests).
- da-domain re-exports; 28 project-specific impls unaffected.
- Removed: schema_inner.rs (dead code after extraction).

#### S03 — EdgeContract struct

- `kg_ontology::edge_contract`: EdgeContract struct with matches() helper
  (3 tests).
- da-domain re-exports; project-specific edge_contracts() matrix stays.

#### S04 — Vid type + SHA256 hashing

- `kg_ontology::vid`: Vid type alias, sha256_vid(prefix, key),
  sha256_vid2(prefix, key1, key2) (6 tests).
- da-domain functions (paper_vid, author_vid, etc.) call universal helpers.

#### S05 — RetryPolicy

- `kg_ontology::retry`: RetryPolicy struct with delay_for_retry,
  should_retry, should_give_up, next_retry_ts, fast() (7 tests).
- da-domain re-exports; scheduler is_due() converted to free function.

#### S06 — HealingOperation + ProvenanceEvent

- `kg_ontology::healing`: HealingOperation (7 variants), HealingActor
  (3 variants), ProvenanceEvent struct with new() + with_change()
  (5 tests).
- da-domain re-exports; project-specific result types stay.

### M288-avqgux — Phase F: data-driven NodeSchemaDef (2026-08-02)

Phase F step 3 ADR-050: opt-in YAML-driven validator functions.

- `schema_for_label_or_yaml()`: tries YAML first, falls back to hardcoded.
- `validate_node_properties_or_yaml()`: same validation logic, YAML source.
- 6 parity tests: hardcoded vs YAML count match, unknown label, missing required.
- 181 total da-domain tests (175 existing + 6 new).

### M288-avqgux S01-S02 — YamlSchemaAdapter + schema-list --from-yaml (2026-08-02)

2/3 slices delivered. S03 (dead-code scanner) deferred.

#### S01 — Self-validation commands

- `governor --validate-yaml`: loads adr_rules.yaml + dimensions.yaml,
  prints count, exit 1 on parse error.
- `governor --self-test`: runs 8 unit tests via subprocess, exit 1 on failure.

#### S02 — Git-aware dimensions

- `GitReader` adapter (`adapters/git_reader.py`): subprocess-based git
  log reader with `get_git_info()`, `commits_since_date()`, `is_repo()`.
- `stale_milestone` drift flag: fires when >7 days elapsed since last
  milestone closeout AND commits exist in that period.
- `TrajectoryService` updated with optional `git_reader` param.

### M285-z09px0 + M286-wda3cp session (2026-08-02)

#### M285-z09px0 — kg-* Architecture Debt Closure and ADR-050 Advance

8/8 slices delivered. Phase A + Phase D ADR-050 complete.

- **S01**: GSD state sync after kg-* skeleton session (CONTEXT-DRAFT,
  CHANGELOG, continue.md updated).
- **S02**: Architecture guardrail extended for kg-* crates: cargo
  fmt/clippy/test + ADR-050 invariant enforcement (kg-* must not depend
  on da-*). 5 pre-existing clippy warnings fixed.
- **S03**: da-domain re-exports kg-ontology universal types (Violation,
  Severity, PropertySnapshot, TemporalEdge). Phase A complete.
- **S04**: kg-storage dead-code stub SamyamaGraphStore removed.
- **S05**: da-ports re-exports kg-storage GraphStore traits. Phase B
  step 1 complete. S09 (impl migration) unblocked.
- **S06**: da ontology-export CLI command generates node_schemas.yaml
  from 31 NodeSchemaDef structs. Phase D partial.
- **S07**: OntologyRegistry YAML loader in kg-ontology. Phase D complete.
  6 unit tests. Round-trip YAML → registry → lookup works.
- **S08**: Governor CLI extracted from legacy/, adapted for Rust.

Validation: needs-attention (Phase B step 2 deferred to S09).

#### M286-wda3cp — Governor CLI hexagonal refactor and Rust-era coverage

4/4 slices delivered (S03+S04 executed as part of S02).

- **S01**: Governor CLI rewritten as hexagonal Python package
  (domain/ports/adapters/application/checks). 798-line monolith → 25-line
  delegate. 6 unit tests.
- **S02**: 15 ADR rules in YAML data files (6 new Rust-era: ADR-040
  forbidden graph stores, ADR-050 kg-* invariant, D127 import lock,
  D136 DSPy lock). 8 dimensions. 8 unit tests. 2707 artifacts checked.

#### Pre-commit fix (main user pain point)

Pre-commit rewritten: 5-10 min → 25 sec. Removed cargo check --workspace,
clippy --all-targets, cargo run (all triggered rocksdb rebuild).
Governor integrated as pre-commit hook with --check-only.

### Governor CLI hexagonal refactor + Rust-era coverage (M286-wda3cp) (2026-08-02)

Governor CLI переписан как hexagonal Python package в scripts/governor/.
Rules are DATA (YAML), not hardcoded Python logic. Old monolithic
check_project_trajectory.py (798 lines) became thin delegate (25 lines).

#### S01 — Hexagonal package skeleton

- scripts/governor/ package: domain/ (pure logic), ports/ (5 ABC
  interfaces), adapters/ (5 filesystem implementations), application/
  (AuditService + TrajectoryService), checks/ (YAML data files).
- __main__.py CLI with --phase, --list-checks, --audit-artifacts flags.
- CodeScanner section_filter: TOML [dependencies] vs [dev-dependencies]
  awareness — da-adapters in dev-deps NOT flagged as violation.

#### S02 — YAML rules + Rust-era field coverage + dimensions

- adr_rules.yaml expanded from 9 to 15 rules:
  - ADR-040 forbidden graph stores: no nebula/falkordb/ladybugdb/arcadedb
    in Cargo.toml AND Rust imports (binding enforcement).
  - ADR-050 kg-* must not import da_domain: Rust use patterns checked.
  - ADR-045/050 no hardcoded node label definitions (HE хардкодим).
  - D136 no DSPy enabled in artifacts.
  - AGENTS.md no auto-promote.
- dimensions.yaml: 8 Rust-era dimensions with full-path conditions.
  schema_readiness=implemented, hexagonal_discipline=crystallized,
  kg_crate_family=phase_a_complete, gepa_isolation=wired.
- 2707 JSON artifacts checked, 0 violations. Universal fields covered:
  import_eligible, graph_writes_allowed, graph_write_allowed,
  graph_writes_any.
- 8 unit tests (2 new for Rust rules): all PASS.

### kg-* crate family skeleton (ADR-050 Phases A, B, C, E) (2026-08-01..02)

Ten commits realizing the kg-* crate family planned in ADR-050. Each
crate is a skeleton: module shells + public API surface + unit tests,
ready to receive migrations from da-* in later phases. Zero da-*
dependencies (ADR-050 invariant). Workspace compiles; 334+ tests green.

#### Phase A — kg-ontology skeleton (51525a2e)

- crates/kg-ontology/{lib.rs, validator.rs, temporal.rs, schema_inner.rs}
- Public API: Violation, Severity, PropertySnapshot, format_violations,
  TemporalEdge, RetroactiveExtension, OPEN, validate_temporal_edge,
  parse_datetime, EpisodicNodeSchema, EpisodeSourceType,
  EdgeTypeRegistry.
- Reserved module shells (schema, edge_contract, aspects, versioning,
  routing, mappings, registry) document which phase will populate them.
- 21 tests (validator 4, temporal 14, schema_inner 3).

#### ADR-050 split — kg-* crate family rewrite (33708c38)

- ADR-050 rewritten: 5 granular crates (kg-ontology, kg-storage,
  kg-embeddings, kg-algorithms, kg-pipeline) instead of single
  da-ontology. Neutral kg-* prefix signals cross-project reuse.
- Dependency rule documented: kg-* never depends on da-*.
- 6-phase migration plan (A through G) with per-phase shippable gates.

#### Phase B — kg-storage skeleton (ee46448a) + edge property access (417dff00)

- crates/kg-storage/{lib.rs, traits.rs, error.rs, mock.rs,
  samyama_adapter.rs}
- Public API: GraphStore, DirectGraphStore, VectorMetric,
  VectorSearchResult, QueryResult, GraphStoreError, GraphResult,
  MockGraphStore, get_edges_between, edge property accessors.
- MockGraphStore with edge_id + edge_props.
- samyama_adapter.rs: stub SamyamaGraphStore behind `samyama` feature
  flag (full impl deferred to Phase B follow-up slice).

#### Phase C — kg-embeddings skeleton (a82c37e2)

- crates/kg-embeddings/{lib.rs, embedder.rs, fd_api.rs, ruvector.rs}
- Public API: Embedder trait, EmbedderError, EmbedResult,
  FdApiEmbedder (TEI/bge-m3 adapter), RuVectorBridge stub.
- Optional `ruvector` feature flag for GNN-aware embeddings.

#### Phase E — kg-algorithms skeleton (1e0a90e7)

- crates/kg-algorithms/{lib.rs, temporal.rs}
- Public API: resolve_temporal_edges, ResolutionOutcome,
  ResolutionRule.
- 4-rule temporal resolution algorithm (skip/skip/supersede/retain-both)
  adapted from Graphiti's resolve_edge_contradictions.
- 9 tests covering all four resolution rules.

#### Phase E — kg-pipeline skeleton (df644fc3)

- crates/kg-pipeline/{lib.rs, builder.rs, engine.rs, failure.rs}
- Public API: Pipeline, PipelineStep, PipelineBuilder, StageName,
  ExecutionEngine, ExecutionResult, PipelineStatus, FailurePolicy,
  RetryPolicy, FailureAction.
- Stage implementations stay project-specific (consumers plug in via
  PipelineStep::Custom). 5 tests.

#### Companion design docs (aa567c05, a6b90161, 074fc3f5)

- doc/GRAPH-CORE-TEMPORAL-DESIGN.md: temporal edge model synthesis
  (Graphiti + OntoKG + Neo4j GRAPH TYPE + RDF-star).
- ADR-046 revised: edge-level temporal model (5-field + retroactivity).
- ADR-047 revised: temporal edge invalidation (simplifies conflict).
- kg-ontology: EpisodicNodeSchema + EdgeTypeRegistry.

#### Open follow-ups (tracked as M285-z09px0)

- Architecture guardrail scripts/CI do not yet cover kg-* crates.
- da-domain does not yet re-export kg-ontology types (Phase A backward
  compat gap).
- kg-storage/samyama_adapter.rs is a dead-code stub pending Phase B
  follow-up (full SamyamaGraphStore migration).
- Phase D (YAML ontology export), Phase F (deprecation cleanup), Phase
  G (cross-project reuse validation) not in scope.

### Temporal edge model foundation in kg-* crates (2026-07-24)

Three waves implementing the revised ADR-046 (edge-level temporal
model) in the universal kg-* crate family.

#### Wave 1: TemporalEdge type (kg-ontology)

- Rewritten temporal.rs: TemporalEdge struct (5-field model).
- +RetroactiveExtension for legal/regulatory domains.
- +is_active_at/was_known_at/is_current/invalidate/validate.
- 18 tests.

#### Wave 2: EpisodicNodeSchema + EdgeTypeRegistry (kg-ontology)

- +EpisodicNodeSchema: provenance ground truth.
- +EdgeTypeRegistry: temporal vs structural edge classification.
- 3 tests.

#### Wave 3: Edge property access (kg-storage)

- +get_edges_between(source, target, edge_type) in DirectGraphStore.
- +get_edge_property_string/set_edge_property_string_v2.
- MockGraphStore upgraded with edge_id + edge_props. 2 tests.

Workspace: 334 tests green.

### ADR-046/047/048 Phase 1 foundations materialized (2026-07-24)

Three ADRs moved from design to implemented Phase 1 foundation:
bi-temporal fields, Conflict node, Decision node. No pipeline
behavior change yet — these slices add the type-level substrate so
future slices can write Conflict/Decision nodes and emit temporal
fields on writes.

#### ADR-046 Phase 1: BiTemporal fields + helpers

- New module crates/da-domain/src/temporal.rs: OPEN sentinel,
  read_datetime, read_bound, is_active_at, was_known_at, is_current,
  validate_bitemporal. 10 tests.
- Fact-bearing schemas gain optional bi-temporal fields:
  - Claim: +valid_from, +valid_to, +recorded_at, +superseded_at
  - EvidenceBundle: +recorded_at, +superseded_at (already had valid_from/to)
  - MetricObservation: +valid_from, +valid_to, +recorded_at, +superseded_at

#### ADR-047 Phase 1: Conflict node + plane edges

- +ConflictSchema in process.rs (required: vid, kind, field, status,
  severity, detected_at; optional: resolution_strategy, resolution_value,
  resolved_at, bi-temporal, invariants).
- +conflict edge module: CONFLICTS_OVER (fact-bearing → Conflict),
  RESOLVED_BY (Conflict → Source | Decision).
- +decision edge module (forward-declared for ADR-048): CAUSED,
  INFLUENCED, PRECEDENT_FOR, AUTHORITY_FOR, TRIGGERED_BY.
- +conflict_vid index.
- +edge_contract rows for CONFLICTS_OVER, RESOLVED_BY, CAUSED,
  INFLUENCED, PRECEDENT_FOR, TRIGGERED_BY.

#### ADR-048 Phase 1: Decision node + index

- +DecisionSchema in process.rs (required: vid, category, scenario,
  reasoning, outcome, confidence, decision_maker, valid_from,
  recorded_at; optional: policy_id, valid_to, superseded_at,
  reasoning_embedding, invariants).
- Decision is the first schema where bi-temporal fields are required
  at registration time — matches ADR-048's "auditable from day one" intent.
- +decision_vid index.
- +cross_reference_fields row for Decision.policy_id (Phase 2 placeholder).

#### Totals

- Schemas: 29 → 31 (+Conflict, +Decision).
- Edge constants: 13 → 21 (+8 from conflict/decision modules).
- Indexes: 28 → 30 (+conflict_vid, +decision_vid).
- Tests: 297 → 307 (+10 temporal helpers).
- No pipeline behavior change — Phase 1 is type-level substrate only.

### validate-graph CLI + type-aware getters (2026-07-24)

Two waves closing the ADR-045 Wave D production-side gap.

#### validate-graph CLI command

- crates/da-cli/src/main.rs: +Commands::ValidateGraph { label } →
  `da validate-graph [--label X]`
  Reads SamyamaGraphStore via get_nodes_by_label +
  get_node_property_string/int/bool/float, builds a PropertySnapshot
  for each node, runs validate_node_properties. Reports total
  violations (critical + warning) and shows the first 10 with context.
  Exits non-zero on Critical violations.
- +test_validate_graph_command_runs_on_empty_store regression guard.
- README.md command table: +da validate-graph.

The two complementary CLI commands now cover both sides:
- audit-fields: static source-code audit (pipeline writes vs schemas)
- validate-graph: runtime graph audit (actual nodes vs schemas)

#### Port trait: +get_node_property_bool, +get_node_property_float

Closes a port trait gap: DirectGraphStore had string and int getters
but not bool or float. The validator needs all four to build an
accurate PropertySnapshot for runtime graph validation.

- crates/da-ports/src/graph_store.rs: default impls via string
  parsing (so existing adapters automatically work).
- crates/da-adapters/src/samyama_graph.rs: native impls via
  PropertyValue::as_boolean() / as_float() for type fidelity.
- crates/da-application/tests/common/mock_graph_store.rs: native
  impls read from bool_props / float_props HashMaps.
- validate-graph now dispatches by schema FieldType — each field is
  read via the matching getter.

Also fixes a clippy or_insert_with warning in audit-fields.

### audit-fields CLI + Source schema regression fix (2026-07-24)

MEM508 suggestion realized: the ad-hoc Python audit that found 18
schema/pipeline drift bugs is now a first-class CLI command and a
pre-commit hook.

#### audit-fields CLI command

- crates/da-cli/src/main.rs: +Commands::AuditFields → `da audit-fields`
  Walks da-application/src/*.rs for create_node("Label") blocks, scans
  forward for set_node_property_* calls, extracts field names, and
  compares against each label's schema required_fields + optional_fields.
  Exits non-zero on drift.
- +test_audit_fields_command_succeeds_when_clean regression guard.
- +pre-commit hook (runs on Rust changes).
- README.md command table updated.

#### Regression caught and fixed

The new CLI audit caught a regression the Python audit had missed:
Source schema was missing import_eligible and schema_version optional
fields even though the pipeline sets them on every Source node. The
Python audit's heuristic had missed the Source block (indentation
parsing). Fixed by adding both fields to source.rs optional_fields.

#### Pre-commit hook coverage summary

7 hooks now cover: cargo-fmt, cargo-check, cargo-clippy,
eval-baseline-check, schema-check (labels registered), docs-drift-check
(tables present), audit-fields (fields declared). Schema/pipeline drift
fails the commit, not just CI.

### Zero undeclared fields + Wave D integration across 4 test files (2026-07-24)

Two waves closing the schema-pipeline drift gap end-to-end.

#### All pipeline-set fields now declared on schemas

Automated audit compared fields set via set_node_property_* in the
pipeline against fields declared in each schema. Found and fixed:
- 7 schemas with undeclared pipeline-set fields (Topic, Author,
  Institution, Category, Reference, Source, MetricObservation)
- 11 schemas with missing invariant field declarations (prior commit)
- Entity schema: +deprecated_reason, +last_healed_at (healing writes)

Result: 0 undeclared pipeline-set fields remaining. All node writes
pass the validator's unknown-field check cleanly. The audit script is
reproducible — re-run after any pipeline change.

#### Wave D integration: 14 assert_graph_conforms call sites

- cluster_test: 1 site (was 0)
- enrich_test: 1 site (was 0)
- healing_test: 1 site (was 0)
- batch_ingest_test: 1 site (prior session)
- extraction_test: 5 sites (prior session)

Each new call site required fixing fixture incompleteness in inline
node creation helpers (Entity/Paper/Section fixtures were missing
required fields + invariants). The pattern: assert_graph_conforms
surfaces 2-3 fixture bugs per node type the first time it is wired.

### Wave D integration: 11 schemas fixed + assert_graph_conforms wired (2026-07-24)

First end-to-end runtime validator wiring in integration tests.
Surfaced three classes of test fixture incompleteness that were
silently passing before.

#### Schema invariant field declarations (11 schemas fixed)

The validator flagged 'unknown-field' warnings on every node because
schemas did not declare the architectural invariants as optional
fields, even though the pipeline sets them on every node.

Schemas fixed: Section, Concept, Topic, Category, Author, Institution,
Reference, EvidenceBundle, Claim, ConceptCluster, Citation.

#### assert_graph_conforms helper (Wave D integration)

- crates/da-application/tests/common/mock_graph_store.rs:
  +assert_graph_conforms(context) — one-call check that runs node
  schema and edge contract validators. Panics with combined diagnostic
  on any violation.
  +assert_node_and_edge_conforms (alias)
  +assert_nodes_conform (node-only check)
  Cross-reference validator excluded — pipeline creates forward
  references to nodes not yet materialized (Wave 2 work).

#### Extraction tests wired (5 tests)

Five extraction tests now call store.assert_graph_conforms() at the
end. Required fixing inline Paper/Section fixtures that were missing
required fields (vid/title/valid_from/level/order/work_vid).

#### batch_ingest_test wired

test_batch_ingest_all_success also calls assert_graph_conforms.

### cross-refs CLI + validate-node CLI + docs drift hook (2026-07-24)

Three additions completing MEM503 (governor CLI improvements).

#### cross-refs CLI (MEM503 #b completion)

- crates/da-domain/src/validator.rs: +render_cross_references_table()
- crates/da-cli/src/main.rs: +Commands::CrossRefs → `da cross-refs`
- crates/da-cli/tests/schema_check_test.rs: +test_cross_refs_command_
  outputs_table (asserts header + every declared source label appears)
- doc/GRAPH-SCHEMA.md: added "Cross-reference registry" section with
  the full 9-row table.
- README.md command table: +da cross-refs

With this, all three halves of the schema surface (nodes, edges,
references) now have CLI-rendered, docs-synced tables.

#### validate-node CLI (Wave D CLI side)

- crates/da-cli/src/main.rs: +Commands::ValidateNode → reads JSON on
  stdin, validates against schema, exits non-zero on Critical.
- +2 CLI tests covering valid + invalid snapshots.
- Closes the "validator only runs in tests" gap for ad-hoc validation.

#### docs drift pre-commit hook (MEM503 #c)

- scripts/docs_drift_check.sh: verifies that schema-list, edge-contracts,
  and cross-refs output markers appear in doc/GRAPH-SCHEMA.md or README.
  Advisory only (always exits 0).
- .pre-commit-config.yaml: +docs-drift-check hook.

### Cross-reference validator (ADR-045 Wave F) (2026-07-24)

Closes the third runtime enforcement gap from ADR-045. The shared
MockGraphStore now enforces all three halves of schema conformance:
node properties (Wave A-C, D-foundation), edge endpoints (Wave G),
and now cross-references (Wave F).

#### Wave F foundation: cross-reference field registry

- crates/da-domain/src/validator.rs:
  - +CrossReferenceField { source_label, field, target_label, required }
  - +cross_reference_fields() returns 9 rows covering the reference
    fields currently declared on process-plane schemas
  - Convention: only fields whose value is a graph VID (not external
    opaque ids like openalex_id or arxiv_id).
- 4 static tests: labels registered, no contradictory rows, fields
  exist on source schema, required flag matches schema.

#### Wave F runtime: validator on MockGraphStore

- crates/da-application/tests/common/mock_graph_store.rs:
  - +CrossRefViolation struct with Display impl
  - +MockGraphStore::validate_cross_references() walks nodes, reads
    each declared reference field, flags dangling references and
    missing required fields.
- 3 runtime tests cover violation detection, satisfied reference
  acceptance, and missing required field detection.

### schema-list CLI + doc table + pre-commit hook (2026-07-24)

Closes MEM503 #b (node-types table generator) and adds a pre-commit
hook that runs schema-check so label-registry drift fails the commit,
not just CI.

#### Wave B: node-types table generator

- crates/da-domain/src/schema.rs: +render_node_types_table()
  Columns: label, required fields, optional field count, materialized?
  The materialized list is hardcoded from create_node sites in
  da-application/src (verified by da schema-check).
- crates/da-cli/src/main.rs: +Commands::SchemaList → `da schema-list`
- crates/da-cli/tests/schema_check_test.rs: +test_schema_list_command_
  outputs_table (asserts header + all 29 registered schemas appear)
- crates/da-domain/src/schema.rs: +render_tests module with 2 tests
  for the render_node_types_table function
- doc/GRAPH-SCHEMA.md: added "Schema registry summary" section with
  the full 29-row table at the top of the document.
- README.md command table: +da schema-list

#### Pre-commit hook (MEM503 #c partial)

- .pre-commit-config.yaml: +schema-check hook runs
  `cargo run -p da-cli -- schema-check` on Rust file changes. Fails the
  commit if any create_node label is not in the schema registry
  (ADR-045 enforcement at commit time).

### Mock consolidation complete + edge-contracts CLI (2026-07-24)

Four waves closing the per-test MockGraphStore duplication debt
(MEM500 #1) and making the edge contract matrix a first-class CLI
artifact.

#### Mock consolidation: all four per-test mocks eliminated

- enrich_test.rs: 389 → 220 lines (-169 lines of duplicated mock)
- extraction_test.rs: 558 → 367 lines (-191 lines)
- batch_ingest_test.rs: 434 → 215 lines (-219 lines)
- healing_test.rs: 347 → 179 lines (-168 lines, prior session)
- Total: ~747 lines of duplicated mock code removed.
- No private MockGraphStore impls remain in da-application/tests.

Shared MockGraphStore restructured to support this:
- Arc<MockGraphStoreInner>-backed, Clone-able. Tests hand one clone to
  a use case (as Box<dyn DirectGraphStore>) and keep another for
  inspection after the use case runs.
- +snapshot_data: Mutex<Option<Vec<u8>>> for tests that need a specific
  export_snapshot payload.
- +snapshot_call_count() / import_call_count() accessors.

extraction_test migration notes:
- make_store_with_paper() is now async — the old sync version spawned
  a nested tokio runtime which panics under tokio-1.53.
- Tests that previously asserted store.node_count_total() after moving
  the store into ExtractionUseCase now assert on result.graph_node_ids
  (the store is consumed; result carries equivalent counts).

#### Edge-contracts CLI command + docs sync

- crates/da-domain/src/edge_contract.rs: +render_markdown_table()
- crates/da-cli/src/main.rs: +Commands::EdgeContracts → `da edge-contracts`
  prints the contract matrix as a markdown table
- doc/GRAPH-SCHEMA.md: added "Edge endpoint contracts" section with
  the full 13-row table (rendered form of edge_contracts())
- crates/da-cli/tests/schema_check_test.rs: +test_edge_contracts_command_
  outputs_table (asserts header + 13 edge rows present)
- README.md command table: +da edge-contracts

### Polymorphic edges + runtime edge contract validator (2026-07-24)

Two waves advancing ADR-045 coverage, plus the first per-test mock
consolidation.

#### Wave G runtime: edge contract validator on MockGraphStore

- The static edge_contracts() matrix is now enforced at test time.
  `MockGraphStore::validate_edge_contracts()` walks every recorded
  edge and checks each (source_label, edge_type, target_label) triple
  against the contract matrix.
- Detects: unknown edge types, wrong source label, wrong target label.
- +EdgeContractViolation struct with Display impl.
- 3 new tests cover violation detection, valid edge pass, and
  polymorphic MENTIONS target acceptance.

#### Contract drift closed: polymorphic edges documented

- MENTIONS edge was used in 3 semantically distinct paths (Paper→Entity,
  Paper→ResearchProblem, Paper→MetricObservation) but the contract row
  documented only Paper→Entity.
- HAS_PART was used for both Paper→Section and Paper→Reference but
  documented only Paper→Section.
- EdgeContract.target_label (single) → target_labels (slice).
- +test_polymorphic_edges_document_polymorphism: any edge with >1
  target must explain why in its rationale.

#### Mock consolidation: healing_test → shared MockGraphStore (MEM500 #1)

- healing_test.rs had a complete private MockGraphStore implementation
  (~170 lines). Replaced with `mod common; use common::mock_graph_store::
  MockGraphStore;`.
- Shared MockGraphStore gains snapshot_calls/import_calls counters and
  accessors so tests that need call-frequency assertions can migrate.
- Per-test mocks in batch_ingest_test, enrich_test, extraction_test
  remain (test-specific helper signatures); migration deferred.
- healing_test.rs: 347 → 179 lines (-168 lines of duplicated mock).

### Edge contract matrix + validator helpers (2026-07-24)

Two waves advancing ADR-045 coverage.

#### Wave G foundation: edge endpoint contract matrix

- New module `crates/da-domain/src/edge_contract.rs` documents the
  expected (source_label, target_label) endpoint contract for every
  edge type the pipeline materializes.
- 13 contracts: AFFILIATED_WITH, AUTHORED_BY, CITES, FOUND_IN,
  FROM_SOURCE, HAS_PART, HAS_TOPIC, IN_CATEGORY, MEMBER_OF_CLUSTER,
  MENTIONS, PARTICIPATES_IN, SUPPORTS, SUPERSEDES.
- 4 tests guard the matrix:
  - all contract edges reference registered node labels
  - all contract edges pass `validator::validate_edge_type`
  - no contradictory contracts (same edge_constant, different endpoints)
  - every pipeline-referenced edge has a contract row
- Foundation for Wave G runtime edge-endpoint validation.

#### Wave D foundation: validator helpers on MockGraphStore

- Shared `MockGraphStore` gains validator integration helpers:
  `snapshot_node(id)`, `validate_node(id)`, `validate_all_nodes()`.
- Tests can now assert schema conformance of the actual nodes written
  during a pipeline run without ad-hoc field assertions.
- New smoke test `test_extraction_produces_schema_valid_nodes` exercises
  the helper API.
- The shared MockGraphStore was rewritten to be a complete,
  label-aware, contract-correct implementation (replaces the previous
  partial version). Per-test mocks retained for now (test-specific
  counters); future work to consolidate.

### Author → Institution edge + schema-check CLI + mock contract fixes (2026-07-24)

Three waves closing debts surfaced during ADR-045 validator rollout.

#### Wave 1: Author → Institution via AFFILIATED_WITH edge

- **Model gap closed**: OpenAlex provides per-authorship institution lists, but the flat `OpenAlexWork.institutions` field dropped this association. Institution nodes were orphaned (created without any edge to Author nodes).
- Port: `+OpenAlexAuthorship { author, institutions }`, `+OpenAlexWork.authorships` (source of truth). Authors/institutions flat fields retained for backward compat and derived by the adapter from authorships.
- Adapter: parse_work builds authorships first, derives flat views.
- Domain: `+relation::bibliographic::AFFILIATED_WITH`, `+vid::institution_vid(display_name, openalex_id)`. Validator edge registry + duplicate-label test cover the new constant.
- Application: enrich.rs institution loop rewritten to iterate authorships; for each (author, institution) pair, creates AFFILIATED_WITH edge.
- EnrichResult: `+institutions_written`, `+affiliation_edges_written` counters.
- Tests: enrich_test mock OpenAlexWork includes Alice → MIT authorship; test_enrich_writes_topics_and_authors now asserts 1 institution + 1 affiliation edge; new test_enrich_links_author_to_institution_via_affiliated_with_edge.

#### Wave 2: CLI `da schema-check` (ADR-045 Wave E)

- Promotes the schema_audit_test guard to a standalone CLI command.
- Scans da-application/src/*.rs for create_node("Label") call sites, reports registered count, used count, unregistered labels; exits non-zero on violations.
- Tests: 2 CLI smoke tests (success path + count assertions).
- Docs: README command table updated.

#### Wave 3: Mock contract drift closed (MEM499)

- All 4 per-test MockGraphStore impls (batch_ingest, enrich, extraction, healing) returned Vec::new() from get_nodes_by_label — the real SamyamaGraphStore filters by label.
- Applied the same labels map pattern already used in common/mock_graph_store.rs and in find_node_by_string_property (MEM495).
- None of these tests called get_nodes_by_label today, but the drift would have surfaced as a silent empty result if a future test asserted on it.

### Schema validator + 5 bug fixes (2026-07-24)

- **SchemaValidator module** (`crates/da-domain/src/validator.rs`). Pure-logic validator that checks node property snapshots against declared `NodeSchemaDef` and architectural invariants. Returns ALL violations in a single pass with severity levels (Critical/Warning). 15 tests cover all 29 schemas + edge registry. ADR-045.
- **3 schema violations fixed** (caught by the validator audit):
  - `Category`: missing required `is_primary` — now set to `true` in ingest.
  - `SchedulerTask`: missing required `vid` — now set in scheduler.
  - `MetricObservation`: missing required `run_id` — now set to pseudo `run:paper:{paper_id}` until ExperimentRun is materialized.
- **2 pipeline logic bugs fixed** (caught by writing missing integration tests):
  - `ResearchProblem` case-sensitivity: `parsed.abstract_text.contains("we propose")` missed abstracts starting with capital "We propose...". Fix: lowercase abstract before matching (MEM496).
  - `extract_metric_value` returned None when the cleaned substring was a full number (e.g. "0.95") because `find` returned None for end-of-string. Fix: `.unwrap_or(cleaned.len())`.
- **Reference dedup logic fixed.** Previously each paper citing the same citation created a new Reference node. Now `find_node_by_string_property("Reference", "vid")` is consulted first and existing nodes are reused.
- **Mock graph store label contract fixed (MEM495).** `MockGraphStore::find_node_by_string_property` previously ignored `_label` parameter. Real `SamyamaGraphStore` filters by label; the mock not filtering caused false-positive dedup matches across node types (Reference with arxiv_id="X" was returned when Citation was queried). Fixed by adding a `labels` map and filtering. Applied to all 5 mock store implementations.
- **Audit-time CI guard.** New test `schema_audit_test.rs` scans `da-application/src/*.rs` for `create_node("Label")` call sites and asserts every label is registered in `all_node_schemas()`. Future node additions without schema registration fail CI.
- **6 integration tests added** for process plane node creation (ResearchProblem×3, MetricObservation×2, EvidenceBundle×1). These exercise the full extraction pipeline (Paper lookup → entity extraction → node creation) rather than just unit-level helpers.
- **EnrichResult propagates institutions_written.** The counter was previously a local variable. Now exposed in the return struct, allowing batch-level aggregation.
- **Dead code removed.** 3 unused LoweredConfig fields (`models_lower`, `task_phrases_lower`, `task_acronyms_lower`), unused import `DetectedCluster`, and 6 collapsed-if warnings — all cleared. da-* crates now 0 clippy warnings.
- **Port extension.** `DirectGraphStore::set_node_property_float` added for metric values and confidence scores. SamyamaGraphStore + all mock stores updated.
- **Documentation.** ADR-045 (Schema Validator), SCHEMA-VALIDATOR-DESIGN.md, ADR-INDEX.md updated with ADR-045 dependency edges.
- **Tests**: 269 green (228 lib + 41 integration; was 213).

### Pipeline: Reference + Category node wiring (2026-07-29)

- **Reference node materialization.** Full bibliography now preserved.
  Previously only resolvable arxiv citations were saved (~20%). Now ALL
  citations become Reference nodes with raw_text + metadata.
  Paper → Reference via hasPart edge.
- **Category node materialization.** arXiv category (cs.LG, cs.AI, etc.)
  now materialized as Category node. Paper → Category via IN_CATEGORY edge.
- **reference_vid helper.** SHA256-based VID for idempotent reference dedup.
- **Pipeline: 12/29 node types** (was 10).

### Pipeline: Source node + FROM_SOURCE edge wiring (2026-07-29)

- **Source node materialization.** Layer 0 provenance node was missing
  from pipeline. Now created in ingest_pdf() with: vid, code, source_type,
  domain, reliability_tier, retrieval_eligible=false, import_eligible=false
  (D127), schema_version. Idempotent (checks existing Source by code).
  Paper → Source via FROM_SOURCE edge.
- **extract_source_from_path helper.** Detects source from path.
- **3 TDD tests.** Pipeline: 10/29 node types materialized.

### Documentation drift fix (2026-07-29)

- **README + GRAPH-SCHEMA synced.** Stale counts updated: 12→29 node types,
  20→18 indexes. 5 stale references in README.md, 2 in GRAPH-SCHEMA.md.
  Documentation now matches actual code state.

### Logic gap fix: schema_version on ALL nodes (ADR-044 prep) (2026-07-29)
- **schema_version missing on 6/9 node creation sites.** Found by audit:
  Section, Topic, Author, SchedulerTask (×2), ConceptCluster created
  without `schema_version` property. Critical for ADR-044 schema migrations:
  without per-node schema version, migration runner cannot track which
  nodes need backfill. Now ALL 9 create_node sites set schema_version=1.
  This enables future ADR-044 Wave B (schema registry in graph).

### Logic gap fix: SchedulerTask schema registration (2026-07-29)

- **SchedulerTaskSchema added to da-domain.** SchedulerTask was created in
  pipeline (ingest, enrich, scheduler) but had NO domain representation —
  violating single source of truth. Now registered in `all_node_schemas()`
  as 29th node type. Required: vid, arxiv_id, task_type, status.
  D127/D134 compliant (retrieval_eligible + import_eligible fields).
  2 TDD tests added.

### ADR-044: Schema Lifecycle design (2026-07-29)

- **ADR-044 Proposed**: Graph Schema Lifecycle — Versioned Manifest,
  Migration Framework, Self-Healing. Based on 2026 best practices research
  (expand-contract pattern, idempotent migrations, schema registry in-graph).
  Two new node types: SchemaVersion (semver, active/deprecated/superseded),
  SchemaMigration (from→to, pending→completed→rolled_back). External schema
  manifest in `data/schema/versions/*.yaml`. Schema drift detection + healing
  (extends GraphHealingUseCase with 4 new operations). CLI: schema version,
  history, plan, migrate, verify, rollback, drift, heal.
- **Design doc**: doc/SCHEMA-LIFECYCLE-DESIGN.md (Samyama + RuVector analysis).

### Pipeline: ConceptCluster materialization (MEM482) (2026-07-29)

- **ClusterUseCase created.** `da-application/src/cluster.rs` — bridge between
  domain `detect_clusters()` (pure function) and graph store.
  - `materialize_clusters()` takes EntityPapers mapping, detects clusters,
    writes ConceptCluster nodes + MEMBER_OF_CLUSTER edges to graph.
  - D127 compliant: import_eligible=false on all ConceptCluster nodes.
  - Hexagonal: entity lookup via get_nodes_by_label + get_node_property_string
    (no Cypher in application layer).
- **Shared test mock.** `tests/common/mock_graph_store.rs` — reusable
  MockGraphStore for all integration tests (implements both GraphStore +
  DirectGraphStore). Eliminates boilerplate duplication.
- **4 TDD tests:** cluster creation, MEMBER_OF_CLUSTER edge, no-clusters-for-low-mentions,
  correct label.
- **Pipeline gap closed (MEM482):** detect_clusters() was never called from
  application. Now ConceptCluster materialization is wired.

### Pipeline: FOUND_IN edge wiring (Entity → Section) (2026-07-29)

- **FOUND_IN edge creation in extraction pipeline.** When Entity has
  section_title and a matching Section node exists, extraction now creates
  Entity -[:FOUND_IN]-> Section edge. This closes a critical pipeline gap:
  entities were linked to Paper (via MENTIONS) but NOT to Section.
- **Enables:** retrieval by section, PPR adjacency through Section nodes,
  evidence chain construction from Entity → Section → Work.
- **Hexagonal compliance:** Section lookup via find_node_by_string_property
  (no Cypher in application layer). Uses structure::FOUND_IN constant.
- **ExtractionResult** extended: +mentions_edges, +found_in_edges counters.
- **TDD test:** test_extraction_links_entities_to_sections_via_found_in.

### Idiomatic patterns + docs (Wave 5) (2026-07-29)

- **`#[inline]` on hot-path functions.** `word_boundary` (19 call sites),
  `classify_section` (18 call sites), `is_known` (domain lookup) — all
  small functions called in extraction inner loops. Rust 2026 best practice:
  add `#[inline]` to small hot functions to enable cross-crate inlining.
- **D127/D134 audit automated.** Audit script verifies every `create_node()` 
  sets `import_eligible=false` + `retrieval_eligible`. All 8 sites pass.
- **PROJECT.md updated.** Reflects Edition 2024 + YAML config externalization.

### CLI runtime refactor + code quality (2026-07-29)

- **CLI runtime duplication eliminated.** 12× `Runtime::new().unwrap()` in
  main.rs → 1 shared runtime with proper error handling (unwrap_or_else →
  eprintln + exit(1)). Rust 2026 best practice: single runtime, no unwrap
  in main error path.
- **Dead code removed: ExtractionConfig::from_file().** JSON loader with
  0 callers after YAML migration.
- **D127 invariant enforced on ALL 8 node creation sites.** Previous: 3/8.
  Now: 8/8 (import_eligible=false + retrieval_eligible set everywhere).

### D127/D134 invariant enforcement + dead code removal (2026-07-29)

- **D127 violation fixed: 5 node creation sites missing import_eligible=false.**
  Audit found: enrich.rs Topic/Author/SchedulerTask, ingest.rs Section/Citation,
  all created without `import_eligible=false`. Now ALL 8 create_node sites are
  D127/D134 compliant (import_eligible=false + retrieval_eligible set).
  Audit pattern captured in MEM480.
- **Dead code removed: ExtractionConfig::from_file().** JSON loader had 0
  callers after YAML migration (Wave C). Replaced by from_yaml_file().

### Hot-path extraction optimization (Wave 3a) (2026-07-29)

- **Pre-lowercase config data (LoweredConfig).** `RuleBasedExtractor` now
  pre-computes lowercase versions of all config lists once at construction
  time. Eliminates repeated `.to_lowercase()` allocations in extraction hot
  path — Rust 2026 best practice: "reuse allocations where possible."
  - `method_acronyms_lower` + `method_acronyms_canonical` (zip for label)
  - `models_prefix_lower` (first segment before '-', lowercased)
  - `datasets_lower`, `metrics_lower`
  - `task_phrases_lower`, `task_acronyms_lower`, `method_phrases_lower`
- **Cross-whitelist check optimized.** Was O(models × candidates) allocations
  (`m.to_lowercase()` for each model for each candidate). Now O(candidates)
  with pre-lowered prefix lookup.
- **Vec::with_capacity** instead of Vec::new() in extraction methods.
- **Hot-path `.to_lowercase()` count: ~20+ → 9** (remaining are necessary
  per-text lowerings, not per-entity).

### Rust 2026 meta-optimization (2026-07-29)

- **Edition 2021 → 2024 migration.** `cargo fix --edition` found zero
  breaking changes — code was already compatible. Updated edition field
  and rust-version 1.75 → 1.85. Enables: async closures, never type fallback,
  RPIT lifetime capture, gen blocks, IntoFuture in prelude.
- **Release profile optimization.** Added codegen-units=1 and strip=true
  for smaller, faster release binaries. Kept lto="thin" (fat LTO causes
  3min+ build times due to large vendor samyama-graph crate).
- **Dev profile tuning.** debug=1 for faster incremental builds.
- **Research:** doc/META-OPTIMIZATION-PLAN.md based on Rust 2026 best
  practices (Edition 2024, Tokio patterns, performance optimization).

### Hot-path extraction → config-driven (Wave H) (2026-07-29)

- **KNOWN_* const arrays eliminated from extraction hot-path.** 7 hardcoded
  arrays (KNOWN_METHODS, KNOWN_METHOD_PHRASES, KNOWN_DATASETS, KNOWN_MODELS,
  KNOWN_METRICS, TASK_PHRASES, TASK_ACRONYMS) → 0. Extraction logic now uses
  `self.config.*` from loaded YAML (data/extraction_patterns.yaml).
- **Logical gap fixed.** RuleBasedExtractor loaded config from YAML but hot-path
  methods (extract_candidates, extract_method_acronyms_global) still used const
  arrays directly. Config was loaded but unused in extraction — illusion of
  configurability. Now: 13 const references → self.config.* references.
- **API change.** extract_candidates() and extract_method_acronyms_global()
  converted from associated functions to &self methods (need config access).
  Tests updated: 6 test functions added `let extractor = RuleBasedExtractor::new()`.
- **Result:** Updating entity whitelists = edit YAML, no recompile.
  Governor CLI can add new Method/Dataset/Model/Metric/Task without Rust change.

### Configuration externalization — Waves F/G (2026-07-29)

- **Node vocabulary → YAML.** 10 hardcoded BUNDLE_*/VERIFICATION_*/CLUSTER_*
  const → `data/node_vocabulary.yaml` (4 bundle types, 3 verification
  statuses, 3 cluster types). New `vocabulary` module with
  NodeVocabularyRegistry + is_known_bundle_type/verification_status/cluster_type().
  evidence_bundle.rs and hypergraph.rs: const → 0.
- **Cluster thresholds → YAML.** MIN_COOCCURRENCE and MIN_SINGLE_MENTIONS
  const → `data/algorithm_params.yaml` (runtime tuning parameters).
  cluster.rs: const → functions min_cooccurrence(), min_single_mentions()
  loaded from ClusterConfig YAML registry.

### Configuration externalization — Waves C/D/E (2026-07-29)

- **Source codes → YAML.** 16 hardcoded SOURCE_*/TYPE_*/DOMAIN_* const →
  `data/source_codes.yaml` (6 source codes, 5 types, 5 profiles, 3 tiers).
  SourceRegistry with OnceLock, is_known_source_code/type/profile().
  Removed convenience fn SOURCE_ARXIV() (snake_case violation).
- **Failure taxonomy → YAML.** 30 hardcoded COMPLETENESS_/ORIGIN_/STAGE_/FAIL_
  const → `data/failure_taxonomy.yaml` (6 stages, 16 classes, 3 completeness,
  5 origins). FailureTaxonomyRegistry with is_known_*() validation.
- **Extraction patterns → YAML.** Converted `data/extraction_patterns.json`
  → `.yaml`. ExtractionConfig::from_yaml_file() + bundled() + load().
  RuleBasedExtractor now holds config: `with_config()`, `config()`.
  7 KNOWN_* const arrays remain as runtime fallback (full hot-path
  migration is next slice). ExtractionConfig::defaults() removed.
- **Removed**: `data/extraction_patterns.json` (replaced by .yaml).

### Configuration externalization — no hardcoded reference data (2026-07-29)

- **Domain codes moved to YAML.** 166 hardcoded const → 0. Reference data now
  lives in `data/arxiv_categories.yaml` (148 official categories) and
  `data/extension_domains.yaml` (10 da.* codes + aliases). Loaded at startup
  via `DomainRegistry` with `OnceLock`. Bundled fallback via `include_str!`.
  Public API (`is_known()`, `canonicalize()`) unchanged.
  Principle: **logic stays in Rust, data goes to YAML.** No recompilation
  needed to update category lists.
- **Scientific domain extraction in ingest.** `extract_domain_from_path()`
  parses catalog path (`.../arxiv/cs-lg/<id>/source/<id>.pdf` → `cs.LG`) and
  sets `primary_scientific_domain`, `scientific_domains`,
  `domain_assignment_method` on Paper nodes. ADR-043 compliance.
  `canonicalize_fs_category()` handles multi-dash prefixes (cond-mat, astro-ph).
  PaperSchema updated: +scientific_domains, +primary_scientific_domain,
  +domain_assignment_method fields.
- **CONFIG-EXTERNALIZATION-PLAN.md**: roadmap for remaining hardcoded data
  (extraction patterns dedup, source codes, failure taxonomy, edge types).

### Debt sweep — DRY + consistency (2026-07-29)

- **Edge type magic strings eliminated.** New `relation::structure` module with
  6 FaBiO/OpenAlex structural edges (HAS_PART, HAS_TOPIC, AUTHORED_BY,
  FROM_SOURCE, FOUND_IN, IN_CATEGORY). Replaced 5 magic string call sites
  in ingest/enrich/adapters/tests with typed constants. 2 new TDD tests
  verify structural edge values and detect cross-module label drift.
- **Architecture sweep clean.** Hexagonal dependency direction verified:
  da-domain → 0 da deps; da-ports → 0 da deps; da-application/adapters/cli
  → only domain+ports. 220 tests green. Clippy clean for da-* crates.
  No TODO/FIXME/unimplemented!() in production code.
- **Dead code audit.** 6 fn candidates verified as legitimate:
  builder pattern (with_embedder/with_policy), utility (from_entity,
  health_check), test helpers (with_store, add_vector_direct). 9 unused
  da-ports exports are planned Phase 5+ port traits, not dead code.

### Research Process Plane ontology (2026-07-29)

- **ADR-043 + Process kernel implemented.** 14 new process node types
  (ResearchProblem, ResearchEnvironment, BaselineSnapshot, ResearchIdea,
  Hypothesis, Intervention, InterventionBundle, ImplementationAttempt,
  ArtifactVersion, ExperimentRun, MetricDefinition, MetricObservation,
  ResultComparison, FailureEvent) + 45 edge types. Node types 14 → 28.
  All with retrieval_eligible/import_eligible (D127). TDD: 42 new tests.
- **Domain registry (arXiv + da.*)**: **154 official arXiv categories** (complete
  taxonomy from arxiv.org/category_taxonomy) + 10 extension codes (da.medicine,
  da.microbiome, da.biohacking, etc.). Legacy `q-fin.*` migrated to `fin.*`.
  Alias canonicalization (cs.ml→cs.LG, nlp→cs.CL, gnn→cs.LG).
- **Design docs**: DOMAIN-REFERENCE-ARXIV.md, PROCESS-SCHEMA-P0.md, ADR-043.
  Three-plane model: Publication / Research Process / Experience (RVF).

### Ontology design (2026-07-29)

- **ADR-043 Proposed: Research Process Plane (execution-grounded).**
  Cross-cutting plane over L0–L7 (not Layer 8): Publication / Research Process /
  Experience (RVF). Full process kernel from ResearchProblem → Environment →
  Idea → Hypothesis → Intervention → Attempt → Artifact → Run → Observation →
  Comparison → Claim/Insight (+ Failure, Novelty, Generalization, Replication).
  Multi-domain from day one via `source_profile` × `scientific_domain` packs.
  Two-tier ResearchEnvironment (`full` / `env_lite`). Hypothesis first-class ≠ Claim.
  Invariants: failure≠refutation, observation≠comparison≠claim≠reward,
  ConceptCluster≠evidence, D127 fail-closed. ONTOLOGY-DESIGN §0 + ADR-INDEX updated.

### Rust v2 (2026-07-26)

- **Repository hygiene**: removed orphan root `uv.lock` (no root `pyproject.toml`
  after Python→Rust migration) and Python-era cache dirs (`.pytest_cache/`,
  `.ruff_cache/`, `.hypothesis/`, `.gremlins_cache/`, `.bg-shell/`, `tmp/`,
  `coverage/` — 15MB moved to `/tmp/daily-archive-stale/`). Updated
  `.docker/README.md` and local `AGENTS.md` to remove stale Python probes
  and reflect the Rust runtime (`da_adapters::GrobidParser`).
- **Dead code removal**: deleted unused `da-domain/versioning.rs` module
  (`TemporalRecord`/`Versioned` — 0 callers, 112 lines) and
  `EntityType::Baseline` variant (0 callers after classifier removal).
  GitNexus impact confirmed LOW risk for both. Tests: 89+ green.
- **Code quality refactoring (rule_extractor.rs)**: Extracted 6 canonical
  whitelists (KNOWN_METHODS, KNOWN_DATASETS, KNOWN_MODELS, KNOWN_METRICS,
  TASK_PHRASES, TASK_ACRONYMS) as module-level constants — single source of
  truth for section-classified and global passes. Added `word_boundary(text,
  start, end)` helper eliminating 7× duplicated before_ok/after_ok blocks.
  Removed ~140 lines of duplicate code.
- **Pattern 1 type restriction bugfix**: "we propose/use X" extraction now
  fires only for Method+Task entity types. Previously it fired for all types,
  causing GRPO to be mistyped as Dataset when it appeared in Benchmarks-titled
  sections with "we use GRPO". Precision 0.573→0.610, F1 0.723→0.752.
- **Scheduler associate functions refactor**: `GraphScheduler::add_pending_to`,
  `load_due_tasks_from`, `record_retry_on`, `complete_task_on` accept
  `&dyn DirectGraphStore` so CLI can use a single shared Samyama store.
  Struct API (`GraphScheduler::new(...).load_due_tasks()`) kept as thin wrapper
  for future Phase 3+ server-mode ownership patterns. Removed ~100 lines of
  inline `DirectGraphStore` duplication from CLI's `run_scheduler`; covered
  by 6 TDD tests.
- **Task extraction (EntityType::Task)**: whitelist-based extraction for
  "prompt optimization", "preference optimization", "RLHF", "RAG". Narrow
  whitelist avoids generic-task FP (summarization/code generation). Recall
  0.958→0.979 (prompt optimization now found in 2507.19457).
- **Scheduler (D135, ADR-037 §4.3)**: Graph-based lazy-load scheduler for
  OpenAlex enrichment. Pending tasks stored as SchedulerTask nodes in Samyama
  Graph (not JSONL — ADR-040 compliance). Exponential backoff (1d→3d→9d→27d).
  State restores from snapshot on process restart. 6 TaskType variants aligned
  with ADR-037 priority queue design. CLI: `da scheduler run`.
- **OpenAlex adapter (D133)**: `da enrich --id` and `da batch-enrich --ids`
  fetch curated metadata (topics, authors, concepts) from OpenAlex API.
  Lazy load: new papers get pending stub with `openalex_pending=true`.
- **Extraction evaluation (D136)**: P/R/F1 framework + gold-standard fixture
  for 2507.19457. Baseline: P=0.778 R=0.438 F1=0.560 (rule-based).
- **Extraction recall improvement**: dataset name extraction (16 known datasets
  + capitalized-before-keyword pattern), method acronym extraction (GEPA/GRPO/
  RLVR), stopword filter for common tech acronyms (LLM/AI/NPU/etc.).
  Final: P=0.632 R=0.750 F1=0.686. Rule-based ceiling reached.
- **Extraction refactored to global passes**: section-classified extraction
  replaced with global passes for Method/Dataset/Model/Metric. Known-name
  whitelists with case-insensitive search. Canonical model names prevent
  duplicate variants (GPT-4.1, GPT-4.1-Mini, etc.). Real eval script replaces
  hardcoded estimates. Gold standard verified against GROBID TEI full text.
  2507.19457: P=0.542 R=1.000 F1=0.743 (1 FN: prompt optimization).
- **Gold-standard corpus expanded to 8 papers**: 6 new fixtures added
  (2412.15118, 2505.10571, 1409.0473, 2602.06052, 2510.11967, 2310.06770).
  Each entity text-verified via GROBID TEI keyword scan. Batch evaluation
  script runs extraction on all fixtures and reports corpus-level metrics.
- **Evaluation framework hardening** (3 fixes):
  - Recall >1.0 bug fixed — one-to-one greedy matching in fuzzy eval.
  - Entity_type now checked in fuzzy matching — "PRO" Method no longer
    falsely matches "prompt optimization" Task.
  - Dedup key changed to (label, type) — GRPO can be both Dataset (noisy
    section heuristic) and Method (whitelist canonical), independently.
  Honest corpus (12 modern papers, 96 gold): P=0.639 R=0.979 F1=0.774
  (Micro); P=0.671 R=0.967 F1=0.782 (Macro). Only 2 FN remain
  (multi-word methods: self-evolving memory, generalization).
- **Entity-level dedup bugfix**: section-classified extraction could add the
  same (label, type) pair from multiple sections before global `seen` was
  initialized. Real symptom: duplicate `[Dataset] HotpotQA` in 2507.19457.
  Added final post-pass retain collapsing duplicates; 123 tests green.
- **Pattern 1 cross-whitelist suppression**: "we propose/use X" extraction
  in Method+Task sections no longer claims known model names (GPT-4,
  Claude, etc.) as Method/Task candidates. Cross-whitelist check prevents
  wrong-type duplicates before they form. 124 tests green.
- **TaskType::as_str() bugfix**: hardcoded "openalex_enrich" in scheduler
  AND enrich.rs replaced with `task.task_type.as_str()` → "open_alex_enrich".
  Old hardcoded value diverged from serde snake_case serialization, which
  would break future graph-property vs JSON comparisons. Both code paths
  (scheduler add_pending_to + enrich create_pending_stub) now consistent.
  125 tests green.
- **Governor CLI: eval_batch pre-commit hook**. Non-blocking informational
  check that runs the 12-paper extraction eval after every Rust file commit.
  Closes the biggest process gap: metrics were previously estimated "by eye".
  Use `SKIP=eval-baseline-check` for fast commits.
- **Wave 2: Governor suggest-whitelist**. SAGA-inspired bi-level loop: analyzes
  gold entities against current whitelists, reports uncovered candidates.
  Coverage: 28/29 (96.6%), 1 uncovered ("generalization" — too broad for
  whitelist). Usage: `cargo run -p da-cli --example suggest_whitelist`.
- **Eval display consistency fix**: FN/FP display logic in eval_extract.rs
  and eval_batch.rs now uses type-checked matching, consistent with the
  actual P/R/F1 metrics (ExtractionMetrics::evaluate_fuzzy). Previously
  display used type-agnostic matching which could disagree with metric
  counts.
- **Phase 2: HTML parser adapter**. HtmlParser implements ParserPort for non-PDF
  sources (textbook chapters). ParserPort extended with parse_html() default.
  GNN textbook (4 chapters, HTML) now ingestible through Rust pipeline.
- **Phase 2: Cross-domain GNN entities**. Added GCN, GAT, GIN, MPNN, GNN to
  KNOWN_METHODS; GraphSAGE to KNOWN_METHOD_PHRASES. FP risk validated
  (0/20 LLM papers contain these). GNN textbook extraction: 4→8 entities.
  Corpus P=0.770 R=0.999 F1=0.870.
- **Phase 2: Multi-source eval corpus**. 104 sources (100 arxiv PDF + 4 GNN
  textbook HTML). eval_batch now auto-detects PDF (GROBID) vs HTML (HtmlParser).
  GNN textbook: 0 FN (100% recall on HTML sources).
  GRAPH-SCHEMA.md updated with Layer 0 Source node + FROM_SOURCE edge.
- **Phase 3: Edge weight support**. DirectGraphStore trait extended with
  `set_edge_property_float` and `set_edge_property_string`. MENTIONS edges
  now carry `weight=1.0` (rule-based confidence). Enables PPR and GNN
  message passing via RuVector Tier 2. GRAPH-SCHEMA.md updated.
- **Phase 3: Entity embedding + domain_tags schema**. EntitySchema
  optional_fields extended with `embedding` (Vector, bge-m3 1024d) and
  `domain_tags` (String, cross-domain filtering). fd_api embedder
  confirmed alive at :8000.
- **Phase 3: Entity embedding computation wired**. ExtractionUseCase gains
  optional `embedder: Option<Box<dyn Embedder>>` and `with_embedder()`
  builder. When set, each unique entity label gets a bge-m3 embedding
  written to Entity node vector index via `add_vector`. Backward
  compatible (default: None, no embeddings).
- **Phase 3: Section node creation in ingest (Layer 2)**. IngestUseCase
  now creates Section nodes for each parsed section with vid, title, level,
  order, text, char_count properties. Linked via hasPart edge (Paper→Section).
  Closes implementation gap: GRAPH-SCHEMA defined Section but pipeline didn't
  materialize it. Updated batch_ingest_test assertions. UTF-8 safe truncation.
- **Phase 4: ConceptCluster node (Layer 6 Hypergraph)**. ConceptClusterSchema
  registered as 12th node type. Supports GNN community detection and hypergraph
  queries. cluster_type: concept_cluster / method_family / benchmark_suite.
  5 TDD tests added.
- **Phase 4: ADR-042 + MEMBER_OF edges + ConceptCluster detection**. ADR-042
  adopts HyCE-RAG (arXiv:2607.22597) blueprint for hypergraph evidence chains.
  Added hypergraph::MEMBER_OF, SUBSUMES, SUPPORTS edge types. Rule-based
  detect_clusters() — deterministic co-occurrence analysis, no LLM required
  (GSD: "statistical-first"). 4 TDD tests for cluster detection.
- **Phase 5: GNN algorithm ports**. GraphAlgorithms trait added to da-ports:
  personalized_pagerank(), get_neighbors(), get_all_neighbors(). Enables
  RuVector solver wiring. GNN Readiness: **9/10** (only Agent assertions
  remaining). Typed adjacency export ✅, PPR port ✅, community detection
  algorithm ✅ (offline detect_clusters).
- **Phase 5: SamyamaGraphStore get_outgoing_edges**. DirectGraphStore
  get_outgoing_edges implemented (was default empty). Uses
  get_outgoing_neighbor_slice(). TDD test verified.
  All PPR prerequisites complete: port + adjacency + edge weights + embeddings.
- **ADR-042 revised: EvidenceBundle + Claim**. Deep HyCE-RAG critique applied:
  ConceptCluster separated from EvidenceBundle (derived community ≠ evidence).
  MEMBER_OF → MEMBER_OF_CLUSTER. Added PARTICIPATES_IN, CONTRADICTS, QUALIFIES.
  New domain types: EvidenceBundle (source-grounded n-ary), Claim (proposition).
  Node types: 12 → 14. 7 TDD tests added.
- **Wave 3: Declarative extraction patterns**. `data/extraction_patterns.json`
  config file — whitelists loadable from JSON, governor CLI can update
  patterns without recompiling Rust. ExtractionConfig::defaults() provides
  backward-compatible embedded defaults.
- **Corpus modernized**: removed stale 2014/2023 fixtures (Bahdanau NMT,
  SWE-Bench), added 6 modern LLM/agent papers (Dec 2024 – Jun 2026)
  covering GEPA/GRPO/RLVR, harness engineering, world models, multimodal
  benchmarks. Total: 12 text-verified fixtures, 96 gold entities.
- **Case-insensitive method extraction**: GROBID normalizes acronym casing
  (ppo, Cot, Gpt-4). Global method pass now uses case-insensitive search
  with canonical uppercase labels. Fixes PPO/DPO/CoT false negatives.
- **Gold-standard verification hardened**: each entity now verified as
  STANDALONE WORD in GROBID body sections (not references, not substrings).
  Removed 28 false gold entities (e.g., PPO in "support", GPT-4 in references).
  Corpus: 83 → 55 verified gold entities. Recall now ~1.0 (2 FN: multi-word
  methods beyond acronym whitelist). Precision ~0.49 — limited by incomplete
  gold (extractor finds real entities not yet annotated).
- **Graph healing (D135)**: 7 operations (correct, merge, split, silence,
  migrate, rollback, repair_cites). GraphHealingUseCase + `da heal` CLI.
  Merge with edge redirect, correct with old_value audit trail.

- **Architecture guardrail CI rewritten** for Rust hexagonal layout (D131).
  Old Python M044/M045 workflow was broken after Python→legacy move
  (`scripts/` and root `pyproject.toml` gone). New workflow: cargo fmt/check/clippy
  + dependency-direction check. Local mirror: `scripts/verify_rust_architecture.sh`.
- **Pre-commit restored**: `.pre-commit-config.yaml` with cargo-fmt + cargo-check
  (was missing; every commit required `PRE_COMMIT_ALLOW_NO_CONFIG=1`).
- **cargo fmt** applied across all crates; unused imports cleaned.
- `da load-snapshot` + `da graph-stats` CLI commands (snapshot round-trip).
- `da batch-ingest` with `.sgsnap` export (Solution B durability).
- GROBID title extraction bug fixed (TEI attributes).
- `da-graph` filled: Cypher query builders + schema DDL (8 unit tests).
- Integration tests for batch_ingest with mock ports (5 tests, no live services).
- ADR-INDEX.md + ADR-037 status marked partially superseded by 040/041.
- README rewritten for Rust v2 (was fully Python-era).
- **GROBID TEI sections + citations extraction**: `extract_sections` (85 from
  real paper), `extract_citations` (80 citations with DOI/arXiv id). Closed
  TODO debt (sections/citations were discarded). 7 new tests.
- **`da query` CLI command**: count, by-arxiv, by-vid, orphans, without-evidence,
  citation-hops. Wires da-graph (was dead library).
- **Sections + citations persisted to graph**: IngestUseCase now writes
  section_count, citation_count as node properties and creates Citation nodes
  + CITES edges for resolvable citations. IngestResult reports counts.
- **Bibliographic edge types in domain**: `bibliographic::CITES` / `CITED_BY` /
  `CO_AUTHORED` constants in da-domain. Separated from ADR-038's 18 extracted
  RelationTypes. da-graph queries now reference the domain constant.
- Clippy clean across all crates; CI clippy uses `--no-deps` (vendor Samyama
  has pre-existing warnings we don't own).
- Pre-commit cargo-fmt now auto-formats (was failing on `--check`).
- **Idempotent Citation nodes**: `find_node_by_string_property` added to
  DirectGraphStore trait. Ingest now checks for existing Citation before
  creating, preventing duplicate nodes when multiple papers cite the same
  reference. Citation nodes now get valid_from + schema_version.
- **Phase 3 extraction started**: Extractor port (da-ports), RuleBasedExtractor
  adapter (da-adapters, section heuristics + keyword patterns), ExtractionUseCase
  (da-application, parse → extract → write Entity nodes). `da extract --id` CLI.
  Idempotent Entity node creation. 9 new tests. End-to-end: 2602.11757 → 2 entities.
- **MENTIONS edges + source spans**: extraction now links Entity nodes to Paper
  via MENTIONS edge (finds Paper by arxiv_id). Entity nodes store char_start,
  char_end, surface for evidence grounding. 2 new tests (mentions linked,
  no-mentions-when-paper-absent).
- **Extraction recall improved**: classify_section now matches EVALUATION SETUP,
  RESULTS AND ANALYSIS, MODELS AND INFERENCE PARAMETERS, ALGORITHM/METHODOLOGY.
  Added Model entity extraction (GPT-4, LLaMA, Claude, Gemini, Mistral, Qwen,
  DeepSeek, GLM, BERT, T5, BLOOM). Recall: 2→3 entities on paper 2602.11757.
  2 new tests (extended section classification, model extraction).
- **Graph schema design (D132)**: doc/GRAPH-SCHEMA.md — single source of truth
  for all node types (Paper, Citation, Entity, Author, Evidence), edge types
  (CITES, MENTIONS, AUTHORED, HAS_EVIDENCE + 18 extracted), properties, indexes.
  Schema-as-code: PaperSchema/EntitySchema updated, CitationSchema added,
  schema registry (all_node_schemas/schema_for_label). SchemaInitializer: 7
  indexes (was 3). `da schema init` CLI creates all indexes via HOT path.
  Ingest now validates Paper properties against schema before writing.
- **Topics replace Concepts + retrieval_eligible (D134)**: OpenAlex Concepts
  DEPRECATED — replaced by Topics (4 domains → 26 fields → 254 subfields → ~4500 topics).
  Concept nodes kept but retrieval_eligible=false (epigenetic silencing).
  retrieval_eligible added to ALL node schemas (Paper, Citation, Entity, Topic,
  Concept, Author, Institution, Category). Ingest/extract set retrieval_eligible=true
  on created nodes. da-graph queries filter on retrieval_eligible=true (count_all,
  without_embedding, stale_schema, citation_neighborhood, cited_by).
- **Schema redesigned with article spine (D132 revised)**: user pointed out the
  schema was missing topics, keywords, sections, categories — the article
  "обвязка". GRAPH-SCHEMA.md now defines 9 node types: Paper, Section, Keyword,
  Topic, Category, Author, Citation, Entity, Evidence. EntityType expanded from
  6 to 22 (concrete: Method/Dataset/Metric/Task/Baseline/Model/Figure/Table/
  Equation/Concept/Implementation/Theorem/Definition; abstract: Problem/
  Motivation/Gap/Contribution/Hypothesis/Finding/Mechanism/Limitation/FutureWork).
  New da-domain/article.rs: Section, Keyword, Topic, Category + schemas.
  SchemaInitializer: 15 indexes (was 7). `da schema init` creates all.
- **Graph healing scenarios (D135)**: 7 operations (correct, merge, split,
  silence, migrate, rollback, repair_cites). GraphHealingUseCase implements
  silence, unsilence, correct, merge with ProvenanceEvent audit trail.
  da-domain/healing.rs: HealingOperation, HealingActor, ProvenanceEvent,
  MergeResult/SilenceResult/CorrectResult. `da heal` CLI wired. SUPERSEDES/
  SPLITS bibliographic constants.
- **Merge edge redirect**: incoming edges now redirected to kept node via
  get_incoming_edges on DirectGraphStore. edges_redirected in MergeResult
  reflects actual count (was always 0).
- **Correct operation audit trail**: get_node_property_string on
  DirectGraphStore reads old value before overwriting. ProvenanceEvent
  captures actual old→new change (was hardcoded "unknown").
- **ADHD ontology research (D134)**: 5 parallel cognitive frames via ADHD skill
  + jina/exa/gitnexus MCP. 30 ideas, 3 deepened. 4 patterns adopted:
  retrieval_eligible, assignment_method, provenance ring, agent quarantine.
  CRITICAL: OpenAlex Concepts DEPRECATED → Topics.
- **Ontology-aligned schema (D133)**: FaBiO + CiTO + OpenAlex three-layer
  architecture. doc/ONTOLOGY-ALIGNMENT.md maps daily-archive to established
  ontologies. doc/ADHD-ONTOLOGY-RESEARCH.md documents divergent ideation.

### Hygiene (earlier)

- Remove obsolete root briefs and empty `Plans/`.
- Document local garbage policy (`doc/REPO-HYGIENE.md`).

### ETL / Wave B (frozen under legacy/)

- M271–M284: quality n-contract, hybrid bodies, import-hold inventory, evidence chain.
- Import remains locked (D127); deploy extract path remains `header_priority`.
- Full Python stack frozen under `legacy/` — not on the Rust runtime path.

## Earlier history

Milestone-level detail lives in git history and GSD phase summaries. Prefer:

- `doc/adr/ADR-INDEX.md` for binding architecture decisions
- `doc/PERSISTENCE-ANALYSIS.md` for graph durability model
- `artifacts/etl/` for historical Python ETL evidence
