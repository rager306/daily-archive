# Project: daily-archive

## Что это

daily-archive — локальная Universal Knowledge Base для научных и технических источников. Базовая архитектура — **7-слойный typed knowledge pipeline**:

Source → Parser → Structure → Extraction → Graph → Review → Agents

Сейчас проект строит доказуемые, fail-closed цепочки перед любым graph import: каталог источников, acquisition, parser/chunk diagnostics, typed extraction contracts, review gates, write-path governance, readiness probes и metadata-only validation artifacts.

## Текущая стадия pipeline

**Стадия: post-M208 graph-data readiness / import-blocked validation.**

M202–M208 закрыты. Контракты extraction metrics, Falkor no-write compatibility, promotion boundary, disposable write pilot, retrieval quality, universal HTML source_kind и read-only SymFSM (O1–O6) существуют на fixture/contract уровне. **FalkorDB live driver не активируется**, пока не отработана цепочка данных и readiness.

M209 добавляет:

- continuity audit по 7 слоям pipeline;
- no-write composition root `run_graph_data_readiness_pipeline` (load → structure → candidate → projection → promotion);
- metadata-only `GraphDataReadinessPackage` с `import_eligible=false` и `graph_writes_allowed=false`.

Это значит:

- operator path для graph-data readiness собирается без live Falkor;
- production graph import всё ещё НЕ включён;
- следующий горизонт — catalog coverage, real-corpus quality gates (10→20→week), затем только при явной готовности — live graph backend.

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

### Parser / hybrid

- Body routes: `html_native` | `mdconverter` | `fitz_offline` | `hybrid` | `hybrid_deferred` | `unavailable`.
- M211–M212 parser body routes + hybrid sidecar offline inject закрыты. Live GROBID/ODL подключаются только на composition root (`hybrid_live_ports` / `article run --mode hybrid`); `hybrid_claimed_success` только с body evidence; import/writes fail-closed.
- Sidecars: `.docker/docker-compose.yml` (GROBID :8070), host `opendataloader-pdf` (`uv sync --extra hybrid`).

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

### Extraction quality

- Довести staged metrics/ablations для extraction quality.
- Зафиксировать benchmark fixtures и failure taxonomy для typed entities/relations.
- Не включать optimizer/DSPy-style claims без verified metrics.

### Graph layer

- Довести FalkorDB backend migration и graph operators O1-O6 до production-ready состояния.
- Проверить schema evolution, relation constraints, idempotency и rollback/failure surfaces.
- Сохранить NetworkX как validation/intermediate path, пока FalkorDB path не доказан.

### Agents layer

- SymFSM/agent integration остаётся концептуально заданным, но требует отдельной разработки.
- Agent actions должны оставаться bounded, observable, fail-closed.

### Test hygiene

- Общий pytest collect сейчас не является clean: collect-only видит 2,905 tests collected и 2 legacy collection errors в `tests/test_m058_s01.py` / `tests/test_m058_s02.py` из-за отсутствующего `marker` module.
- Это не блокировало M198, потому что финальная валидация была targeted, но перед full-suite claims нужно либо восстановить optional legacy dependency, либо зафиксировать quarantine/skip policy.

## Тестовое покрытие и verification surfaces

### Инвентарь тестов

- Test files в `tests/`: 311.
- M198-specific test files: 16.
- Governance/ratchet/guardrail-style test files: 13.
- Graph-related test files: 23.
- Corpus/parser/catalog/ingest/PDF-related test files: 35.
- Pytest collect-only: 2,905 tests collected, 2 legacy collection errors.

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

## Current roadmap posture

| Area | State |
|---|---|
| Architecture crystallization | Complete enough for current work |
| Source/catalog/parser/chunking | Substantial staged coverage exists |
| Extraction quality | Needs metrics/ablations before production claims |
| Graph readiness | Validate-only readiness complete; production import blocked |
| Backend writes/imports | Explicitly disabled |
| Operator readiness | M198 package/runbook complete |
| Agents/SymFSM | Future development |

## Immediate next best milestone

The next milestone should not “turn on imports” directly. Safer sequence:

1. repair or quarantine legacy pytest collection errors;
2. design production import activation contract;
3. run GitNexus impact gates for queue/backend/import seams;
4. rehearse schema migration/import eligibility promotion in validate-only mode;
5. only then consider controlled write-enabled pilot.
