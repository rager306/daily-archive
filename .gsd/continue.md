# Continue — M436 закрыт; kg-pipeline движок честен

## Last action

M436-q9cuc1 complete (skip-delivered + UAT + sha256:0776105b… validate +
gates + complete + checkpoint):
- `d5e93613`: фиктивный контракт ExecutionEngine заменён реальным:
  per-step retry из policy registry + injectable `Sleeper` (ADR-051
  contour 1, NoopSleeper в тестах), FailurePolicy Abort/Continue/
  RetryThenAbort с транзитивным скипом зависимых, `attempts_per_step`/
  `skipped_steps`, наблюдаемый FSM (`StepEvent` + `StatusChanged`,
  все 6 статусов достижимы), `EngineControl` pause/stop (stop выигрывает).
- ADR-064 принят: движок владеет семантикой исполнения, Orchestrator
  остаётся продуктовой композицией. Обратная совместимость сохранена
  (execute() = однопопыточный abort), потребитель зелёный.
- Governor пин `pipeline_engine_honest` (required: max_attempts,
  StatusChanged, EngineControl) — отрицательная фикстура доказана
  (DRIFT_RISK high при удалении маркера). ON_TRACK flags=0.

## Current state

- Волна M420→M436 (17 milestones) закрыта. QUEUE пуст, STATE: no active.
- D127 locked. Governor ON_TRACK flags=0 violations=0.
- Полный guardrail PASS (158.7s). GitNexus: low risk, 0 affected processes.

## Next action (priority)

1. Кандидаты следующей волны:
   a) corpus-wide semantic graph rebuild (MEM679, gold-корпус,
      materialize_extracted путь);
   b) миграция PipelineOrchestrator на движок kg-pipeline (ADR-064 §5,
      follow-up — большой рефактор, оценить риски сначала);
   c) docs-drift-check за audit-командой.
2. GitNexus reindex перед следующим крупным редактированием — новые
   символы (StepEvent/EngineControl/Sleeper) не разрешаются.

## Do not

Unlock D127. Pin a fake. `let _ =` на pub write API. `FailureAction`
остаётся экспортируемым-но-непотребляемым (документировано ADR-064) —
не удалять без ревизии. Governor forbidden-паттерны — точные строки
кода, не английские фразы (M435/M436 урок; использовал required-маркеры).
