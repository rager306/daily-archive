# Changelog

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
