# Changelog

## 2026-09-05 — M472-nxpy4n/S01: governor flags bare from_env PDF constructors

DATA lists allowed GrobidParser::from_env roles. Governor scans declared
da-cli roots and flags a bare from_env used as a PDF parse constructor
(policy_stamp). Health probes, with_profile, consult helper and tests stay
allowed. Facade default stays rich. SHA of grobid_compatibility.yaml rebound
in the same change. No graph. D127 false.

Green: scholarly_parse_governance 85.

## 2026-09-05 — M471-oi80qz/S02: production PDF constructors consult ProfileRoute

ingest, batch-ingest, extract, summarize, scheduler, da load journey,
multimodal production, eval_batch, eval_extract, figure_context,
gepa_hub_research, pipeline_figures, gold_corpus_canary and catalog_sweep
PDF parsers use corpus_grobid_parser() (DATA structural default).
Health is_alive probes keep from_env. Facade default stays rich.
No graph. D127 false.

Green: parse_profile 6; load 20; production 11; load_journey 2; bins+examples check.

## 2026-09-05 — M471-oi80qz/S01: shared ProfileRoute consult helper

consult_corpus_grobid_profile fail-opens to DATA structural (D204).
da load PDF path uses the consult instead of from_env().profile().
GrobidParser::from_env stays historical rich. HTML path unchanged.
No graph. D127 false.

Green: parse_profile 6; grobid_profile_from_route 2; stamp_parse_grammar.

## 2026-09-05 — M470-485ukv/S02: parse-pdf consults ProfileRoute

parse-pdf maps consulted ProfileRoute to GrobidProfile instead of hardcoding
structural(). Receipt grobid_profile is stamped only on the Java parse-pdf
path. grammar stamp no longer claims a Java profile. DATA default stays
structural (D204). Rich is a closed consult variant, not corpus default.
No WASM wat on this point. No graph. D127 false.

Green: resolve_parse_profile 4; parse_grammar 12; stamp + grobid_profile CLI 4.

## 2026-09-05 — M470-485ukv/S01: FSM ProfileRoute native default structural

DecisionPoint now parses profile_route as its own point, not an alias of
grammar_route. Closed ProfileRoute {structural, rich}. Native fallback
consults DATA default=structural (D204). Rich is named, never implied.
No WASM wat. Governor SHA-binds data/policies/registry.yaml and flags
non-structural default via policy_stamp. No graph. D127 false.

Green: da-domain fsm_policy 15; da-application fsm_policy_fallback 5;
parse_grammar 8; wasm_policy 3; scholarly_parse_governance 83.

## 2026-09-05 — M469-j23uk8/S01: stamp grobid_profile separate from grammar_profile

parse-pdf receipts now stamp grobid_profile=structural next to FSM grammar_profile.
Stdout names both fields so they cannot be mixed. Java GROBID call stays
GrobidProfile::structural() (D204 corpus default). No rich. No fingerprint
rotation. No graph. D127 false.

Green: stamp_parse_grammar unit; choose_parsed_action unit.

## 2026-09-04 — M468-4k3p7s/S01: ledger and leftover pins stop claiming loss_detected

`defaults.promotion.reason` no longer says tracked conservation is still
loss_detected. Pristine cache_identity and parse_compat_baseline pins are
Conserved. Frozen lift hashes rotated after caption-head skip (sections
264→183). Mutated citation deletion still LossDetected. SHA of
grobid_compatibility.yaml bound in scholarly_parse_plane in the same change.
this_wave_promotes stays none. Java production_route. No graph. D127 false.

Green: cache_identity 23; parse_compat_baseline 16; data_contracts 74;
test_scholarly_parse_governance 83.

## 2026-09-04 — M467-ase1bu/S03: Conserved acceptance evidence after region honesty

Regenerated artifacts/parse_compat/acceptance_evidence.json from ConservationEngine.
Three preserved TEI documents are Conserved; no_silent_loss is true; verdict PASS.
SHA bound in scholarly_parse_plane in the same change. Java production_route.
No graph. D127 false.

Green: acceptance_evidence offline; test_scholarly_parse_governance 83.

## 2026-09-04 — M467-ase1bu/S02: flat lifter skips caption heads; anchors bag-aligned

TeiLifter extract_sections_flat no longer treats figure/table caption `<head>` as a
section. Four flat walkers share next_section_head so tables, formulas and
anchors stay on the same windows. Citation anchors compare as a digest bag:
bibliography grouping is not Reordered. Pristine conservation on the three
preserved TEI files is Conserved. Hub body-head floor is 69 (was 80 while
captions inflated the IR). Java production_route. No graph. D127 false.

Green: extract_ 45; conservation lib 9; conservation_contract 14; conservation_properties 6.

## 2026-09-04 — M467-ase1bu/S01: three-region scanner honesty for back annex

ConservationEngine now has three TEI regions: header, body, back. Annex `<head>`
in `<back>` is not a Section. A targeted `<ref type="bibr">` in `<back>` is a
typed CrossReference remainder, not CitationAnchor linkage_loss. Back
`biblStruct` stays CitationEntry (bibliography lives there). Pins:
section_missing 0/0/0, linkage_loss 0/0/0, rlm cross_refs 67. Remaining
LossDetected is CountMismatch: the flat GROBID head scan still emits figure
caption heads as sections (S02). Java production_route. No graph. D127 false.

Green: conservation lib 9; conservation_contract 14; conservation_properties 6.

## 2026-09-04 — M466-xxitm7/S03: honest acceptance evidence after scanner rewrite

Regenerated artifacts/parse_compat/acceptance_evidence.json from ConservationEngine.
Verdict stays FAIL: remaining extra body heads (hub 1, rlm 6) and rlm
linkage_loss=1 are still typed losses. no_silent_loss stays false. SHA bound
in scholarly_parse_plane in the same change. Java production_route. No graph.
D127 false.

Green: acceptance_evidence offline; test_scholarly_parse_governance 83.

## 2026-09-04 — M466-xxitm7/S02: unresolved bibr is not linkage loss

A GROBID `<ref type="bibr">` without `target` is an unresolved marker, not a
lost citation link. ConservationEngine classifies it as a typed CrossReference
remainder. linkage_loss is now 0/1/0 on hub/rlm/reca (was 36/29/99). Remaining
rlm=1 is a targeted bibr the nested-div lifter still does not attach. Java
production_route. No graph. D127 false.

Green: conservation lib 7; conservation_contract 14.

## 2026-09-04 — M466-xxitm7/S01: scanner honesty for header biblStruct and caption heads

ConservationEngine no longer counts the paper's own header sourceDesc/biblStruct as a
cited work, and no longer counts figure caption heads as sections. That was a
scanner lie, not a missing bibliography. Remaining pinned section_missing is
1 (hub) and 6 (rlm) extra body heads the nested-div lifter does not emit.
citation_missing is 0. linkage_loss stays typed. Java production_route.
No graph. D127 false.

Green: conservation lib 6; conservation_contract 14.

## 2026-09-04 — M465-sqt7w0/S07 W6: identity ignores workers, no family promote

