# ontology/ — словари и контракты онтологии

Все YAML-словари проекта. DATA not code — редактируй здесь, не в Rust.

## Основные

| Файл | Что описывает |
|---|---|
| `node_schemas.yaml` | 31 тип узлов графа (Paper, Entity, Section…) |
| `edge_contracts.yaml` | Контракты рёбер (какие label могут соединяться) |
| `pipeline_stage_contracts.yaml` | Этапы конвейера + ACAP слои |
| `prompt_tasks.yaml` | Мета-промпты для LLM |
| `doi_prefix_map.yaml` | Source code → DOI prefix |
| `pdf_triage_policy.yaml` | Политика triage PDF |
| `identity_indexes.yaml` | Индексы идентичности |
| `property_indexes.yaml` | Индексы свойств |

## Политики

| Файл | Что регулирует |
|---|---|
| `context_conservation_policy.yaml` | Сохранение контекста |
| `snapshot_derivation_policy.yaml` | Производные снапшотов |
| `revision_readiness_policy.yaml` | Готовность ревизий |
| `figure_quote_policy.yaml` | Окна цитирования фигур |
| `finalize_policy.yaml` | Финализация |

## Черновики

`drafts/` — правила до Accept (не активны).
