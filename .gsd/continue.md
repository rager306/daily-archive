# Continue — M438 закрыт; архитектурные хвосты M436-M437 закрыты

## Last action

M438-ar8owh complete (skip-delivered + UAT + sha256:db6e2993… validate +
gates + complete + checkpoint):
- ADR-066 / D280: PipelineOrchestrator остаётся product-side. Миграция
  на движок kg-pipeline закрыта как deferred-by-design: пересечение
  ~30 строк из 2655; durable cross-process retry (кэш-persisted
  RetryableFailure + RetrySuppressed) vs bounded in-process retry
  (Sleeper) — комплементарные семантики. ACAP admission, cache identity,
  GraphMaterialize rerun, StageRunRecord не входят в универсальный крейт.
- docs_drift_check.sh теперь покрывает 5 команд (+audit-fields,
  +schema-check); GRAPH-SCHEMA.md получил секцию Schema drift gate.
  Negative fixture доказан; advisory-only сохранён (FAST-ONLY).
- Коммиты: `32dc02da` (волна), `dd7a7561` (gitnexus stats), `ae1907b7`
  (trajectory). GitNexus reindex выполнен: 30339 узлов / 56801 рёбер.

## Current state

- Волна M420→M438 (19 milestones) закрыта. QUEUE пуст, STATE: no active.
- D127 locked. Governor: продуктовых флагов 0; resource_target_bloat
  [medium] (target/ 21GB, incremental 5GB) — очистка по явному «да».

## Next action (priority)

1. Открытых архитектурных хвостов больше нет. Кандидаты:
   a) 27 no-version бумаг — блокировано до появления evidence upstream;
   b) перевод docs_drift_check.sh в blocking — требует hermetic binary
      path (сейчас противоречит FAST-ONLY);
   c) FailureAction: потребитель или удаление (ADR-064 Consequences);
   d) новая разведка по логам/продукту (не фабриковать no-op).
2. Disk cleanup по явному «да» пользователя.

## Do not

Unlock D127. Изобретать версии статей. Хардкодить stage-id словари.
Мигрировать Orchestrator без нового consumer (ADR-066). Governor
forbidden/required паттерны — точные строки кода/маркеры.