Reuse stamp_parse_grammar and layout_smp receipts. TeiIdentity and D204 fingerprints
do not contain worker/smp/concurrency. SMP semantic receipts stay equal for
workers 1..=cap. grobid defaults.promotion: this_wave_promotes none, deployed false,
max_non_candidate_families 1. Conservation is still loss_detected so no family
is candidate-promoted. Governor adds family_promote_cap and worker_identity_leak.
Java production_route. Graph frozen. D127 false.

Green: identity_and_fingerprint_ignore_worker_count; layout_smp every_worker_count;
architecture_contract 38; data_contracts 74; test_scholarly_parse_governance 83.

## 2026-09-04 — M465-sqt7w0/S06 W5: missing Image is typed absence not zero figures

Reuse detect_figure_heads/table/formula. An observation without Image/Graphic
returns Terminal NotObserved, not Figure with empty receipts. Table and Formula
stay independent on the same observation. Ledger stage.figure_table_formula is
shadow with native_visual.rs. Governor adds typed_visual_absence and
zero_count_from_absence. Fulltext composition stays forbidden. Java
production_route. No graph. D127 false.

Green: native_visual_contract 6; architecture_contract 38;
test_scholarly_parse_governance 79.

## 2026-09-04 — M465-sqt7w0/S05 W4: Java body bytes survive splice, overlap is hard fail

Reuse compose_native_fulltext. Only admitted header/reference subtrees splice.
Exact <body> bytes after splice equal Java. listBibl inside body is
ChangedReadingOrder and returns the original envelope. DATA independently_selectable
stays header,references; segmentation/visual stay fallback. Governor adds
body_java_fallback and reading_order_change. Java production_route. No graph.
D127 false.

Green: native_fulltext_composition_contract 6; native_fulltext_composition_data_contract 2;
architecture_contract 38; test_scholarly_parse_governance 75.

## 2026-09-04 — M465-sqt7w0/S04 W3: bibliography segment then fields then compose

Reuse NativeReferenceSegmenter, NativeCitationParser, and compose_references.
Ledger citation names/citation/list/reference_segmentation are shadow with crate
surfaces (were absent or a CLI example). run_reference_sequence is segment then
fields then compose; empty list or empty middle segment cannot invent a
ReferenceCandidate. Structural profile still has consolidate_citations=false.
Governor adds bibliography_sequence and lost_references. Java production_route.
No graph. D127 false. Patent citation endpoints stay absent.

Green: native_composition_contract 10; architecture_contract 38; data_contracts 74;
test_scholarly_parse_governance 71.

## 2026-09-04 — M465-sqt7w0/S03 W2: header sequence date then name then affiliation

Reuse NativeDateParser, NativePersonalNameParser, NativeAffiliationParser,
and compose_header. Ledger surfaces that hid none behind prose now point at
native_date.rs, native_personal_name.rs, native_affiliation.rs, and
native_composition.rs (shadow). run_header_sequence stops on empty date and
rejects empty or foreign authors. Governor ledger_surface treats none-prefix
prose as missing; header_sequence and invented_author gates added.
Architecture proxy set includes W0 health/version. Java production_route.
No graph. D127 false.

Green: native_composition_contract 8; architecture_contract 38; data_contracts 74;
test_scholarly_parse_governance 67.

## 2026-09-04 — M465-sqt7w0/S02 W1: lexical lattice honesty, one region per token

Reuse, not a new parser. LayoutTokenKind::from_lexical_text never returns
Image or Formula. NativeSegmenter assigns exactly one closed region per
admitted token including opaque visuals. Ledger stages pdfalto_layout and
document_segmentation are shadow with real surfaces (were fallback/none).
Governor adds lexical_visual_mix and region_coverage. Existing 9-lane
segmentation report stays mixed pass/unsupported; unsupported is typed
absence, not a fake 202-page green. Java production_route. No graph.

Green: layout_contract 8; native_segmentation_contract 5; data_contracts 74;
test_scholarly_parse_governance 63.

## 2026-09-04 — M465-sqt7w0/S01 W0: honest GROBID probes, measured conservation, policy stamp

ADR-076 / D321: three automata, graph frozen, M464 was platform-only.
GROBID GET /api/health and /api/version are fail-closed probes on
GrobidHttpBackend (not ParseBackend). TeiIdentity.backend_version is
additive; historical cache keys unchanged until a version is stamped.
Acceptance evidence measures ConservationEngine on three preserved TEI
files instead of hardcoding no_silent_loss. Current lift is loss_detected
on all three, so verdict is FAIL (honest). parse-pdf stamps policy_source
wasm|native_fallback|none and policy_hash or none. Governor scholarly_parse_plane
gains ledger_surface, backend_identity, policy_stamp, silent_loss.
No graph writes. D127 false.

Green: da-parse backend_contract+data_contracts+cache_identity+conservation;
da-cli source_cli; test_scholarly_parse_governance 59; clippy da-parse/da-cli.

## 2026-08-25 — M462-ieh2tt: load network is on by default

--allow-network was an extra step. AcquisitionPolicy.allow_network now
defaults true. da load / acquire-arxiv fetch missing catalog assets unless
--disallow-network or DA_LOAD_DISALLOW_NETWORK=1. Hidden --allow-network
is a no-op so old scripts keep parsing. ensure_only() stays offline for
tests. Catalog hits still work with allow_network false (ADR-056).
D127 false. D290.

Green: policy_network; resolve_network; mock_miss; ensure_miss;
acquisition_batch 2; arxiv_acquisition_contract 4; clippy
da-domain/cli/ports/application --lib; cargo build -p da-cli.

## 2026-08-25 — M461-6665iq: URL locator is network intent for load FSM

Pasting an arXiv URL is now a first-class path. locator_implies_network is
true only for http(s); bare ids stay fail-closed (ADR-056). da load on a
URL allows network unless DA_LOAD_ALLOW_NETWORK is 0/false and defaults
stages to ingest+extract. da source run canonicalizes the URL to the
arXiv id, ensures the catalog, then walks the existing FSM. Parser
try_parse_arxiv_locator is unchanged (GitNexus CRITICAL). D127 false. D289.

Green: locator_implies 1; resolve_network 1; url_locator_defaults 1;
normalize_source_target 1; clippy da-domain/cli --lib; cargo build -p da-cli.

## 2026-08-25 — M460-7tqron: operator graph-query honours exhausted negatives

M459 gated agent-run; da graph-query and da query still hit the store without
admit_retrieval. admit_operator_read maps paper-scoped admit_retrieval onto
GraphQueryCliError::Runtime containing negative_blocked. Empty paper_id
fail-opens. graph_query::run consults before opening the store. da query
by-arxiv/by-vid uses the same helper; count/orphans/graph-stats stay ungated.
CLI reads DA_RETRIEVAL_PAPER_ID and DA_NEGATIVE_MEMORY_DIR inline (dual bin/lib).
Not Samyama. D127 false. D288.

