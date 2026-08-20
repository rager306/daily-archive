# Continue — M427 implemented (pending GSD seal)

## Last action

M427-r7k6yi S01 implemented (code green; GSD skip-delivered closeout next):

- Hybrid-режим `search_papers` доказан end-to-end на реальном движке:
  seeded 3 статьи, hybrid-запрос находит attention-статью с числовым
  score; fallback без embedder'а уходит в full-mode и находит
  diffusion-статью; пустой стор — 0 строк без ошибки. Путь оказался
  корректен — непроверенный прод-код стал проверенным.
- da-mcp 44 passed; clippy clean.

Волна: M420→M427 (7 milestones, 3 продуктовых бага + 2 фейка-мока).

## Next action (priority)

1. GSD skip-delivered closeout for M427.
2. da-mcp полностью покрыт. Кандидаты: graph_rank / get_graph_stats на\n   реальном движке (последние непроверенные инструменты), либо\n   процессный разрыв из STATE/AUDIT/QUEUE.

## Do not

Unlock D127. Pin a fake. `if let Ok` на engine-вызовах.
toLower(missing_prop) → coalesce.
