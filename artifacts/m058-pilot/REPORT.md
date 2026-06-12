# M058 Pilot Evidence Report

## 0. Резюме M058

M058 проверил два независимых направления после M057: улучшение figure-caption слоя через `plotextractor` v2 и возможность масштабировать Marker как альтернативный PDF-парсер. Итог смешанный, но полезный: S01 успешен, S02 корректно остановил дальнейшее расширение, а S03 и S04 отменены по gate-решению S02.

S01 доказал, что TeX-источник можно использовать для более структурированного извлечения фигур: получено 104 figure/caption объекта по 5 PDF, все 5 TeX tarball доступны, общий объём TeX около 23 MB, labels доступны примерно для 99% объектов, image paths примерно для 60%. S02 доказал другое: Marker на первом page-limited прогоне даёт полезный сигнал качества, но этот сигнал не годится для решения о масштабировании полного корпуса. Поэтому M058 закрывается как пилот evidence-synthesis, а не как разрешение на production import.

External network is not authorized. Graph writes is disabled. Fact promotion is not authorized. LLM calls is disabled. Production import is disabled.

## 1. Контекст: M057 3-layer graph + S01 v2 figure layer

M057 уже собрал диагностический 3-layer graph: `citation`, `table_similarity`, `figure_similarity`. Его размер — 9403 edges: 4454 citation edges из M056, 4934 table-similarity edges из OpenDataLoader/fd и 15 figure-similarity edges v1 из regex caption extraction. ADR-011 принял этот content graph как supplement к ADR-010, но не как основание для production import.

M058 добавляет четвёртый слой: `figure_similarity_v2` из TeX-derived captions, labels и image paths. Этот слой тоже даёт 15 inter-doc edges, но отличается качеством источника: v1 был regex-слоем поверх уже извлечённых caption strings, а v2 связывает caption с TeX label и image path там, где они доступны. Поэтому v2 не заменяет весь M057 graph, а усиливает его как более трассируемый figure evidence layer.

## 2. S01 plotextractor v2

S01 обработал 5 PDF: `2605.18747`, `2601.05808`, `2602.10090`, `2507.19457`, `1804.02767`. Для каждого найден TeX source, статус `tex_status=ok`, суммарно извлечено 104 figures и 104 captions. `plotextractor` v2 сохранил safety defaults false и не выполнял внешние продвижения данных.

Ключевые метрики S01:

- sample size: 5 PDF;
- TeX availability: 5/5;
- total figures: 104;
- total captions: 104;
- TeX tarball size: около 23 MB суммарно;
- label availability: 0.990385;
- image path availability: 0.605769;
- figure v2 similarity edges: 15;
- mean similarity: 0.779106;
- threshold: 0.75.

Сравнение с M057 показывает важный tradeoff: M057 v1 остаётся шире по корпусу и имеет более высокий mean similarity для своих 15 edges, но v2 даёт то, чего у v1 не было — labels и image paths. Для graph-readiness это важнее, чем небольшая разница в score, потому что downstream audit может связать edge с исходным TeX evidence.

## 3. S02 Marker stage 1

S02 выполнил Marker stage 1 на 5 PDF в page-limited режиме (`page_range=0`, `pilot_max_pages=1`). Запуск использовал loopback bind host `127.0.0.1`; строка с альтернативным loopback alias намеренно не используется. Marker version: 1.10.2. Среднее время на PDF — 586.275 seconds по summary, средний markdown length — 3528, средний body word count — 517.

Качество оказалось promising, но недостаточным для масштабного решения:

- successful: 5;
- failed: 0;
- available OpenDataLoader comparisons: 2/5;
- Marker better than OpenDataLoader: 50%;
- avg quality delta: 157.5;
- fifth requested id `2305.14314` не был доступен локально;
- executable replacement: `1804.02767`, потому что он есть в S01 и имеет OpenDataLoader correctness data.

Gate-решение S02: не идти автоматически в S03. Причина не в провале Marker как инструмента, а в слабости evidence: одна страница на PDF не отвечает на вопрос full-document cost/readiness. Это именно тот случай, где iterative gate должен остановить дальнейший расход времени.

## 4. S03 и S04 cancelled per S02 gate

S03 должен был расширить Marker до cumulative 15 PDF, а S04 — до cumulative 45 PDF. Оба шага зависели от положительного gate после S02. Так как S02 вынес NO-GO, S03 и S04 отменены. Это не недовыполнение плана, а правильное применение guardrail: слабый page-limited signal не масштабируется автоматически.

