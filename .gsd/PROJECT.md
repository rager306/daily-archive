# Project: daily-archive

## Что это

daily-archive — локальная Universal Knowledge Base для научных и технических источников. Базовая архитектура — **7-слойный typed knowledge pipeline**:

Source → Parser → Structure → Extraction → Graph → Review → Agents

Сейчас проект строит доказуемые, fail-closed цепочки перед любым graph import: каталог источников, acquisition, parser/chunk diagnostics, typed extraction contracts, review gates, write-path governance, readiness probes и metadata-only validation artifacts.

## Текущая стадия pipeline

**Стадия: post-M256 + Wave B extraction quality path (import закрыт).**

Wave A closed. Wave B stamp open (`artifacts/wave-b/human_go.json`, D124). Gold↔hybrid join **10** body-grounded papers; oracle + lexical floor **entity/relation F1=1.0**; header-priority constrained select (no LLM) **~0.90 entity / ~0.70 relation** on n=10 (was 1.0 on n=6) via `verify_wave_b_constrained_select.py`. Structured extract context (outline/sections/candidates/pageindex-bridge) default for LLM pilots; free-form invent still weak. **Not** DSPy optimizer; **not** import. LLM surface: 9router (agnes default / MiniMax quality / grok fallback); optional summary stage composition-default off.

**Live FalkorDB driver и production import выключены**, пока не будет отдельного явного Wave D readiness milestone.

Закрыто с M209 (high-signal):

- M209–M212: graph-data readiness package; parser body routes; hybrid sidecar offline inject
- M213–M216: hybrid body 10→20, catalog coverage, readiness handoff (import закрыт)
- M217–M221: GROBID TEI header/citations ETL; scholarly metrics; citation inventory + review policy
- M222–M223: multi-source inventory; GNN textbook HTML catalog; company_blog HTML proof; Stanford PDF capture; catalog index **~230**
- M224–M229: non-LLM preprocess (clean/quality/HTML main/language/outline/fingerprint/spans/windows) + **ADR-036**
- M230–M232: optional YAKE inject (composition only; language-aware; cleaned body)
- M233–M235: non-gating `preprocess_rollup` на hybrid и non_arxiv; fail-closed empty default
- M236–M238: multi-tree import-hold inventory; default package roots; ADR closeout
- M239–M240: `scripts/verify_import_hold_inventory.py` + pre-commit hook `m239-import-hold-inventory`

Это значит:

- hybrid scholarly + multi-source custody paths usable для оператора **без** authorization на import;
- `pilot_eligible ≠ import_eligible`; SafetyFlags fail-closed;
- non-LLM preprocess и optional YAKE — enrichment only; **не** двигают `proof_pass` / `handoff_verdict` / import;
- import-hold inventory + pre-commit держат `import_eligible = True` / `graph_writes_allowed = True` вне пакета;
- **следующий горизонт — не «ещё hold-polish»**, а осознанный выбор: реальная data readiness без import, operator path, пауза волны, или отдельный import/Falkor milestone.

## Архитектура сейчас

### Основная модель

- **7-layer pipeline** остаётся основной end-to-end моделью.
- **Hexagonal + onion overlay** принят для кода: domain/application/infrastructure, ports только там, где есть реальный seam, миграция или test contract.
- **Graph target**: FalkorDB typed graph schema остаётся целевой production graph DB.
- **Intermediate graph**: NetworkX используется как безопасный промежуточный/проверочный слой.
- **LadybugDB**: исторический/retiring контекст, не целевой путь для новых production decisions.
- **Review gate**: fail-closed, без soft warnings вместо gates.
- **Write-path governance**: production writes/imports требуют явного разрешения, staged validation и независимых evidence artifacts.

### Ключевые safety invariants

Эти инварианты остаются обязательными:

- `graph_writes_allowed=false` до явного future milestone approval;
- `schema_migration_allowed=false` до отдельного schema migration milestone;
- `import_eligible=false` для readiness artifacts;
- no direct extractor → graph write;
- deterministic/statistical preprocessing before LLM stages;
- stable IDs and EvidencePath discipline;
- staged validation before scale claims;
- retired `arxiv_archive.graph_readiness_review` не восстанавливать; текущий модуль — `research_graph.infrastructure.graph.readiness.review`.

## Что уже проработано

### Source/catalog/corpus

- Канонический article catalog существует и используется как источник истины.
- Есть правила для arXiv ingestion в canonical catalog.
- Проработаны selection, coverage, provenance, catalog safety и migration evidence для R024/R0xx волн.

### Parser/structure/chunking

- Parser replay и chunking проверяются на multiple corpus batches.
- Есть coverage reports и parser/chunking regression tests.
- Bounded repair и chunk import contracts существуют как review-only / validation artifacts.

