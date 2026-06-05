---
title: QuantMind Architecture Pattern Research Direction
status: research-direction
slice: S04
milestone: M033-732r1t
---

# S04 Research Direction: QuantMind Architecture Pattern Study

## Вывод

`LLMQuant/quant-mind` — это перспективный, но ещё незрелый Python-фреймворк для извлечения структурированных знаний из финансово-исследовательских материалов. В README он описывается шире — как платформа, превращающая papers/news/blogs/reports в queryable knowledge base и semantic knowledge graph. По фактическому коду текущий рабочий центр проекта уже: **извлечение структуры из научных статей**, прежде всего через `paper_flow`.

Оценка для M033: **хороший архитектурный прототип / research framework**, но **не готовая production RAG/Knowledge Base система**. Его стоит использовать как основу для схем, пайплайна извлечения и PageIndex-подобного дерева документа; хранение, полноценный retrieval, graph layer, memory и e2e production-поток ещё в работе.

## Что реально реализовано

### 1. `paper_flow`: извлечение Paper-структуры из статьи

Главная реализованная функция — `paper_flow`. Она принимает один из вариантов `PaperInput`, получает сырой контент, конвертирует его в markdown/plain text, затем запускает `Agent(output_type=Paper)` и возвращает типизированный объект `Paper` как `TreeKnowledge`.

Поддерживаемые входы:

| Вход | Статус |
|---|---|
| arXiv ID / arXiv URL | реализовано |
| HTTP URL, PDF или HTML | реализовано |
| локальный PDF / HTML / Markdown / plain text | реализовано |
| raw text | реализовано |
| DOI | объявлен в типах, но фактически не реализован |

DOI сейчас явно выбрасывает `NotImplementedError`, потому что отсутствует fallback через Unpaywall/OA PDF resolver.

### 2. Fetch/format слой

Есть отдельный `preprocess.fetch` слой: async-функции получают байты и метаданные, но не парсят и не вызывают LLM. Для arXiv используется `arxiv` Python lib для метаданных и `httpx` для скачивания PDF. Для произвольных URL есть размерный лимит `max_bytes=50_000_000`, timeout и нормализация `content-type`.

PDF сейчас извлекается через PyMuPDF как plain text, без восстановления структуры, таблиц, формул и markdown-иерархии; в коде прямо указано, что более качественные движки вроде `marker-pdf` предполагаются как будущий option. HTML переводится в markdown через `trafilatura`, включая boilerplate stripping и сохранение таблиц.

### 3. Knowledge-схемы

Сильная часть проекта — Pydantic-схемы знаний. Базовый слой содержит provenance (`SourceRef`), extraction metadata, `as_of`, confidence, citations, tags и обязательный контракт `embedding_text()`.

В проекте есть три концептуальные формы знаний:

| Форма | Назначение | Статус |
|---|---|---|
| `FlattenKnowledge` | атомарные карточки: news, earnings, factor, thesis, paper card | реализовано как базовая форма |
| `TreeKnowledge` | иерархические документы: papers, filings, transcripts | реализовано |
| `GraphKnowledge` | связи между объектами: paper-cites-paper, factor lineage, news-mentions-ticker | placeholder, не реализовано |

`TreeKnowledge` особенно важен: он описан как PageIndex-style retrieval, где агент читает root summary, summaries детей, выбирает ветку, углубляется и lazy-loads leaf content; embeddings используются только как coarse pre-filter, а не замена reasoning. Это хорошо ложится на задачу оптимизации RAG по иерархии документа.

`GraphKnowledge` пока заблокирован как design-intent placeholder: наследование от него выбрасывает `NotImplementedError`.

### 4. Batch processing

`batch_run` — единственная batch/concurrency-примитива MVP. Она запускает flow по списку входов с bounded concurrency, собирает ошибки, поддерживает `on_error="skip"`/`"raise"` и `on_progress`. Важное ограничение: `memory=` в batch запрещён, чтобы избежать race hazards в stateless MVP.

### 5. `magic.py`: natural language → typed input/config

Есть resolver: `resolve_magic_input()` интроспектирует сигнатуру flow, строит schema prompt по Pydantic-типам input/cfg и через lightweight agent возвращает `(input_obj, cfg_obj)`. В prompt есть полезное ограничение: не выдумывать file paths или URLs, а при отсутствии конкретного идентификатора предпочитать `RawText`.

## Текущая архитектура

Фактический package tree в `quantmind/` сейчас состоит из:

```text
quantmind/
├── configs/
├── flows/
├── knowledge/
├── preprocess/
├── utils/
├── __init__.py
└── magic.py
```

Это означает, что в текущем master нет production-ready `storage/`, `mind/`, `retrievers/`, `rag/` или `mcp/` слоя. Философия новой архитектуры — не писать собственный agent runtime, а использовать OpenAI Agents SDK. Проект позиционируется как domain library поверх `openai-agents`, фокусируясь на финансовых схемах, pipeline-ах, memory patterns и knowledge formats, а не на инфраструктуре агентов.

## Качество инженерии

Плюсы:

1. **Хорошее разделение слоёв.** `preprocess` не зависит от `knowledge/flows`, `configs` отделены, `flows + magic` — верхний слой. Это зафиксировано import-linter контрактами в `pyproject.toml`.
2. **Нормальный tooling.** Есть `ruff`, `basedpyright`, `import-linter`, `pytest`, `pytest-cov`, `pytest-asyncio`, coverage gate `--cov-fail-under=75`.
3. **CI verify pipeline есть.** `scripts/verify.sh` проверяет форматирование, lint, type check, import contracts и pytest coverage.
4. **Тесты не декоративные.** Например, `test_paper.py` покрывает dispatch по content-type, arXiv/http/local/raw/DOI branches, extra tools, guardrails, run hooks, model settings и memory placeholder.

Минусы / риски:

1. **Документация частично опережает код.** README говорит про semantic knowledge graph, embeddings, DeepResearch, RAG, Data MCP и broad multi-source ingestion, но сам README признаёт, что это long-term vision, not current capabilities.
2. **Storage layer описан в docs, но отсутствует в текущем package tree.** Это выглядит как legacy/proposed design, не как реализованный слой.
3. **Есть следы устаревшей документации.** `docs/EMBEDDINGS.md` импортирует `quantmind.config` и `quantmind.llm`, тогда как `pyproject.toml` говорит, что transitional packages `config/`, `flow/`, `llm/`, `models/` удалены и защищены import-linter контрактом.
4. **Version mismatch.** В `pyproject.toml` версия проекта `0.2.0`, а в `quantmind/__init__.py` стоит `__version__ = "0.0.1"`.
5. **Python version mismatch.** README badge указывает Python 3.8+, но `pyproject.toml` требует Python `>=3.10`.
6. **Некоторые config поля пока не подключены.** `BaseFlowCfg` содержит `timeout_seconds`, `output_dir`, `memory_dir`, token/cost guardrails и archive flags, но текущий `run_with_observability()` реально прокидывает в SDK в основном tracing metadata и `max_turns`; memory идёт как placeholder, а archive — no-op stub.

## Что это даёт для базы научных статей

Для daily-archive полезны три идеи:

1. **TreeKnowledge вместо плоских чанков.** Статья представляется как дерево секций: root → sections → subsections → leaves. Это лучше обычного chunking, если нужен PageIndex-подобный navigation/reasoning по структуре документа.
2. **Разделение “полный документ” и “summary card”.** `Paper` — дерево, `PaperKnowledgeCard` — flat карточка для tagging/filtering/dashboard, deep questions идут к дереву.
3. **Строгая provenance-модель.** `Citation`, `SourceRef`, `ExtractionRef`, `as_of`, `confidence` и `content_hash` закладывают основу для auditability и дедупликации.

Текущий проект сам по себе не решает production-пайплайн:

| Компонент | Состояние |
|---|---|
| ingest arXiv/PDF/HTML | частично реализовано |
| структурирование статьи в дерево | реализовано через LLM agent |
| storage | описан, но в текущем коде отсутствует |
| vector index | не реализован как рабочий слой |
| retrieval API | не реализован |
| graph relations | placeholder |
| memory | placeholder / open trajectory |
| evaluation | unit tests есть, retrieval/extraction quality evals не видно |
| deterministic non-LLM indexing | нет; core extraction использует LLM agent |

## Как применять

### Вариант A — использовать как reference architecture

Наиболее безопасно: взять идеи и схемы, но не завязываться на проект как production dependency.

Что взять:

```text
Paper → TreeKnowledge
PaperKnowledgeCard → flat summary/index card
SourceRef / Citation / ExtractionRef
preprocess.fetch + preprocess.format separation
batch_run concurrency pattern
magic resolver pattern
```

Что заменить / дописать:

```text
PDF parser: PyMuPDF → Marker / Docling / Nougat / GROBID / custom scientific parser
Storage: filesystem + SQLite/Postgres + vector index
Retrieval: tree traversal + embeddings prefilter + reranker
Quality evals: golden papers, citation accuracy, section hierarchy accuracy
DOI resolver: Crossref + Unpaywall
Dedup: content hash + arXiv/DOI canonical IDs
```

### Вариант B — попробовать как библиотеку для paper extraction

Минимальный запуск возможен через `uv pip install -e .` и `OPENAI_API_KEY`, но для M033/S04 это не нужно: `paper_flow` вызывает OpenAI Agents SDK и может тянуть arXiv/network. Это выходит за рамки static pattern study.

## Итоговая оценка

| Критерий | Оценка |
|---|---:|
| Архитектурная идея | 8/10 |
| Чистота нового кода | 7/10 |
| Готовность как библиотека | 5/10 |
| Готовность как production RAG/KB | 3/10 |
| Полезность для PageIndex/иерархического retrieval | 7/10 |
| Документационная согласованность | 4/10 |
| Риск интеграции сейчас | средний/высокий |

## Рекомендация S04

Использовать `quant-mind` как источник архитектурных паттернов и, возможно, как экспериментальный модуль для extraction of papers в будущем. Не принимать его как готовую платформу knowledge graph/RAG. Для daily-archive особенно ценно направление `TreeKnowledge + PaperKnowledgeCard + provenance`; storage, retrieval, parser quality и eval layer нужно проектировать отдельно или дописывать поверх.

S04 should therefore be a **static architecture pattern study**, not a runtime integration probe. No OpenAI/API/network run is required.
