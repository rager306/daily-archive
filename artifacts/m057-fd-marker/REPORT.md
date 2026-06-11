# M057 REPORT: fd content-graph v1

## 0. Резюме: итог M057

M057 закрывает первый content-graph pass поверх результата M056. Итоговый диагностический граф содержит **9403 edges** в трёх evidence layers: **4454 citation**, **4934 table_similarity** и **15 figure_similarity**. Это не production import и не запись в LadybugDB: production import is disabled, graph writes are not authorized, fact promotion is disabled.

Практический вывод: content graph v1 принят как дополнительный evidence layer к citation-only графу M056. Табличные связи OpenDataLoader стали главным содержательным сигналом, потому что они дали плотный слой междокументных и внутридокументных похожестей. Figure layer полезен как слабый, но независимый сигнал: он дал только 15 междокументных edges, что фиксирует низкое overlap по captions/figures на текущем корпусе.

Все пять safety defaults остаются false:

- `graph_writes_authorized`: false
- `production_import_authorized`: false
- `fact_promotion_authorized`: false
- `external_network_authorized`: false
- `llm_calls_authorized`: false

Локальная fd-проверка использует `http://127.0.0.1:8000`; именованный loopback alias намеренно не используется.

## 1. Контекст и связь с ADR-010

ADR-010 зафиксировал результат M056: 1-hop BFS от anchor `2605.18747` дал достаточную parser-scale проверку на 149 unique PDFs, но не дал достаточную graph-readiness уверенность. Citation-only граф M056 содержал 4454 candidate citation edges, однако внутреннее target-set overlap было слабым: граф показывал, что парсер может извлекать ссылки, но не доказывал, что corpus уже готов для содержательного knowledge graph.

M057 отвечает именно на этот пробел. Вместо повторного расширения BFS он проверяет, можно ли получить дополнительные связи из содержимого документов: таблиц и фигур. Поэтому M057 не заменяет M056, а дополняет его: citation layer остаётся структурным сигналом, table_similarity добавляет content-level signal, figure_similarity добавляет независимый visual/caption-level signal.

## 2. S01 fd validation

S01 проверил локальный fd embedding service на адресе `http://127.0.0.1:8000` и подтвердил готовность embedding path для диагностического content graph. Результат: **7/7 тестов pass**. `latency_p95_ms = 253.397`, что в отчёте округляется до **253 ms**. Cache speedup составил **82x**, что подтверждает практическую пригодность повторных embedding calls для локального анализа без внешней сети.

Safety posture S01: external network is disabled, LLM calls are not authorized, graph writes are not authorized, fact promotion is disabled, production import is disabled. Все эти флаги представлены в `artifacts/m057-fd-marker/fd-validation.json` как false.

## 3. S02 table similarity

S02 построил table text corpus из OpenDataLoader artefacts и embedded **1468 таблиц**. После cosine threshold `0.85` получено **4934 table_similarity edges**, из них **2591 inter-doc** и 2343 intra-doc. Средняя похожесть слоя: **0.894583**.

Этот слой является главным содержательным результатом M057. В отличие от citation-only графа, таблицы дают прямой content-level сигнал: похожие экспериментальные сводки, benchmark tables, architecture comparisons и metric tables могут связывать статьи даже тогда, когда citation layer слабый или не замыкается внутри target set. Поэтому ADR-011 принимает OpenDataLoader tables как primary content evidence для graph-readiness gate v1.

## 4. S03 figure similarity

S03 построил figure-caption corpus и embedded **937 фигур**. После threshold `0.80` получено **15 figure_similarity edges**, все **15 inter-doc**. Средняя похожесть слоя: **0.819044**.

Низкое число edges — не провал, а важный диагностический результат. Figure captions в текущем корпусе дают мало overlap, но слой остаётся независимым evidence source. Он не должен доминировать в graph-readiness gate v1, зато подтверждает, что pipeline может нормализовать visual/caption evidence в тот же graph edge contract.

## 5. S04 chart extraction