### Extraction/LLM boundary

- Сохранён statistical-first принцип.
- LLM JSON boundary изолирован; provider-specific logic вынесен в config/helpers.
- MiniMax/GLM hot-pluggable (M201); reviewed metrics/ablations (M202) на fixture harness.
- Live fleet extraction quality всё ещё требует real-corpus staged gates.

### Parser / hybrid / multi-source

- M213/M214 hybrid body gates (10→20) live-proven; M215 catalog coverage; M216 readiness handoff + scholarly_wrapper (import-blocked).
- M217 pure GROBID TEI header/citations ETL; M218–M219 scholarly metrics on batch gate; M220 citation inventory; M221 review policy (`ready_for_human_review`, import false).
- M222–M223 multi-source: catalog inventory; GNN textbook HTML register; company_blog via `universal_source` (not hybrid TEI); nature metadata-only; Stanford PDF captured.
- Body routes: `html_native` | `mdconverter` | `fitz_offline` | `hybrid` | `hybrid_deferred` | `unavailable`.
- Live GROBID/ODL only on composition root; `hybrid_claimed_success` requires body evidence; sidecars candidate-only (ADR-008/009).
- Sidecars: `.docker/docker-compose.yml` (GROBID :8070), host `opendataloader-pdf` (`uv sync --extra hybrid`).
- PageIndex is format-agnostic full-text structure (not PDF-native TEI tree); hybrid PDF supplies body text for the same builder.

### Graph/readiness

- FalkorDB остаётся production target, но **live driver и production import не включены** (явный hold до data readiness).
- GraphReadPort + NetworkX/DisabledFalkor no-write path (M203/M206).
- Promotion boundary: `pilot_eligible ≠ import_eligible` (M204).
- Disposable write pilot isolated under `pilot_write/` (M205) — не production activation.
- M209 composition root готовит graph-data readiness package без Falkor.

### Governance/operations

- M198 дал финальный readiness package:
  - `m198.readiness_evidence.v1`
  - `m198.readiness_evidence_index.v1`
  - `m198.operator_diagnostics.v1`
  - `m198.readiness_report.v1`
  - `m198.readiness_rehearsal.v1`
  - `m198.smoke_parity_audit.v1`
  - `m198.disabled_backend_safety.v1`
  - `m198.validation_package.v1`
  - `m198.gitnexus_impact_gates.v1`
- R076, R077, R078 validated.
- Post-S17 GitNexus full rebuild: 47,196 nodes, 65,108 edges, 1,000 clusters, 300 flows.

## Что ещё надо проработать

### Перед production graph import

- Отдельно спланировать production import milestone.
- Перед любыми queue dependency edits выполнить exact GitNexus impact; текущий seam `UniversalKBQueue._dependencies_satisfied` считается HIGH/out-of-scope.
- Спроектировать/подтвердить schema migration path.
- Утвердить explicit import eligibility promotion rules.
- Проверить write-path governance не только на artifacts, но и на реальном import path.

### Non-LLM article preprocess (next)

- Deterministic body clean + HTML main-content + body_quality diagnostics before YAKE/PageIndex (vendor lessons: quant-mind clean/outline, yago hygiene, xberg positions — patterns only, no runtime swap).
- OutlineSignals / language / content fingerprint as structure/stat enrichment.
- Preprocess never opens import; scholarly vs web quality profiles.

### Extraction quality

- Staged metrics/ablations exist (M202); live fleet quality still needs real-corpus gates.
- No optimizer/DSPy production claims without verified metrics.

### Graph layer

- Довести FalkorDB backend migration и graph operators O1-O6 до production-ready состояния.
- Проверить schema evolution, relation constraints, idempotency и rollback/failure surfaces.
- Сохранить NetworkX как validation/intermediate path, пока FalkorDB path не доказан.

### Agents layer

- SymFSM/agent integration остаётся концептуально заданным, но требует отдельной разработки.
- Agent actions должны оставаться bounded, observable, fail-closed.

### Test hygiene

- Full `pytest --collect-only` is clean: **3178 tests collected** (post-M223).
- M058 optional pilots (`plotextractor` / `marker`) use `pytest.importorskip` and skip when deps absent — not collection errors.
- Prefer targeted suites for milestone closeout; full-suite green claims still need intentional runs, not only collect.

## Тестовое покрытие и verification surfaces

### Инвентарь тестов

- Pytest collect-only (post-M223): **3178 tests collected**, 0 collection errors.
- Hybrid/multi-source suites: `test_m213_*` … `test_m223_*` plus scholarly/citation/inventory modules.
- Governance/no-write/onion ratchets remain required on new application/composition code.

### Что реально проходило в финальной M198 валидации

M198 closeout verification:

