# Continue — M432 complete; CI matches the swept surface

## Last action

M432-uoupho complete (skip-delivered + UAT + sha256:06dc73e5… validate +
gates + complete):
- `d3137585`: CI-разрыв закрыт — мёртвый `-p da-graph` убран (clippy падал
  бы unmatched package ID), **da-mcp добавлен в CI** (46 real-engine
  тестов M425-M428 впервые в CI), интеграционные наборы da-cli (268, вкл.
  кросс-процессный ADR-061 контракт) и da-adapters (234) добавлены;
  verify-script синхронизирован.
- Локальное доказательство команд вскрыло и закрыло 2 реальных дефекта:
  1) **schema drift**: 4 поля пишутся с M418/M419, но не декларированы
     (Paper pdf_type / pdf_pages_needing_ocr / pdf_has_encoding_issues;
     Entity embedding_sha256) → декларированы, audit-fields = OK;
  2) CWD-зависимый figure-тест → DA_FIGURE_QUOTE_POLICY пинится абсолютно
     (CARGO_MANIFEST_DIR).
- Workspace swallow-sweep **полностью закрыт** (kg-* финальный скан чист,
  STUB-AUDIT запись).

Волна: M420→M432 (13 milestones).

## Next action (priority)

1. audit-fields сканер построчный — пропустил 2 из 4 дрейф-полей
   (multi-line вызовы). Кандидат: docs-drift-check / scanner fix.
2. Либо процессный разрыв из STATE/QUEUE.
3. Disk: target/ 22.7GB (incremental отстроился) — очистка по явному «да».

## Do not

Unlock D127. Pin a fake. `let _ =` на pub write API. Тесты с
workspace-относительными DATA-путями без CARGO_MANIFEST_DIR-пина.
Ручной перенос 64-символьных SHA.
