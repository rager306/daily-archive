# Continue — M437 закрыт; корпусный семантический граф построен

## Last action

M437-p43udw complete (skip-delivered + UAT + sha256:2260e195… validate +
gates + complete + checkpoint):
- `4dee321e`: **первый корпусный семантический граф** — 173 бумаги →
  `artifacts/gold_corpus/corpus_graph.sgsnap` (21868 узлов / 51518 рёбер),
  AllWavesReport с sha256-привязкой, replay_zero_writes по всем 20 волнам,
  chain непрерывен, D127 locked.
- ADR-065: каталог 100% versionless → честный `tei_declared_revision`
  binding (GROBID idno `arXiv:{id}v{N}` из parse-кэша); fail-closed
  сохранён; 27 бумаг без версии исключены DATA-манифестом.
- Canary fix (red-first): stage-id из run report, не hardcoded `extract`.
- `data/graph_tasks/gold_corpus.tasks.cypher`: 10 задач, suite PASSED
  (0 EMPTY / 0 ERROR).

## Current state

- Волна M420→M437 (18 milestones) закрыта. QUEUE пуст, STATE: no active.
- D127 locked. Governor: продуктовых флагов 0; resource_target_bloat
  [medium] вернулся (target/ 21GB, incremental 5GB) — очистка по явному «да».

## Next action (priority)

1. Кандидаты: миграция PipelineOrchestrator на движок kg-pipeline
   (ADR-064 §5, оценка рисков сначала); docs-drift-check за
   audit-командой; 27 no-version бумаг — когда появится evidence.
2. GitNexus reindex перед следующим крупным редактированием.
3. GSD-урок волны: validate требует гомогенные evidence-классы
   (Contract=runtime, Integration=artifact, UAT=runtime gsd_uat_exec
   на каждый browser-requiring слайс).

## Do not

Unlock D127. Pin a fake. Изобретать версии статей. `let _ =` на pub
write API. Хардкодить stage-id словари — читать из run report.