Финальное состояние M058 для отчёта: 3/5 slices complete по смыслу пилота — S01 complete, S02 complete, S05 synthesis complete после этой работы; S03 и S04 cancelled per S02 gate. Если roadmap checkbox ранее выглядит закрытым для S03/S04, этот report трактует их как cancelled/skipped by gate, а не как выполненную Marker-экстракцию.

## 5. Combined graph

S05 собрал `artifacts/m058-pilot/combined-edges.json` и `artifacts/m058-pilot/per-layer-summary.json`. Итоговый graph содержит 9418 normalized edges в 4 слоях:

| Layer | Edges | Mean similarity | Source evidence |
|---|---:|---:|---|
| `citation` | 4454 | n/a | M056 BFS candidate citations |
| `table_similarity` | 4934 | 0.894583 | M057 fd table similarity |
| `figure_similarity_v1` | 15 | 0.819044 | M057 regex figure captions |
| `figure_similarity_v2` | 15 | 0.779106 | M058 TeX-derived figure captions |

Normalized schema: `source_paper_id`, `source_artifact_type`, `source_artifact_idx`, `target_paper_id`, `target_artifact_type`, `target_artifact_idx`, `similarity_score`, `evidence_layer`, `evidence_id`, плюс artifact ids и relation type. Citation edges не имеют similarity score, поэтому их `mean_similarity` равен null; это честнее, чем искусственно превращать citation relation в numeric similarity.

## 6. Graph-readiness gate v2 status

Статус: PARTIAL.

S01 evidence strong для figure layer v2: TeX-derived extraction добавляет labels/image paths и сохраняет 15 inter-doc similarity edges. Это улучшает auditability figure-caption слоя и заслуживает binding supplement в ADR-012.

S02 evidence insufficient для Marker scale: page-limited extraction не проверяет full-document cost, table/figure completeness, OCR variability, multi-page layout loss и tail latency. Поэтому Marker не должен быть масштабирован до 15/45/166 PDF на основании M058 S02.

Graph-readiness v2 всё ещё требует более сильного corpus-level evidence: 2-hop BFS cite-graph expansion, fd production hardening и явный выбор GraphDB/graph runtime по ADR-002 линии.

## 7. ADR-012 decision

ADR-012 принимает v2 figure-caption слой как binding supplement к ADR-011. Решение узкое: `figure_similarity_v2` добавляется к diagnostic graph manifest, потому что TeX labels и image paths улучшают provenance. Решение не расширяет разрешения безопасности и не авторизует импорт.

Marker scale stopped: M058 не принимает Marker как full-corpus parser. Marker остаётся candidate для будущего full-document пилота, когда будут готовы inputs, cost budget, comparison harness и stop/go thresholds.

## 8. M060 plan

M060 должен продолжить не Marker scale-up, а graph-readiness foundation:

1. 2-hop BFS cite-graph expansion от текущих anchor papers, чтобы проверить, растёт ли connected evidence лучше, чем в M056 1-hop.
2. fd production hardening: локальный service health, deterministic batch interface, retry/failure evidence и stable artifact contracts.
3. ADR-002 GraphDB selection: выбрать рабочий graph backend/runtime для диагностического graph evidence, не смешивая это с production import.
4. Сохранить safety posture: External network is not authorized unless a later milestone explicitly changes it; graph writes is disabled for M058-derived artifacts.

Chart extraction и Marker full-document scale должны быть вынесены из M060 core path, если они не нужны для cite-graph/fd/GraphDB decision. Их можно вернуть как отдельные pilots после появления full-document cost evidence.

## 9. Lessons

Первый урок: iterative gate worked as designed. S02 не пытался «доказать успех» после weak evidence, а остановил S03/S04 до expensive expansion.

Второй урок: page-limited evidence is not actionable for scale. Одна страница полезна для smoke-test и interface validation, но не для решения о 15/45/166 PDF.

Третий урок: v2 figure captions ценны не количеством edges, а provenance. 15 edges v2 не больше, чем 15 edges v1, но labels/image paths делают слой пригоднее для audit и будущего graph inspection.

Четвёртый урок: safety defaults должны быть частью каждого artifact, а не устной договорённостью. В S01, S02 и S05 они сохранены как machine-readable false значения.

## 10. Next milestones

- M060: 2-hop BFS + fd production hardening + ADR-002 GraphDB selection.
- M061: full-document parser evaluation only if M060 confirms graph-readiness path and inputs are local/ready.
- M062: chart extraction or multimodal figure/table enrichment as separate pilot, not bundled with parser scale decisions.

M058 closes as a successful evidence pilot with a deliberately deferred scale decision. The right next action is to preserve the combined graph manifest and ADR-012, then start M060 from the narrowed graph-readiness question.
