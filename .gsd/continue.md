# Continue — M428 implemented (pending GSD seal)

## Last action

M428-kyglr1 S01 implemented (code green; GSD skip-delivered closeout next):

- `graph_rank`: первый seeded структурный тест на реальном движке — hub
  (2 входящие CITES) ранжируется первым в PageRank; WCC даёт компоненты
  3+2. Алгоритм был корректен — непроверенный код стал проверенным.
- `get_method_lineage`: два `if let Ok` (тот же класс M426) больше не
  глотают ошибки движка — Err наружу; var-length `predecessor_of*1..N`
  доказан поддержкой движка (цепочка transformer→vit). В tools.rs ноль
  `if let Ok` на engine-вызовах.
- da-mcp 46 passed; clippy clean.

Волна: M420→M428 (8 milestones; 4 бага: CI-поиск, full-mode фильтр,
references swallowing+schema, lineage swallowing; 2 мок-фейка).

## Next action (priority)

1. GSD skip-delivered closeout for M428.
2. da-mcp полностью покрыт и без своллоинга. Кандидаты: процессный
   разрыв из STATE/AUDIT/QUEUE; либо волна свежего аудита другой
   плоскости (da-application stages на pin-паттерны).

## Do not

Unlock D127. Pin a fake. `if let Ok` на engine-вызовах.
toLower(missing_prop) → coalesce. Validate требует структурные
evidence-строки с gsd_uat_exec на каждый запланированный класс.
