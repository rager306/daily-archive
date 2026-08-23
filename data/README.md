# data/ — научные данные daily-archive

Единственный каталог для всех данных проекта. Каждый подкаталог описан ниже.

## Структура

```
data/
├── articles/               (УДАЛЁН — мигрирован в sources/)
├── processed/<paper_id>/   ← обработанные статьи по стадиям конвейера
│   ├── manifest.json       ← паспорт: статус каждой стадии, хэши, конфиг
│   ├── parsed/article.json ← ParsedArticle (полные тексты секций)
│   ├── context/projection.json ← DocumentContextProjection AST
│   └── extracted/entities.json ← ExtractedEntity[]
│
├── sources/                ← ИММУТАБЕЛЬНЫЕ ассеты (read-only)
│   ├── arxiv/<category>/<id>/
│   │   ├── identity.json / article.json  ← метаданные
│   │   ├── article.v*.json               ← версионированные метаданные
│   │   └── source/*.pdf, *.html          ← оригинальные файлы
│   ├── textbook/<textbook_id>/<chapter>/ ← учебники (по учебнику)
│   │   ├── textbook_manifest.json        ← формат + способ парсинга
│   │   └── source/…
│   ├── blog/, nature/, course/           ← прочие источники
│   └── …
│
├── ontology/               ← словари онтологии (YAML)
│   ├── node_schemas.yaml   ← 31 тип узлов
│   ├── edge_contracts.yaml ← контракты рёбер
│   ├── pipeline_stage_contracts.yaml ← этапы конвейера
│   ├── prompt_tasks.yaml   ← мета-промпты
│   ├── doi_prefix_map.yaml ← DOI префиксы источников
│   ├── identity_indexes.yaml, property_indexes.yaml
│   ├── pdf_triage_policy.yaml
│   └── drafts/             ← черновики правил (до Accept)
│
├── gold_standard/          ← F1 эталон (193 .gold.json файла)
│
├── graph_tasks/            ← семантические задачи (Cypher)
│   ├── gepa_hub.tasks.cypher
│   ├── table_cell_fidelity.tasks.cypher
│   └── …
│
├── graphs/                 ← снапшоты графа (.sgsnap)
│   ├── catalog_materialized.sgsnap  ← полный граф каталога
│   └── gold_corpus.sgsnap           ← граф gold корпуса
│
├── schemas/                ← JSON-схемы каталога статей
│
├── path_config.yaml        ← ЕДИНЫЙ источник путей (meta-prompt принцип)
│
├── extraction_patterns.yaml ← словарь методов/сущностей
├── arxiv_categories.yaml    ← категории arXiv (148+)
├── algorithm_params.yaml    ← параметры алгоритмов
├── catalog_sweep_thresholds.yaml ← пороги свипа каталога
├── extension_domains.yaml   ← расширенные домены
├── failure_taxonomy.yaml    ← таксономия отказов
├── node_vocabulary.yaml     ← словарь типов узлов
└── source_codes.yaml        ← коды источников (arxiv, textbook, blog…)
```

## Принципы

1. **sources/ — read-only.** После загрузки файлы не меняются.
2. **processed/ — пересчитываемое.** Можно удалить и перестроить из sources/.
3. **manifest.json — паспорт статьи.** Отслеживает каждую стадию конвейера.
4. **path_config.yaml — единый источник путей.** Код не содержит hardcoded paths.
5. **Governor контролирует структуру.** Нарушения флагаются автоматически.

## Жизненный цикл статьи

```
sources/arxiv/cs-cl/2507.19457/source/2507.19457.pdf
    ↓ parse (GROBID structural)
processed/2507.19457/parsed/article.json
    ↓ context (AST)
processed/2507.19457/context/projection.json
    ↓ extract (rule-based + dictionaries)
processed/2507.19457/extracted/entities.json
    ↓ enrich (OpenAlex)          [pending]
    ↓ materialize (Samyama FSM)
graphs/catalog_materialized.sgsnap
    ↓ verify (ACAP replay)
verified: PASS
```

## Не класть сюда

- Кэш конвейера → `artifacts/cache/` (пересчитываемо)
- Отчёты прогонов → `artifacts/reports/`
- Временные файлы → `/tmp/`
- Python-era данные → `archive/python-era-cold-*/`

## Governor проверяет

```bash
PYTHONPATH=scripts python3 -m governor --phase preflight --check-only
```

Флагает:
- `data_integrity_hardcoded_url` — URL в коде вместо env
- `data_integrity_nested_dir` — матрёшки типа article_catalog/article_catalog
- `graph_section_text_incomplete` — секции < 500 chars (стабы)
- `graph_enrichment_missing` — нет Author/Venue/Topic узлов
- `textbook_manifest_required` — учебник без манифеста