Green: graph_query 9 including admit_operator_read + operator_query_needs_admit;
clippy -D warnings da-cli --lib; cargo build -p da-cli.

## 2026-08-25 — M459-sfjk31: agent retrieval honours exhausted negatives

Vasiliev gap after M456-M458: LLM stages and Layer H freeze existed, but
AgentReadPlane.query could still return evidence for a paper whose critic
repair exhausted. admit_retrieval is paper-scoped: any exhausted negative
for that paper blocks. Empty paper_id fail-opens. Blocked is typed
AgentFailureCode::NegativeBlocked (FailedVisible), never rows=0.

AgentRunUseCase::with_negatives consults after plan validation and before
identity/query. CLI reads DA_RETRIEVAL_PAPER_ID and opens
JsonlNegativeMemory from DA_NEGATIVE_MEMORY_DIR (inline env; dual bin/lib).
Not Samyama. D127 false. D287.

Green: admit_retrieval 4; all_failure_codes; agent_run 26 including Spy
plane query_calls=0; da-cli agent_run 22; clippy -D warnings
da-domain/application/cli --lib.

## 2026-08-25 — M458-qxcimz: ContextBundle freezes as_of policy_hash and MSC

ADR-053 Layer H was identity + evidence + claims only. ContextBundle now
carries optional as_of, policy_hash, scope, task_kind, compiled_at
(serde default empty so legacy JSON still loads). freeze_id hashes
identity + as_of + policy_hash + scope + task_kind; compiled_at is
observational and excluded. AgentRunRequest.freeze is stamped onto the
bundle at construction. CLI da agent-run reads DA_CONTEXT_AS_OF,
DA_CONTEXT_POLICY_HASH, DA_CONTEXT_SCOPE, DA_CONTEXT_TASK_KIND; compiled_at
from SystemClock. Domain stays clock-free. Not Samyama. D127 false.

Green: freeze_id 2 + legacy JSON 1 + context_bundle 2; agent_run 24
including run_stamps_request_freeze; da-cli agent_run 22 including env
freeze; clippy -D warnings da-domain/application/cli --lib.

## 2026-08-25 — M457-jj82w7: figure_analysis_stage admits before multimodal LLM

Same Vasiliev/M455 gap as summaries: figure_analysis_stage called the
vision model without consulting negative memory. execute() now admits
canonical_id+figure_analysis (pattern=None) before analyze(). Missing
canonical_id fail-opens (legacy unbound contexts). Blocked maps to
Terminal error_class=negative_blocked; SpyModel is never called.

CLI production.rs and load_journey.rs open JsonlNegativeMemory from
DA_NEGATIVE_MEMORY_DIR (inline env; bin/lib dual compile). Open failure
fail-opens the stage without a store.

Green: figure_analysis_stage 11 (blocked skips LLM + missing id
fail-open), clippy -D warnings da-application/cli --lib.

## 2026-08-25 — M456-g1nv83: summaries_agent_stage admits before LLM

M455 CLI gate was bypassed by the pipeline. Now run_summaries_agent
consults NegativeMemoryPort (stage=summaries, pattern=None) before the
LLM call. BlockedExhausted is a typed error; SpyLlm is never invoked.
Admit without a pattern fail-closes on any exhausted record for that
stage (pipeline reuse). CLI build-summary and eval-frozen open
JsonlNegativeMemory from DA_NEGATIVE_MEMORY_DIR.

Live: critic-repair exhausted on p-block/summaries → build-summary
prints blocked_exhausted, no model call. Not Samyama. D127 false.

Green: domain negative_memory 7, summaries_agent_stage 2, clippy
-D warnings da-domain/application/cli --lib.

## 2026-08-25 — M455-5jxhte: negative semantic knowledge blocks reuse

Vasiliev gap: critic Reject/repair_exhausted was logged but retrieval
could reuse the same claim. Now:

- Domain: NegativeMemoryRecord + admit_claim (key = paper_id+stage+pattern).
  Exhausted reject denied; other paper/stage/resolved allowed.
- Port NegativeMemoryPort separate from CVF AgentMemory.
- Jsonl adapter `{dir}/{paper_id}.jsonl`. Path: DA_NEGATIVE_MEMORY_DIR.
- persist_if_exhausted only on exhausted outcomes.
- CLI: critic-repair writes negative; `da source memory-admit` denies.
  Live: 2507.19457 figure_analysis schema violation → blocked;
  other paper/stage → allowed. Not Samyama. D127 false.

Green: domain negative_memory 6, application critic_repair_stage 5,
adapters jsonl 1, clippy -D warnings touched crates --lib.

## 2026-08-25 — source capture: Vasiliev 2026 three graph roles

Added Substack article as a blog source (not graph truth, D127):
`data/sources/blog/vasiliev2026knowledge-graph-memory-graph/`.
HTML captured locally (gitignored `data/**/*.html`, sha256 in identity.json);
tracked: identity.json, citation.bib, article.md sidecar, NOTES.md, loader.
Cited from ADR-040 (tier = roles), ADR-048, ADR-053 Layer H, ADR-074.
Research note: `doc/research/vasiliev-2026-three-graph-roles.md`.

## 2026-08-25 — M454-c3yx94: critic repair process + Governor current-milestone

Two process gaps closed:

1. Critic repair was DATA + a domain library with zero application callers.
   AgentManifest now parses optional `repair.mechanism.reject_triggers`.
   `run_critic_repair()` calls `route_repair` and returns typed RepairOutcome.
   CLI: `da source critic-repair` (DA_CRITIC_* env). Live: reject →
   figure_analysis/reanalyze_with_diagnostics; confirm → no action;
   attempts=2 → repair_exhausted.

2. Governor flagged every ROADMAP 🔄 empty PLAN (M447/M449/M450 ghosts).
   Empty-PLAN diagnostics now apply only to STATE-active (or preferred
   ROADMAP-active if STATE missing). Live: only M446-rw9a2p/S02 remains.

Green: domain agent_manifest 4, application critic_repair_stage 4,
pytest test_gsd_planning_reader 15, clippy -D warnings da-domain/
application/cli --lib.

## 2026-08-25 — M453-aym18d: parse-pdf consults grammar_route

Process gap from M452: WASM/native policy host existed, parse-pdf still
hardcoded grammar_profile=arxiv_pdf_dot.

- DATA: grammar_route.profile_map + default_profile in
  data/policies/registry.yaml. xy_cut stamps intent, keeps arxiv_pdf_dot
  parse path (no XY-Cut CLI yet).
- Application: resolve_parse_grammar() — consult + map + fail-open (8 tests).
- CLI: run_parse_pdf stamps grammar_profile, grammar_route, policy_hash.
  FSM parsed-stage: page_outline + TOC cache → parse-pdf-toc, else parse-pdf.
  Features from clap / DA_PARSE_* env / TOC observation cache.
- ADR-074 accepted (W1–W3 infra; LoRA still D136).

Green: domain fsm_policy 11, application parse_grammar 8, cli stamp+choose 2,
clippy -D warnings da-domain/application/cli --lib.

