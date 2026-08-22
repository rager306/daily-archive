# Pipeline Stage Audit — ADR requirements vs implementation (2026-08-22, M445)

Scope: every pipeline stage from multi-source acquisition to the agent read
plane, checked against binding ADRs. Verdicts: READY / READY-WITH-GAPS /
GAP. Evidence paths are repo-relative. Companion decision trail: D276
(stage→layer binding), D279 (triage DATA), D284 (hot-path indexes).

## 1. Пошаговая таблица конвейера (строки = шаги, колонки = оси анализа)

Порядок строк = порядок исполнения. Обозначения: ✅ готово · ⚠️ готово с пробелом · ❌ пробел.
Подробности по каждой оси — в §2–§6 ниже.

| № | Шаг (вход → выход) | ADR / ACAP-слой | Логика · ветвления | Разрывы / пробелы | FSM · расписание | Агенты · контекст | Хардкод · словари | Вердикт |
|---|---|---|---|---|---|---|---|---|
| 1 | **acquire**: id/URL → ассеты (pdf/html) + SourceIdentity + LoadReport | 056 / source_registered | каталог-hit → есть; miss → script-ensure; pdf и html не смешиваются (разные sha) | ❌ только arXiv + HTML-учебник; W10 натив-fetch не сделан; DOI/PubMed/bioRxiv нет | TaskType::Ingest (реальный) | Source-узел L0; агентам хватает идентичности | source_codes.yaml есть, exercised 2 кода | ❌ GAP |
| 2 | **catalog-bind**: ассет → версия vN + SourceManifestation | 056+065 / source_verified | точная версия → tei_declared fallback → versionless=typed-skip; sha256(manifest)==pdf_hash | ⚠️ 15 статей без версии (no_stamp) | — (часть acquire) | ревизионная идентичность для запросов | — | ✅ |
| 3 | **triage**: pdf → PdfTriageDecision (garbled-страницы) | 053+D279 / — | garbled% > порога (DATA) → block/continue; fail_open env | — | — | page_text_missingness штампуется (агент видит пробелы) | pdf_triage_policy.yaml ✅ | ✅ |
| 4 | **parse**: pdf/html → ParsedArticle (TEI, секции, цитаты, таблицы, формулы) | 053+D204 / parsed | GROBID structural ИЛИ HtmlParser (по формату); координаты фигур; strict-кэш по fingerprint | ⚠️ alias-шероховатость document_context (Q4#4) | TaskType::Reparse | вся полнота статьи живёт тут | fingerprints = идентичность кэша (by design) | ✅ |
| 5 | **document_context** ("pageindex" AST): ParsedArticle → DocumentContextProjection (path_id, порядок, prev/next) | 053 / structured | точечный path_id; порядок в родителе; `~N`-рестарты НЕ становятся родителями; HTML без path-identity → BLOCKED, not_claimed | ❌ постраничный уровень (страница→колонка→блок) не моделируется — только секции | — | навигация агента по статье (единый join-ключ) | governor-контракт html_structure_honesty ✅ | ⚠️ |
| 6 | **figure_detect**: TEI → DetectedVisualArtifact | 060 / parsed(alias) | тип фигуры по TEI-маркерам; без синтетики | — | — | кандидат-доказательства для агента | figure_type_labels.yaml | ✅ |
| 7 | **figure_analysis**: артефакт → типизированный payload (chart/photo/compound) | 060+062 / extracted | chart-native; панели по caption-маркерам; quote-окна ограничены DATA | ⚠️ кросс-страничная непрерывность окон нет | — | богатый мультимодальный контекст | figure_quote_policy, panel_markers ✅ | ✅ |
| 8 | **table_fidelity**: таблицы → ячейки (row/col/digest) | 047 / evidence_linked | конфликты типизированы, никогда не перезаписывают | — | — | табличные факты с доказательством | — | ✅ |
| 9 | **equation_extraction**: MathML → формулы + missingness | 014? — / embedded | неграндед → typed missingness, не тихий пропуск | — | — | формулы как контекст | equation_coverage_contract ✅ | ✅ |
| 10 | **revision_materialization**: ParsedArticle+версия → Paper/PaperRevision/Section/Reference/Source/Category | 058 / structured | [from,to)-интервалы, SUPERSEDES; replay-идемпотентно (589/589 ноль нарушений) | — | — (вызывается MaterializeFsm) | темпоральный граф — база всех запросов | property_indexes (D284) ✅ | ✅ |
| 11 | **extraction_compute**: секции → ExtractedEntity[] | 045 / extracted | rule-based по DATA-словарям; vid=хэш(тип+метка) | ⚠️ 3 статьи zero-mention (не-CS лексика) | TaskType::Extract | сущности — ядро для агента | extraction_patterns.yaml (methods.phrases) | ⚠️ |
| 12 | **extraction_materialization**: сущности → Entity-узлы + MENTIONS/FOUND_IN/EvidenceBundle | 042 / evidence_linked | Entity дедуплицируется по vid (общие узлы между статьями) | ❌ авторы/журналы НЕ сущности (цитатных узлов нет) | — | evidence-цепи для заземления ответов | — | ⚠️ |
| 13 | **embedding**: title+abstract → вектор (1024d) | 040 / embedded | FD_API через env; кэш Vec<f32>; DeterministicEmbedder в тестах | — | TaskType::EmbedStale (замена модели = санкц. процесс) | векторный поиск агента | FD_* env ✅ | ✅ |
| 14 | **summary**: секции → извлекающие саммари по ревизиям | — / summarized | bounded→chained; ревизионно-заземлены | — | — | быстрые выжимки для агента | section_summary_roles.yaml | ✅ |
| 15 | **enrich** (OpenAlex): arxiv_id/DOI → Work/Citation + CITES | 042 / — | нет данных → честный stub (openalex_pending); фильтр по DOI | ⚠️ фильтр `doi:10.48550/arxiv.{id}` — единственный хардкод; Crossref/doi.org нет | TaskType::OpenAlexEnrich | библиографический контекст | ❌ нужны citation_styles/venue/author-словари | ⚠️ |
| 16 | **assurance + checkpoint**: граф → ACAP-вердикт + .sgnap | 055+382 / accepted_checkpoint | replay через физические исходы; валидный раздел applied=0; SnapshotIdentity | — | — | кандидат-evidence; D127 заперт | acap_control_plane.yaml | ✅ |
| 17 | **query / serve**: снапшот → RO-плоскость (RESP, DA.IDENTITY, --verify) | 061 / — | записи fail-closed; digest-проверка в Rust | ⚠️ RuVector-мозг не подключён; REVIEW/OUTPUT-состояния FSM (039) нет; полный remote-стор P1 | — | рабочий цикл агента готов на чтение | — | ⚠️ |

### Сводка по осям (сколько шагов в каком состоянии)

| Ось | ✅ | ⚠️ | ❌ |
|---|---|---|---|
| Логика/ветвления корректны | 15 | 2 | — |
| Мультисорс (источники) | 2 кода | — | W10/DOI/PubMed/bioRxiv |
| Постраничный AST | секции ✅ | окна цитат | страницы/колонки/блоки |
| FSM-адмиссия без хардкодов | да (D276) | alias-шероховатость | — |
| Расписание (TaskType) | 5 живых | GraphWrite отложен by design | — |
| Готовность к агентам (чтение) | read-plane ✅ | мозг/REVIEW-OUTPUT | — |
| Словари DATA | 29 YAML | +5 приоритетных | — |
| Научный контекст | якоря/DOI-on-Work | Crossref/doi.org | сущности-цитаты |


## 2. Scheduling / FSM

- Scheduler TaskTypes: Ingest, Reparse, Extract, EmbedStale, OpenAlexEnrich
  are real tasks with executors/producers (M410–M412, M416–M418);
  **GraphWrite deferred-by-design** (D278, governor guard asserts nothing
  claims it).
- ACAP runtime admission: `StageIdRegistry::acap_layer` is the only
  lookup; orchestrator has **no hardcoded stage match** (D276 verified).
  Missing layer ⇒ typed skip. Governor pins `ledger_layers` — the one
  known wrinkle is the alias-canonicalization gotcha above.

## 3. Meta-prompt FSM readiness (hardcode hunt)

- Prompt surface is DATA: `data/ontology/prompt_tasks.yaml`; prompt/model
  fingerprints are part of cache StageCacheKey (ADR-055 cache contract) —
  prompt changes invalidate only prompt-dependent keys (tested).
- ADR-059 optimizer FSM exists; DSPy/GRPO admission locked until frozen
  eval (D136) — by design, not a gap.
- Hardcodes found (exhaustive scan of adapters/application for URLs):
  - `openalex_adapter.rs`: DOI filter `doi:10.48550/arxiv.{id}` is
    arXiv-specific and string-built → should come from a source-code→DOI-
    prefix map in `source_codes.yaml` when non-arXiv sources land.
  - All service endpoints/env: clean (GROBID_*, FD_*, DA_*).
  - Stage implementation/configuration fingerprints: identity strings by
    design (cache versioning), not config hardcodes.

## 4. Dictionaries / reference data (29 YAML)

Implemented: extraction_patterns (methods.phrases), node_schemas (31),
edge_contracts, node_vocabulary, source_codes, arxiv_categories,
extension_domains, failure_taxonomy, algorithm_params,
catalog_sweep_thresholds, pdf_triage_policy, figure_* (schemas, type
labels, panel markers, quote policy), equation_coverage_contract,
identity_indexes, property_indexes, revision_classification/readiness,
section_summary_roles, snapshot_derivation, context_conservation,
influence_flower_policy, entity_resolution, decision_categories,
acap_runtime_control_plane, prompt_tasks.

**To add for unification/links (priority order):**
1. `citation_styles.yaml` — arXiv/bibTeX/DOI/URL reference-shape parsing
   rules (today only GROBID biblStruct + arXiv-id heuristic).
2. `venue_registry.yaml` + `author_identity.yaml` — canonical venues/
   authors with aliases; needed before citation entities can materialize
   without name-collision chaos.
3. `doi_prefix_map.yaml` (source_code → DOI prefix) — kills the OpenAlex
   hardcode and generalizes enrichment.
4. `method_taxonomy.yaml` — method.phrases → taxonomy parents (lineage
   queries beyond flat strings).
5. Extend `methods.phrases` for math-oc / robotics lexes (3 zero-mention
   alarms: 1203.2295, 2302.06590, 2504.16925).

## 5. Scientific context (citations, references, links)

Present: Reference nodes per paper (81–84 avg) + CITES edges with
anchors (`citations_with_anchors`, revision_anchor_edges), Citation
arxiv_id resolution, OpenAlex Work enrichment (stub-with-pending honest),
DOI carried on Work, evidence chains (ADR-042).

Missing: DOI-first resolution path (doi.org fetch), Crossref fallback,
citation-entity extraction (authors/venues as first-class nodes),
co-citation / citation-context (WHERE in the text it cites — anchor exists
for GROBID markers only), non-arXiv external-id space.

## 6. Anomalies / irregularities register

1. `edge_contracts.yaml` exists in BOTH `data/` and `data/ontology/` —
   drift risk; unify on one path with the other as generated export.
2. `document_context` alias-canonicalization FSM wrinkle (deferred Q4#4).
3. 3 zero-mention extraction alarms (known, backlogged).
4. 15 versionless papers (typed skip; recovery path designed, not built).
5. GraphWrite executor absent by design — but producers that could fill
   it (corpus graph materializer) now exist; revisit D278 when durable
   Samyama server (P1) lands.
6. target/ bloat flag (housekeeping, non-blocking).

## 7. Verdict

Linear spine acquisition→checkpoint: **READY end-to-end on the arXiv
catalog corpus** (proven by the 589-paper zero-failure run + regression
harness). The honest frontier, in order: multi-source acquisition
(DOI/PubMed/bioRxiv), citation/venue/author dictionaries, page-level AST,
agent brain wiring. Nothing in the spine blocks agent read usage today.
