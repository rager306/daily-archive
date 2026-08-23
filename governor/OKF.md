# governor/ — мета-слой контроля (отделён от научных данных)

```
governor/
├── fixtures/                 ← positive/negative JSON пары для Governor чеков
│   ├── acap_runtime_control_plane/   ← 8 файлов (4 пары)
│   ├── equation_coverage_contract/   ← 2 файла
│   ├── html_structure_contract/      ← 2
│   ├── navigation_contract/          ← 2
│   ├── promotion_contract/           ← 2
│   ├── read_only_agent_contract/     ← 2
│   ├── table_provenance_contract/    ← 2
│   ├── temporal_wire_contract/       ← 2
│   ├── visual_lineage_contract/      ← 2
│   └── prompt_eval/                  ← YAML правила prompt evaluation
│
└── (Python код в scripts/governor/)
```

Контрактные YAML: scripts/governor/checks/*.yaml (15 файлов).
Фикстуры читаются Governor trajectory_service при --check-only.