## 2026-08-25 — M452-mhx138 S02: WASM FSM policy host (ADR-074 W3 infra)

Hot-swappable policy modules at FSM decision points. No LoRA (D136).

- Domain: `ArticleFeatures`, `GrammarRoute`, `Admission`, `PolicyRegistry`
  (YAML-driven native thresholds). 10 tests.
- Port: `FsmPolicyPort` in da-ports. Native fallback in da-application
  (fail-open on Load/Instantiate/Io). 4 tests.
- Adapter: `SharedWasmPolicyHost` (wasmi 0.51) in da-adapters only.
  WAT fixtures under `data/policies/`. Hot-swap test: hash + decision
  change without process restart. 3 tests.
- DATA: `data/policies/SCHEMA.yaml` + `registry.yaml`; `policies` added
  to `data/SCHEMA.yaml` and governor structural_directories.
- CLI: `da source sona-policy <point> [--pages --bookmarks --quality]`
  Live: grammar_route typical → xy_cut (wasm sha256 stamped);
  monograph 700p+TOC → page_outline; scan → grobid_structural;
  batch_admission pages=0 → skip (native).
- D285: wasmi not wasmtime; onion boundary held.

Green: domain fsm_policy 10, application 4, adapters wasm_policy 3,
clippy -D warnings on da-domain/ports/application/adapters/cli --lib.

### M446 W-A1: словари сделали живыми — кэш инвалидируется, rebuild 6 минут

- **Найдены и закрыты два системных разрыва** (DATA-first нарушался):
  1) Раннеры (sweep, production journey) строили экстрактор из
  **bundled-снимка в бинарнике** — правки data/extraction_patterns.yaml
  не меняли рантайм. Теперь: from_yaml_file за env
  DA_EXTRACTION_PATTERNS (default data/…), честный fallback на bundled.
  2) Stage-контракты несли **статический** configuration_fingerprint —
  смена словаря не инвалидировала кэш extraction. Порт Extractor
  получил cache_fingerprint() (default None), RuleBasedExtractor
  отдаёт sha256 значений конфига, RuleExtractionStage компонует
  `;rules_sha256=` в ключ. Тест: разные словари → разные ключи.
- **methods.phrases +18 фраз** (math-oc алгоритмы, SE-эконометрика,
  robotics diffusion): 3 zero-mention статьи извлекают 2/1/5 сущностей.
- **Полная пересборка: 14с sweep (604, кэш) + 6.3 мин материализация
  (589/589, 0 failed, 0 replay-noop)** — было ~2ч; индексы M443 дали
  ~19x на полном корпусе. Regression-check против предыдущего отчёта:
  0 findings. Drift: **0 zero-mention алармов**, mentions 9.84→11.08.
- Лоадер материализации различает старые/новые extraction-записи по
  живому fingerprint словаря (история не удаляется).
- Новый инструмент extract_probe (не-gold аналог eval_extract).
- **Отложено честно**: gold F1 spot-check заблокирован pre-existing
  аномалией — 19 статей лежат в 2+ категориях каталога
  (байт-идентичные PDF; чинится lookup-унификацией, не удалением).

### M445 pipeline stage audit: все 20 этапов против ADR

- Артефакт doc/review/pipeline-stage-audit-2026-08-22.md: поэтапная
  таблица (ADR/слой ACAP/логика/пробелы), планировщик и FSM-адмиссия,
  мета-промпт готовность + хантинг хардкодов (найден один: OpenAlex
  arXiv-DOI фильтр), инвентарь 29 YAML-словарей + приоритетные
  дополнения (citation_styles, venue_registry, author_identity,
  doi_prefix_map, method_taxonomy), научный контекст (якоря есть;
  DOI/Crossref/цитатные сущности — нет), реестр аномалий.
- Вердикт: линейный хребет acquisition→checkpoint ГОТОВ сквозно на
  корпусе arXiv-каталога; фронтир — мультисорс, словари цитирования,
  постраничный AST, агентный мозг.

### M444 corpus regression harness: регрессии ловятся на всех статьях по пути

- Направление пользователя: работать со ВСЕМИ статьями и ловить регрессию
  инлайн, не золотыми замерами. Gold остаётся только эталоном F1.
- **Regression diff engine** (da-cli/catalog_regression): типизированные
  находки между двумя run-отчётами — PapersRemoved/Added, RevisionDrift,
  StructuralDrop (с относительным допуском DA_REGRESSION_TOLERANCE),
  ReplayHonestyLost, FailedPapers; exit 1 при любой находке. 6 юнит-тестов.
- **corpus_regression_check** example: на идентичных реальных отчётах
  589/589 — 0 findings (exit 0); на тампернутом — поймал все 3 внесённые
  регрессии (удалённая статья, дрейф ревизии 2→3, entities 12→4), exit 1.
- **Drift-статистика по всем статьям** (catalog_drift_stats.json):
  9.84 mentions/статья в среднем; нулевых — ровно 3 (те самые low-yield
  не-CS статьи из бэклога). Семантика исправлена по первому применению:
  entities_created — глобальный dedup-счётчик (0 на статью = пере-
  использование vid, норма), alarm-сигнал — нулевые MENTIONS.
- **Попутно найден и исправлен баг артефактов**: PaperRunRecord писал
  `revision` дважды (дубликат JSON-ключа из flattened FSM-состояния);
  строгие парсеры отклоняли отчёты. Writer исправлен, закоммиченный
  full-run отчёт починен (round-trip dedup, значения идентичны).

### M443 indexed hot path: материализация 50–143x быстрее на полном сторе

- Диагноз: 2-часовой full-run M442 — каждый identity-lookup (Entity vid,
  Paper arxiv_id, Section/Reference/revision vids) без property-index
  линейно сканировал стор, растущий до 66k узлов; replay удваивает.
- Инфраструктура индексов в SamyamaGraphStore уже была (backfill+трекинг,
  протестирована) — composition root их просто не создавал.
- Новый контракт da-application/materialization_indexes: 9 уникальных
  identity-ключей + ensure_materialization_indexes; **Citation.arxiv_id
  сознательно исключён** (дубликаты между статьями: min-id vs scan-order
  семантика) — задокументировано.
- Runner обеспечивает индексы после open/preload. Бенчмарк на полном
  снапшоте (examples/materialize_index_bench): **Entity.vid 50x,
  Paper.arxiv_id 143x** на miss-пробах; сборка 9 индексов по 66k узлов —
  389 мс one-time.
- Регрессия A/B: 30-статейная канарка с индексами — **структурно
  идентичный** граф (3341/6990 узлов/рёбер), 0 replay-noop нарушений.
- Governor-пин logging поймал новый bench-пример до коммита (M440-урок
  работает). 5 новых тестов (2 lib + 3 integration).

### M442 full run: 589 статей каталога → единый семантический граф

- **589/589 completed, 0 failed, 0 replay-noop violations** (wall-clock
  ~2 ч, оффлайн, workers=1). Снапшот catalog_materialized.sgsnap:
  **66299 узлов / 169916 рёбер**; graph-stats disk→query same-process OK.
