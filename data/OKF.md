# data/ — научные данные

```
data/
├── sources/                  ← исходные ассеты (read-only)
│   ├── arxiv/<category>/<id>/source/*.pdf
│   ├── textbook/<textbook_id>/<chapter>/
│   ├── blog/, nature/, course/
├── processed/<paper_id>/     ← обработанные статьи (стадии конвейера)
│   ├── manifest.json         ← паспорт статьи
│   ├── parsed/               ← ParsedArticle
│   ├── context/              ← AST проекция
│   └── extracted/            ← сущности
├── ontology/                 ← YAML словари онтологии
│   └── drafts/               ← черновики правил
├── gold_standard/            ← F1 эталон (.gold.json)
├── graph_tasks/              ← Cypher задачи
├── graphs/                   ← .sgsnap снапшоты графа
├── schemas/                  ← JSON-схемы каталога
├── path_config.yaml          ← единый источник путей
└── *.yaml                    ← root-словари (extraction_patterns, …)
```

Подробности: doc/adr/ADR-067-normalized-data-layout.md
