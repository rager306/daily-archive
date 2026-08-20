# Continue — M425 implemented (pending GSD seal)

## Last action

M425-0fynay S01+S02 implemented (code green; GSD skip-delivered closeout next):

- **Два реальных бага найдены** переводом da-mcp query-тестов с Mock (нет
  Cypher) на реальный in-process SamyamaGraphStore:
  1. `search_papers` заявлял case-insensitive, но движковый CONTAINS
     регистрозависим, а пост-фильтр видел только найденное движком →
     lowercase-запрос возвращал 0. Фикс: retry с `toLower` + `coalesce`
     (только при нуле строк на точном проходе).
  2. В full-mode title-only пост-фильтр стирал abstract/topics-совпадения
     (Mock-артефакт «in case Mock returns unfiltered» внутри продукта).
     Фильтр теперь только в title-режиме.
- Перевёрнуты пришпиленные no-op тесты (CI `<=1`, full `<=5`); добавлены
  регрессии: get_paper, get_entities (MENTIONS), tool ≡ raw-Cypher parity.
- STUB-AUDIT: mock Cypher = test-double by design, покрытие — Samyama-тесты.
- da-mcp 40 passed; clippy clean; governor пины `mcp_search_ci_retry`,
  `mcp_real_engine_tests`.

## Next action (priority)

1. GSD skip-delivered closeout for M425 (UAT + sha256 validate + complete).
2. Следующий кандидат: оставшиеся query-инструменты (get_sections,
   get_summary, get_references) на Samyama + hybrid-поиск; либо новый
   процессный разрыв по AUDIT/STATE.

## Do not

Unlock D127. Pin a fake with a test. Полный проход cargo test через
tail/echo (маскирует exit-коды → gsd_exec_false_success_evidence).
`toLower(missing_prop)` в Samyama бросает type-error — использовать
`coalesce(p.prop, '')`. Пост-фильтр в full-mode не возвращать.