- Покрытие материализации каталога: **589/604 = 97.5%** (523 TEI + 66
  pdf-stamp recovery + 15 честных no_stamp).
- Сравнение с gold-графом (173 статьи, 21868/51518): плотность на статью
  сопоставима (catalog 112.6/288.5 vs gold 126.4/297.8 узлов/рёбер);
  наборы статей не пересекаются by construction — отчёт фиксирует
  honest non-equivalence (artifacts/catalog_sweep/
  catalog_vs_gold_comparison.json).
- Все 589 статей несут revision-owned identity (v1..v11).

### M442 feature-matrix gate + crash-safe resume материализации

- **Пин feature-matrix**: verify_rust_architecture.sh теперь компилирует
  `da-cli --features pdf-classifier --examples`; Governor-политика
  `feature_matrix_gate` (DATA YAML + Python-чек + 3 юнит-теста)
  флагает исчезновение пина. Урок M441 закрыт двумя слоями.
- **Crash-safe resume** (--resume / DA_MATERIALIZE_RESUME=1): прерванный
  full-run продолжает с места остановки — preload снапшота, skip
  завершённых статей по prior-отчёту, merge записей и totals.
  E2E: 5 → resume → 10 total, граф растёт ровно на новые статьи
  (380→680 узлов), 0 дубликаций, 0 replay-нарушений.
- Хелпер prior_completed_ids (flattened-state парсинг) + 3 юнит-теста
  (17 в модуле). Баг nested-парсинга пойман E2E-канаркой и исправлен.

### M441 version recovery: PDF-штампы закрывают 66/81 no_tei_revision

- Разведка: 9/12 пропущенных PDF несут штамп arXiv:<id>vN на первой
  странице — источник версии именно этого ассета (без сети и без
  угадывания latest).
- `arxiv_stamp_revision` (da-application, чистая логика): таблица
  форматов (vN, суффикс категории, перенос строки, регистр), отказ для
  чужих id/v0/без-версии; RevisionRecoveryFsm (TeiChecked → StampChecked
  → Recovered/StillUnbound, terminal-absorb) + D127-canary. 6 новых
  тестов (всего 15 в модуле).
- Runner: режим `--recover` за feature `pdf-classifier` (pdf-inspector,
  первые 2 страницы, оффлайн): **66 recovered / 15 still_unbound /
  0 pdf errors** по реальным 81; merge в --run через
  DA_MATERIALIZE_RECOVERY (+66 materializable).
- Merged-канарка 30 статей: 30/30 completed, 0 failed, 0 replay-noop;
  восстановленные статьи прошли до Completed со своими версиями
  (1611.01796→v2, 1905.08085→v5, 2005.14165→v4).
- Попутно исправлен pre-existing баг feature-сборки: load_journey звал
  trait-метод RegionTextPort без импорта трейта (ломал
  `cargo check -p da-cli --features pdf-classifier` со времён M422;
  полный гейт эту комбинацию не собирал). Ре-экспорт
  `da_adapters::pdf_inspector` для feature-пути.
- Env: DA_MATERIALIZE_RECOVERY_REPORT, DA_MATERIALIZE_RECOVERY.

### M440 sweep materialization: кэш свипа → семантический граф (S01-S03)

- Coverage-audit 604 ok-статей свипа по ADR-065 tei_declared_revision
  fallback: **523 materializable (86.6%), 81 typed-skip (no_tei_revision)**,
  reconcile без пропусков; артефакт materialization_coverage.json (D283).
- Явный FSM материализации (da-application/catalog_materialization.rs):
  scan→bind→materialize→verify(replay-noop), терминальные состояния
  поглощают события, illegal-переходы типизированы, D127-canary —
  import/promote/unlock событий нет by design; proptest на absorb.
- Runner (examples/catalog_materialize_runner.rs): полный оффлайн-replay
  кэша (parse+extraction+embedding) через materializer-only use cases,
  revision-owned identity, manifest-дайджест из parsed.pdf_hash с
  cross-check против sweep-отчёта (drift → typed Fail), экспорт .sgsnap.
  Env-ручки: DA_MATERIALIZE_{LIMIT,SNAPSHOT,RUN_REPORT} (.env.example).
- Канарка 25 статей: **25/25 completed, 0 failed, 0 replay-noop
  violations**; повторный прогон структурно идентичен (2626 nodes / 5484
  edges; байтовые отличия — только Clock-метки по ADR M330/M398);
  graph-stats по цепочке disk→query one-process OK.
- Тесты: 25 новых (14 da-cli audit+loader, 9 FSM incl. proptest,
  2 integration MockGraphStore); clippy чист.

### M439 catalog-wide sweep: 606 статей обработаны, отклонения типизированы

- Полный обход каталога вне gold-200: **604/606 ok (99.7%), 0 failed**,
  9 типизированных source-аномалий, wall-clock 12.3 мин (workers=4).
- Найден и исправлен продуктовый баг: rule_extractor резал текст по
  байтовым индексам из lowercase-копии — паника на мультибайтных символах
  роняла весь прогон. Char-boundary snapping + red-first регресс-тест с
  мутационной проверкой (panic воспроизводится двойной мутацией).
- lib `da-cli/catalog_sweep.rs`: DATA-пороги, типизированный
  классификатор отклонений, инвентарь; example-runner с resume.
- Отклонения: 3 статьи с 0 entities (не-CS лексика — расширение DATA
  словаря), 9 source-аномалий (8 no_pdf → HTML-путь, 2 инфраструктурные).
- Метрики и рекомендации оптимизации: artifacts/catalog_sweep/
  {sweep_report,aggregate}.json + analysis.md.


### M438 architecture closure: orchestrator defer + drift-gate coverage

- ADR-066 / D280: PipelineOrchestrator stays product-side — engine
  migration (ADR-064 §5 follow-up) closed as deferred-by-design. Recon
  showed the overlap with kg-pipeline's ExecutionEngine is ~30 lines of
  step-walking; the orchestrator owns durable cross-process retry
  (RetryableFailure persisted to cache, RetrySuppressed on next run)
  while the engine owns bounded in-process retry with an injected
  Sleeper — complementary semantics, not duplication. ACAP admission,
  cache identity, GraphMaterialize rerun and the StageRunRecord surface
  must not enter a universal crate.
- docs_drift_check.sh now covers all five doc-rendering commands: the
  three original ones plus `audit-fields` and `schema-check` (the
  ADR-045 Wave E schema drift gate). GRAPH-SCHEMA.md gained a canonical
  "Schema drift gate" section carrying both commands' current output and
  the same-commit update rule. Negative fixture proven: hiding either
  command marker from both docs makes the detector report it. The hook
  stays advisory-only by design (FAST-ONLY pre-commit policy).


### M437 corpus-wide semantic graph rebuild (MEM679)

- Full gold-corpus semantic graph materialized for the first time:
  **173 papers -> artifacts/gold_corpus/corpus_graph.sgsnap**
  (21868 nodes / 51518 edges), sha256-bound AllWavesReport with
  `replay_zero_writes` on every wave and a continuous graph chain —
  the ADR-051 §8.1 disk-to-query contract now proven at corpus scale.