- 82 selected tests passed.
- Ruff passed.
- Pyrefly: 0 errors.
- GitNexus detect_changes: LOW, affected_count=0.
- GSD milestone validation: pass.

Покрытые классы verification:

| Class | Current coverage |
|---|---|
| Contract | Readiness schemas, evidence contract, package schema tests |
| Integration | S13 rehearsal, S16 validation package, S17 runbook compatibility |
| Operational | Operator diagnostics, readiness report, runbook, final closeout evidence |
| Governance | M195-M198 no-write/import ratchets, GitNexus impact gates |
| Static | Ruff and Pyrefly in final readiness scope |
| UAT/artifact | Slice UAT artifacts plus final validation evidence |

### Важное ограничение

Финальное `82 passed` — это targeted readiness/governance suite, а не утверждение, что весь исторический repository suite полностью green. Для полного green claim сначала нужно устранить/классифицировать legacy collection errors.

## Текущая поза roadmap

| Область | Состояние |
|---|---|
| Архитектура (hex/onion, ADR) | Достаточно стабильна для текущей работы |
| Source/catalog/parser/chunking | Существенное staged coverage |
| Non-LLM preprocess + YAKE boundary | **Закрыто** M224–M235 / ADR-036 |
| Import-hold inventory + operator gate | **Закрыто** M236–M240 |
| Extraction quality | Нужны metrics/ablations до production claims |
| Graph readiness | Validate-only есть; **production import заблокирован** |
| Backend writes/imports | Явно выключены |
| Operator readiness | M198 package/runbook + import-hold verify/pre-commit |
| Agents/SymFSM | Будущая разработка |

## Ближайший следующий шаг

**Import и live Falkor не включать**, пока волны A–C не дадут evidence. SafetyFlags не «смягчать».

### Уже сделано (не переоткрывать без причины)

- M202–M223: hybrid scholarly + multi-source operator path (import закрыт).
- M224–M240: non-LLM preprocess, ADR-036, YAKE composition inject, dual-wire rollup, import-hold inventory, operator verify, pre-commit guard.
- Статистика-first: YAKE/PageIndex — core (ADR-024 + ADR-036).

### Длинный горизонт ETL → ~99% (волны; import в конце)

Цель ~99% — **operator-honest end-to-end data preparation + measured quality + explicit write gate**, не «включить Falkor наугад».

| Волна | Фокус | Критерий готовности волны | Import? |
|---|---|---|---|
| **A. Data readiness** | Catalog↔body/hybrid coverage, continuity audit, preprocess metrics 10→20 | Измеримый coverage report; gaps classed; handoff/readiness на live artifacts | **Нет** |
| **B. Extraction quality** | Real-corpus metrics/ablations до optimizer | Staged extraction quality gates; no DSPy claims without metrics | **Нет** |
| **C. Structure graph-ready** | Chunk/structure quality vs promotion contract | Documented graph-ready fractions; still no write | **Нет** |
| **D. Explicit import pilot** | Fail-closed promotion + optional disposable pilot | Evidence package + GitNexus impact; SafetyFlags only if approved | **Только явно** |
| **E. Agents** | Read-only / SymFSM over ready graph | After D | After D |

**Порядок обязателен:** A → B → C → (D только по go) → E.  
**Неподключённый функционал** (nature fulltext, live Falkor, fleet DSPy, agentic collapse) — не подключать «чтобы % вырос»; только по wave criteria.

### Сейчас: Wave B extraction quality (post gold-debt + constrained select)

1. **Done M241–M256:** Wave A closed; stamp open; statistical hybrid; gold↔hybrid join; lexical floor.
2. **Done (session):** body-ground gold for 1611/2109; candidate coverage 12/12; oracle F1=1.0; grounding audit; structured extract context + pageindex-bridge; GEPA offline full-set entity F1=1.0; **header_priority constrained select ~0.90/0.70 on n=10** (1.0/1.0 on n=6; no LLM).
3. **Next B:** constrained LLM select (`candidate_id` only) vs header baseline on n=10; full-6 structured vs raw ablation; optional GEPA reflection only if constrained LLM beats header without inventing labels; **no DSPy optimizer** until metrics demand; **no import**.
4. Import/Falkor не открывать до Wave D + evidence. **Не переспрашивать go на A/B.**

```bash
uv run python scripts/verify_wave_b_gate.py
uv run python scripts/verify_wave_b_gold_body_grounding_audit.py
uv run python scripts/verify_wave_b_gold_hybrid_metrics.py
uv run python scripts/verify_wave_b_constrained_select.py
uv run python scripts/verify_wave_b_gold_hybrid_constrained_pilot.py
uv run python scripts/verify_import_hold_inventory.py
```

### Операторские команды (hold)

```bash
uv run python scripts/verify_import_hold_inventory.py
uv run python scripts/verify_onion_layering.py
```
