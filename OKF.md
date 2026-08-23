# daily-archive — корневой индекс структуры

```
daily-archive/
├── data/              ← научные данные (статьи, словари, графы, gold)
├── governor/          ← мета-слой контроля (фикстуры Governor)
├── crates/            ← Rust workspace (гексагональный)
├── scripts/governor/  ← Python Governor CLI
├── doc/               ← ADR + аудиты
├── artifacts/         ← scratch текущей волны (чистится по завершении)
├── archive/           ← холодное хранилище (D237, append-only)
├── logs/              ← structured logs (ADR-052)
└── target/            ← cargo build output (gitignored)

Единый CLI: PYTHONPATH=scripts python3 -m governor --help
Полный гейт: governor architecture
Health check: governor doctor
```

Каждый подкаталог имеет свой OKF.md с детальным описанием.