- Process gap fixed (ADR-065): catalog is 100% versionless, so exact
  revision binding blocked the whole corpus. Added honest
  `tei_declared_revision` binding from the GROBID idno already stored in
  the parse cache (`arXiv:{id}v{N}`); fail-closed preserved when the
  document declares no version. 27 papers without any declared version
  are excluded via filtered DATA manifest (no silent denominator loss).
- Canaries: canary verification stage-id drift fixed (report-driven
  `extraction_compute` instead of hardcoded `extract` — red-first test);
  20 embedding-enabled waves over the shared cache root,
  `replay_all_cache_hit` everywhere, import_eligible=false throughout.


### M436 kg-pipeline honest execution engine

- Recon finding: `kg-pipeline::ExecutionEngine` was a fictitious contract —
  the doc-comment promised retry/failure policy/status tracking while the
  loop aborted on first error; `RetryPolicyRef` and `FailurePolicy` were
  never consumed; 4/6 `PipelineStatus` variants were unreachable. The
  product path never noticed because execution lives in
  `PipelineOrchestrator` (ADR-049) and kg-pipeline was used only for
  topology.
- Fixed (ADR-064): per-step retry from a policy registry with an injected
  `Sleeper` port (ADR-051 determinism infra — no real sleeps in tests);
  `FailurePolicy::Abort`/`Continue`/`RetryThenAbort` semantics with
  transitive skip of dependents; `attempts_per_step`/`skipped_steps` in
  `ExecutionResult`; observable FSM — `StepEvent` stream (Started/Retried/
  Failed/Skipped/Completed + StatusChanged Pending→Running→terminal) with
  all six statuses reachable; cooperative `EngineControl` pause/stop
  checked between steps.
- Red-first + mutation-checked: retry success on attempt 3, sleeper call
  count, Continue→abort mutation and pause-check mutation both caught by
  tests. `execute()`/`execute_with_policies()` keep historical single-
  attempt abort behavior (no consumer change; da-application
  acap_control_plane_test green).


### M435 Governor pins the audit gate

- M433/M434 made the audit-fields drift gate structurally correct
  (multi-line `extract_set_fields`, both product surfaces), but it only
  ran under CI's `schema_check_test` — in the dev loop a regression to
  the line-based scan or a single-surface scope would pass every commit
  and silently re-blind the ADR-045 gate.
- Two Governor pins (`audit_field_scanner` requiring `extract_set_fields`;
  `audit_both_surfaces` requiring `src_dirs` and forbidding the restored
  line-by-line `for bline in block.lines()` blind spot) now enforce it at
  every commit in ~1s with no build — the first line of defense.
- The pin construction itself exposed the scanner's doc-comment
  false-positive class once more: a naive `forbidden: line-by-line`
  tripped on the module doc's own description. The forbidden pattern is
  the exact regressed code shape, not the English phrase. D127 locked.

### M434 Audit both surfaces and declare sha fields

- The audit-fields drift gate scanned only da-application/src — blind across
  the crate boundary. da-cli load_journey stamps `embedding_sha256` on Paper
  (the first-attach path beside add_vector) and the shared embedding_write
  helper stamps it on PaperRevision (the revision content node); neither
  schema declared it, and M432 declared it only on Entity. The extended
  audit (90 files across da-application + da-cli) immediately reported the
  Paper drift — red run closed by declarations on both schemas.
- The scanner also learned to ignore its own doc-comment examples (a
  docstring `create_node("Label")` / `set_node_property_*(…, "field", …)`
  was being reported as drift). Binary audit now reports OK scanning both
  surfaces; schema unit tests pin the two new declarations. D127 locked.

### M433 Multi-line field scanner

- The audit-fields drift gate was line-based: cargo fmt routinely splits
  `set_node_property_*` calls across lines, and the scanner missed exactly
  those fields (M432: 2 of 4 drifted fields invisible — a blind gate is a
  fake gate). The extraction now lives in a pure tested lib function
  `da_cli::audit_fields::extract_set_fields` scanning the block from each
  occurrence to the matching closing paren.
- The fixed scanner immediately exposed its own false-positive class: a
  dynamic property loop (field passed as a variable) has no string literal
  and an unbounded scan misattributed the next literal —
  `create_edge(…, "hasSummary")` was reported as a Summary property.
  Paren-bounding closes it: the field must be a string literal inside the
  call's argument list. Five fixtures (single-line, multi-line, chained,
  no-string, dynamic-loop regression); binary audit OK. D127 locked.

### M432 CI matches the swept test surface; schema drift closed

- CI listed a dead `-p da-graph` (clippy fails with unmatched package ID
  on a fresh runner), omitted da-mcp entirely (46 real-engine tests never
  ran in CI), and skipped da-cli/da-adapters integration suites incl.
  the ADR-061 cross-process contract. Fixed and synced with the verify
  script. Running the suites locally exposed real schema drift: 4 fields
  written by M418-M419 but never declared (Paper triage trio, Entity
  embedding_sha256) — declared, audit-fields OK. The figure-journey test
  pins DA_FIGURE_QUOTE_POLICY absolutely. D127 locked.

### M431 Honest direct property write on the adapter
## 2026-08-25 — toolchain 1.94.1 → 1.98.0 (Rust 2025–2026 wave study)

Изучена сводка компиляторных изменений 2025–2026; применено:

- **rustc 1.94.1 → 1.98.0** (rustup update stable): v0 symbol mangling
  на stable (читаемые backtrace с generic-типами — проверено живым
  тестом), 4 минорных релиза компиляторного перфа (~−5% wall по
  nnethercote survey), LLVM-апгрейды.
- **line-tables-only**: подтверждён уже установленным (profile.dev
  debug = 1) — изменений не требуется.
- **LLD**: rust-lld присутствует в sysroot 1.98; дефолт линковки
  x86_64-linux проверен рабочей сборкой; отдельный override не внесён.
- **НЕ принято** (nightly-only, продакшен-дисциплина): next-gen trait
  solver (nightly с 2026-08-21), Polonius Alpha (2026-08-04),
  -Z annotate-moves. Переоценить после стабилизации; trait-solver
  интересен для TAIT в гексагональных трейтах.

Верификация на 1.98: domain 705 ✓ adapters 246 ✓ cli 176 ✓,
clippy -D warnings clean, RocksDB system-link работает, sccache
перепрогрет (однократно). cargo check -p da-domain 40s → 22s после
прогрева.

## 2026-08-25 — ADR-073 citracer trio in Rust (upstream resilience)

Deep vendor-source research wave 2 (13 repos via codebase-memory; ADR-073).
Implemented for OpenAlex — all in Rust per product direction:

- `da source fetch-openalex <ids|pending> [--no-purge]`:
  negative-cache purge (stale 404/422 self-heal; citation-salvage makes
  old negatives wrong) → bounded-concurrent path-form fetch (8 parallel;
  probe proved doi-filter pipe-OR is HTTP 400 — batch rides concurrency,
  not unsupported API syntax) → per-pid slim cache files (enrich-layer
  consumes unchanged).
