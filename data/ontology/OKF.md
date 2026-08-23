# ontology/ — словари и контракты онтологии

```
ontology/
├── node_schemas.yaml                 ← 31 тип узлов
├── edge_contracts.yaml               ← контракты рёбер
├── pipeline_stage_contracts.yaml     ← этапы конвейера + ACAP
├── prompt_tasks.yaml                 ← мета-промпты LLM
├── doi_prefix_map.yaml               ← source → DOI prefix
├── pdf_triage_policy.yaml            ← triage PDF
├── identity_indexes.yaml             ← индексы идентичности
├── property_indexes.yaml             ← индексы свойств
├── entity_resolution.yaml            ├── разрешение сущностей
├── figure_quote_policy.yaml          │
├── finalize_policy.yaml              ├── политики
├── revision_readiness_policy.yaml    │
├── snapshot_derivation_policy.yaml   │
├── context_conservation_policy.yaml  │
├── section_summary_roles.yaml        │
├── influence_flower_policy.yaml      │
├── decision_categories.yaml          ┘
└── drafts/                           ← черновики (до Accept)