Chart extraction через PlotExtract отложен до M059. Причина: M057 уже доказал 3-layer content graph без chart-specific extraction, а chart extraction требует отдельного качества: axis parsing, legend parsing, series extraction, image/table alignment и проверку ошибок на реальных figures.

Решение: не смешивать chart extraction с закрытием M057. M059 должен взять chart extraction как отдельную проверку и совместить её с 2-hop BFS или другим расширением корпуса. Production chart extraction is disabled в рамках M057.

## 6. Combined graph

S04 нормализовал три входных источника в единый edge schema:

`{source_paper_id, source_artifact_type, source_artifact_idx, target_paper_id, target_artifact_type, target_artifact_idx, similarity_score, evidence_layer, evidence_id}`

Итоговые файлы:

- `artifacts/m057-fd-marker/combined-edges.json`
- `artifacts/m057-fd-marker/per-layer-summary.json`

Итоговая статистика:

| Layer | Edges | Mean similarity | Distinct source papers | Distinct target papers |
|---|---:|---:|---:|---:|
| citation | 4454 | 1.085990 | 162 | 2658 |
| table_similarity | 4934 | 0.894583 | 77 | 81 |
| figure_similarity | 15 | 0.819044 | 10 | 9 |
| **total** | **9403** | — | — | — |

Для citation layer `similarity_score` равен `citation_count`, поэтому mean больше 1.0. Для table и figure layers это cosine similarity.

## 7. Graph-readiness gate v1

Graph-readiness gate v1 считается пройденным для диагностического режима, потому что выполнены три условия:

1. Есть минимум три независимых evidence layers: citation, table_similarity, figure_similarity.
2. Общий граф содержит 9403 normalized edges, включая содержательный table layer.
3. Все пять safety defaults остаются false; graph writes, production import и fact promotion не разрешены.

Это gate не означает разрешение на production import. Он означает, что проект может перейти от citation-only негативного вывода ADR-010 к content-assisted graph evaluation в следующем milestone.

## 8. ADR-011 decision

ADR-011 принимает content graph v1 через fd как supplementary evidence к M056 citation graph. Решение: объединять три слоя evidence в diagnostic graph manifest, считать OpenDataLoader table similarity primary content signal, а figure similarity — low-volume supplementary signal.

Binding consequence: дальнейшие graph-readiness работы должны учитывать content layers, а не оценивать corpus только по citation overlap. При этом все safety defaults остаются false, и production import is disabled.

## 9. Marker недоступен в env

Marker re-extraction не закрыт в M057 из-за проблемы окружения `transformers.onnx`. Это зафиксировано как deferred decision, а не как скрытый пропуск. M057 опирается на OpenDataLoader tables/figures и fd embeddings; Marker будет возвращён в M059 после исправления env.

Решение defer: Marker re-extraction is disabled for production use in M057. M059 должен сначала исправить `transformers.onnx` env path, затем повторить extraction и сравнить Marker output с OpenDataLoader output.

## 10. Lessons + next milestones

Lessons:

- Citation-only graph оказался недостаточным для graph-readiness, но полезным как structural baseline.
- Table similarity — самый сильный content evidence layer в текущем корпусе.
- Figure similarity даёт низкий overlap, однако сохраняет ценность как независимый diagnostic signal.
- fd service достаточно быстрый для локального диагностического прохода: 7/7 validation tests pass, p95 около 253 ms, cache speedup около 82x.
- Safety posture должен оставаться явным в каждом artefact, иначе следующие агенты могут перепутать diagnostic graph с production graph.

Next milestones:

1. M059: chart extraction via PlotExtract, Marker env fix, повторная Marker extraction и 2-hop BFS/content graph evaluation.
2. M058/M059 graph expansion: проверить, даёт ли 2-hop BFS больше внутренних связей, чем 1-hop baseline ADR-010.
3. Следующий graph-readiness gate: добавить quality checks для edge evidence, duplicate control, paper-id canonicalization и failure diagnostics.

Итог M057: content graph v1 принят как diagnostic evidence. Production import is disabled.