- CircuitBreaker in da-adapters (threshold 5, cooldown 120s, success
  resets) — complements action-level retry from the audit wave.
- Toolchain registry: enriched = [fetch-openalex, enrich-layer]; python
  pre-hook removed from the runtime path.

Live: 560×200 (+1 recovered), 85 honest 404 (textbook chapters with
arXiv-shaped DOIs + not-yet-indexed young papers — purge retries them
on every run). Clippy 1.98 new lint fixed in load.rs (as_chunks).

## 2026-08-25 — python-crutch purge wave (runtime path is Rust-only)

Audit of ALL python-in-runtime-path crutches (m451_enrich_openalex.py class).
Classification + action:

REPLACED BY RUST:
- m451_doi_import.py (hand-tuned one-paper DOI mirror) → `da source acquire
  <doi>` handler (route doi_mirror → OpenAlex oa_location → PDF download →
  identity.json). Live: routes + finds PDF; ACM returns honest 403 paywall.
- m451_background_runner.py (per-section LLM summaries) → `da source
  background-run [task|pid]` (da-cli/background_tasks.rs): registry-driven
  policy (background_tasks.yaml), injection scrub (da-domain), models.yaml
  binding, batches ≤16, fingerprint cache, rate limit + daily budget,
  honest failed markers, idempotent rerun ({"cache":1} on retry). LLM
  output parser handles the three 9router shapes: array, json_object
  wrapper, concatenated per-section object stream.

DEAD PORTS REMOVED (functionality already in Rust):
- m451_navigation_build.py (ported: da-domain/navigation.rs)
- m451_summaries_build.py (ported: build-summary action)
- scripts/tests/test_navigation.py (covered by Rust tests)

KEPT (sanctioned patterns):
- m451_pdf_observations.py / m451_pdf_toc_observations.py — reference
  extractors (PyMuPDF observations; Rust stages consume; ADR phase-2)
- m451_textbook_import.py / m451_monograph_import.py — one-time manual
  acquisition tools (4 books imported; Rust textbook-acquire is backlog)
- m451_migrate_article_manifests.py / m451_reference_layers.py /
  m451_summary_eval.py — one-time migration/pilot/eval session tools

Registry honest: doi_mirror_fetch impl=rust, per_section_lazy impl=rust.

## 2026-08-25 — book boundary-tail recovery + margin captions (manual audit)

Manual page-by-page comparison of extracted algorithm sections vs the
original PDF (user: «срафнить с оригиналом — что теряем») surfaced two
loss classes:

1. BOUNDARY TAILS (103/267 section boundaries, CRITICAL): content on the
   next section's first page ABOVE its heading (continuation paragraphs,
   equations — e.g. the core Q-learning update eq 17.10 with ←) was
   dropped by both ranges: anchor-splitting removed it from the next
   section, and page_end=next_page-1 excluded it from the previous one.
   The earlier anchor fix had silently converted misattribution into
   DELETION. Fix: build_pdf_toc_sections reassigns the shared-page
   prefix (up to the next heading anchor, fail-open) to the previous
   section. Live: lost boundaries 103 → 22 (residual = anchors not
   found on imprecise TOC pages); Q-Learning 1449→2454 chars with eq
   17.10 + ← recovered.

2. MARGIN CAPTIONS (173 Algorithm N.N blocks): Tufte-layout algorithm
   captions/footnotes live in the margin zone and never reached parsed
   sections. Now attached as a structured `margin_notes` field per
   section (pages range joined) — 146 sections carry Algorithm captions;
   body reading order stays clean (never merged).

Tests: boundary_tail_reassigned_to_previous_section (eq 17.10 fixture),
margin_notes_attached_structured_not_merged. Clippy 1.98 new lints
fixed en route (Option::zip in ingest.rs).

## 2026-08-25 — ADR-074 WASM MicroLoRA policy modules + roadmap update

ADR-074 proposed: WASM MicroLoRA as swappable FSM policy modules.
Three-layer architecture: FSM core (Rust) → decision points (trait
objects) → policy artifacts (.wasm, sha256-versioned). Five decision
points: grammar_route, batch_admission, concurrency_tune,
retry_policy, summary_depth. Staged W1→W4 behind D136.

Requirements registered: R097-R104 (evidence gate, multi-domain
routing, survey spine, boundary-tail recovery, margin notes,
negative-cache purge, circuit breaker, WASM policy modules).

ROADMAP: M451 (current) + M452-M457 future milestones added:
M452 SONA ReasoningBank extraction, M453 WASM hot-swap infra,
M454 figure-repair loop, M455 temporal query filters,
M456 RRF reranker composition, M457 content_ref vector layer.

continue.md updated with full session state and pending backlog.

## 2026-08-25 — Wave 1: citation materialization + domain fix + manifests

Technical debt wave 1 (audit items D1+D2+D6):

1. D2 DOMAIN FIX: trench-real-analysis identity gained
   coarse_topic_code=math-ho → routing now resolves to [mathematics]
   (was computer_science via default fallback). ads-discrete-structures
   already had math-discrete. algorithms-for-decision-making stays cs-ai.

2. D1 CITATION SALVAGE MATERIALIZED: re-extracted 28 articles that
   carry GROBID-mangled citation IDs. The try_parse_arxiv_locator
   salvage (committed earlier) now has data flowing through it —
   ~1000 citation edges recovered on next graph wave.

3. D6 DOMAIN MANIFEST REFRESH: biology domain gained 3 papers (was 0
   — stale manifest from before multi-domain routing landed; the
   q-bio articles' routing entries existed but no build-extracted
   had run on a biology article since). All five domains now show
   non-zero processed counts.

4. FSM boundary-agent type unification: find_heading_anchor now
   returns Option<(usize, AnchorVerdict)> — Accepted vs Rejected
   verdict alongside the offset. Callers destructure. Dead code
   (unused number-line swallow after FSM classify) removed.
   AnchorVerdict enum lives in pdf_toc (not boundary_agent).

Green: domain 713, adapters 249, cli 176, pytest 142,
validate 645x0, clippy clean.

## 2026-08-25 — Wave 2: figure-repair loop (D3)

D3 from audit: 4 articles had terminal figure_analysis failures
(schema violations, JSON parse, checksum mismatch). The critic
repair contract covered enriched/citations/summaries but NOT
figures — asymmetric gap.

Fix: map_error now classifies LLM output errors as Retryable
(not Terminal): StructuredOutput (schema violation → truncated
quote retry), Json (parse → simplified prompt retry),
ImageContextMismatch (checksum → rebuilt context retry).
Critic manifest reject_triggers gains figure_analysis →
reanalyze_with_diagnostics.

TDD: test written first (red), then map_error extended (green).
The pipeline's existing retry infrastructure (StageExecutionError::
Retryable → RetrySuppressed on cache) handles the rest — no new
pipeline code needed, just honest error classification.

Next graph wave: the 4 failed articles will retry automatically
when their cache entries expire.

