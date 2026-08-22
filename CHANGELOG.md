# Changelog

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