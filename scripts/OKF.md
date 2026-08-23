# scripts/ — скрипты сборки, проверки и миграции

```
scripts/
├── governor/             ← Python пакет Governor CLI (гексагональный)
│   ├── domain/           ← типы (Verdict, DriftFlag)
│   ├── ports/            ← интерфейсы
│   ├── adapters/         ← filesystem readers, scanners
│   ├── application/      ← trajectory_service.py (оркестратор)
│   ├── checks/*.yaml     ← DATA-контракты правил
│   └── tests/            ← self-test
├── check_project_trajectory.py  ← legacy entry point (→ governor)
├── verify_rust_architecture.sh  ← полный гейт (fmt+clippy+tests+invariants)
├── docs_drift_check.sh          ← проверка дрейфа доков
├── migrate_data_layout_m447.sh  ← миграция структуры data/
└── tests/test_governor.py       ← 83 unit-теста Governor
```

Единый CLI: PYTHONPATH=scripts python3 -m governor <subcommand>