## 2026-08-25 — Wave 3: audit-fidelity false-positive fix + .env completeness

1. AUDIT-FIDELITY GATE FIX: 42 boundary-tail "losses" were false
   positives from the Rust gate. The FSM boundary-agent returns
   Rejected verdict for ambiguous anchors (table cells like "Hex
   world" inside Table F.1) — the split doesn't happen, so the tail
   content stays in BOTH sections. The gate was checking only the
   current section for the tail probe → 42 false "lost" tails.
   Python probe (without FSM) confirmed 0 actual losses.

   Fix: the gate now checks BOTH the current AND next section
   (Rejected = content in both). Result: 231/231 recovered ✓.

   Also fixed: accidental `continue` inside a nested `if let` was
   skipping coverage and margin checks (305→117 sections).

2. ENV COMPLETENESS: 9 env vars referenced in Rust code but missing
   from .env.example: DA_DOI_PREFIX_MAP, DA_EXTRACTION_PATTERNS,
   DA_FIGURE_MODEL_BINDING, DA_FIGURE_QUOTE_POLICY, DA_GRAPH_DATA_PATH,
   DA_MULTIMODAL_CONTEXT_SCHEMA, DA_REGRESSION_TOLERANCE,
   NINE_ROUTER_API_KEY, OPENALEX_BASE_URL. All added with defaults.

Green: domain 713, cli 176, application 337, pytest 142,
validate 645x0, clippy clean. Audit-fidelity: 231/231 tails,
91.4% coverage (NEEDS-ATTENTION from <98% threshold — honest,
glossary/table sections are the worst performers).

## 2026-08-25 — Wave 4: Governor test_fidelity_drift + raw backfill + registry honesty

1. GOVERNOR DIMENSION: test_fidelity_drift — ловит класс регрессий
   «код зелёный, но гейт врёт» (wolves 1-3 lessons: 42 false positives,
   coverage drop 305→117, boundary deletion — ни один юнит-тест не
   поймал). YAML contract: watched_sources, fixture_requirements
   (trench: 48-52 sections, 100% tails, ≥85% coverage), 4 checks
   (audit-fidelity pass, sections stable, no-python-in-runtime,
   domain manifests fresh).

2. D4 LEGACY BACKFILL: 603 manifests gained synthetic raw events
   (actor: legacy-implication-backfill, note: parsed proves raw).
   FSM sequences now complete — no gaps in lifecycle_events.

3. D5 TEXTBOOK PLACEMENT: registry impl → rust. Manual workflow
   documented (identity.json + PDF → da source acquire → run).
   Zero reference tools remain in the FSM runtime path.

Governor pre-existing BLOCKED (1764 violations): reverse_adr_audit
(41 subdir patterns + GSD planning projections) + target bloat
(42GB) + graph section stubs — all pre-existing, not from waves 1-4.

## 2026-08-25 — Wave 5: Governor schema fixes + hardcoded URL + textbook manifest

Governor BLOCKED causes addressed:

1. SCHEMA.YAML COMPLETENESS (41→0 subdir violations):
   - data/processed: subdir_pattern += trench-*, ads-*, gnn-ch-*,
     algorithms-* (textbooks don't have dots in names)
   - data/: valid_subdirs += agent_memory, agents, domains, prompts
   - data/sources: valid += acm, doi
   - data/sources/arxiv: valid += article_catalog
   - doc/: valid += research

2. HARDCODED URL: revision_query.rs 'https://example/...' →
   DA_REVISION_ASSET_BASE_URL env var (added to .env.example)

3. TEXTBOOK MANIFEST: algorithms-for-decision-making/textbook_manifest.json
   created (format, parsing approach, TOC bookmarks, grammar profile)

Remaining Governor BLOCKED causes (graph materialization backlog, NOT
from waves 1-5):
- graph_section_text_incomplete: avg 364 chars <500 (M439/M440 backlog)
- graph_enrichment_missing: 0% coverage (enrichment wave needed)
- graph_revision_binding_low: 0/295 papers (revision binding wave needed)
- reverse_adr_audit: 1764 violations ≈ 295 stubs + 885 enrichment +
  295 revision (sum of the three graph gaps above)
- resource_target_bloat: 42GB target/ (cargo clean advisory)
- gsd_planning_projection: 4 old milestones without PLAN tasks

These are structural corpus work items (graph materialization),
not code bugs. They map to existing roadmap M439/M440 backlog.

Green: domain 713, cli 176, app 337, pytest 142, validate 645x0.

## 2026-08-25 — Wave 5 continued: Governor planning projection fixed (6→4 flags)

GSD planning projection flags eliminated:
- STATE.md Active Milestone → M451 (was stale M446)
- M446-M450 marked complete in ROADMAP/STATE/QUEUE
- .gsd/phases/451/ created with S01-PLAN.md (12 tasks in Governor
  _TASK_ROW format: - [x] **TNN**: description)
- PLAN file placed at phase_dir root level (not nested S01/) —
  Governor's glob("*-PLAN.md") is non-recursive

Governor flags: 6→4. Remaining 4 = graph materialization backlog:
- reverse_adr_audit_violations [high]: 1764 (sum of below)
- graph_section_text_incomplete [high]: avg 364 chars <500 threshold
- graph_enrichment_missing [high]: 0% Author/Venue/Topic coverage
- graph_revision_binding_low [high]: 0/295 revision bindings

These are structural corpus work items (M439/M440 backlog), not code bugs.

## 2026-08-25 — Wave 6: pipeline completion + char-boundary fix

1. CHAR-BOUNDARY FIX: enriched_authors.rs f[..1] panicked on
   math italic glyphs (𝒚 = 4 bytes). Fixed with chars().next().
   This blocked enrich-layer for 4 articles (2504.16084 etc).

2. PIPELINE COMPLETION: run-batch on all remaining incomplete
   articles. Result: 0 articles without enriched (was 7),
   0 without summarized, 0 without extracted.

3. Governor: target_bloat flag eliminated (40GB → 13GB after
   incremental cleanup + cargo clean -p stale artifacts).
   resource_target_bloat no longer appears in flags.

Green: domain 713, cli 176, app 337, adapters 249, pytest 142,
validate 645x0, clippy clean.

## 2026-08-25 — M452-mhx138 S01: SONA bridge + sona-analyze CLI

SONA ReasoningBank bridge implemented in Rust:
- features_to_embedding(): CVF trajectory JSON → SONA embedding vector
  [grounding, latency_norm, scrubbed, tokens_norm, inverse_quality]
- build_reasoning_bank(): K-means++ clustering via ruvector-sona
  PatternConfig {max_trajectories: 10000, quality_threshold: 0.3}
- summarize_patterns(): human-readable output

CLI: da source sona-analyze <agent_id>
Live proof on summarizer agent: 55 trajectories → 7 patterns extracted
(avg_quality range 0.50–0.90). Two unit tests pass.

This is ADR-074 Phase W2 Step 1: pattern extraction without LoRA
training. D136 gate still applies to actual weight adaptation.
