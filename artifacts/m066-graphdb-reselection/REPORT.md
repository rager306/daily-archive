# M066: повторный выбор GraphDB

## 0. Резюме M066

M066 завершает повторную оценку GraphDB для daily-archive и выбирает **Neo4j** как целевую production GraphDB. В расширенном бенчмарке M066 S01 Neo4j получил **76/90** и занял первое место; прежний выбор **LadybugDB** из ADR-020 получил **62/90** и больше не является выбранной production GraphDB.

Итог M066 закреплён в **ADR-021**, который supersedes ADR-020. ADR-020 остаётся историческим решением M063/M065, но его binding-выбор LadybugDB заменён новым binding-решением: Neo4j.

## 1. Контекст

M063/M065 выбрали LadybugDB по 12-критериальному сравнению: LadybugDB тогда набрал **39/45** и выглядел лучшим вариантом из-за низкой стоимости миграции от NetworkX и сильной graph-vector позиции. После этого возникли дополнительные production-вопросы: конкурентные записи, транзакционность, UDF, GRAFBLAS-класс алгоритмов, multi-process safety и полнота документации по advanced-возможностям.

M066 был создан как повторная оценка с расширенными критериями. Главный риск, который изменил решение: в offline shared-counter harness LadybugDB показал только **101 успешную запись из 300**, то есть **33,67% success** и **199 lost writes**. Для будущей ingestion-нагрузки scientific KG это неприемлемый сигнал.

## 2. S01: 18-критериальный benchmark

S01 расширил исходные 12 критериев M063 до 18 критериев и сравнил пять кандидатов: FalkorDB, LadybugDB, Neo4j, HelixDB и Apache AGE. Новые advanced-критерии покрывали concurrent writes, GRAFBLAS graph algorithms, UDF support, ACID transactions, multi-process safety и advanced-feature documentation.

Ключевые результаты:

| Кандидат | M063 baseline | M066 score | M066 rank | Advanced |
|---|---:|---:|---:|---:|
| Neo4j | 34/45 | 76/90 | #1 | 29/30 |
| FalkorDB | 35/45 | 68/90 | #2 | 22/30 |
| Apache AGE | 28/45 | 64/90 | #3 | 25/30 |
| LadybugDB | 39/45 | 62/90 | #4 | 12/30 |
| HelixDB | 30/45 | 54/90 | #5 | 15/30 |

Concurrent write harness также изменил картину: Neo4j, FalkorDB и Apache AGE прошли 300/300 без lost writes; LadybugDB потерял 199 записей; HelixDB потерял 299 записей. Поэтому прежний лидер по M063 перестал быть приемлемым production-выбором.

## 3. S02: ADR-021

S02 выпустил **ADR-021: GraphDB Re-Selection for M066** в формате M034-style binding ADR. Документ закрепляет Neo4j как production GraphDB target, связывает выбор с evidence из S01 и явно supersedes ADR-020.

ADR-021 оставляет важную границу: M066 выбирает базу и migration target, но не включает production-import или запись в production graph. Практическая миграция должна идти отдельным milestone с Cypher rewrite, Neo4j transactions, schema mapping и проверкой safety defaults.

## 4. Top-3 M066

Top-3 после расширенного benchmark:

1. **Neo4j — 76/90.** Лучший общий баланс: зрелые транзакции, процедуры/UDF, многопроцессная безопасность, документация и graph algorithms.
2. **FalkorDB — 68/90.** Сильная graph-vector позиция и хорошие concurrent writes, но ниже advanced-production профиль и меньше binding-evidence для текущих требований.
3. **Apache AGE — 64/90.** Привлекателен за счёт PostgreSQL-соседства и ACID-профиля, но уступает по graph/vector ergonomics и migration fit; остаётся условной будущей альтернативой, если PostgreSQL-консолидация станет главным ограничением.

LadybugDB опустился на четвёртое место из-за concurrent write evidence и слабых advanced-критериев.

## 5. Почему Neo4j

Neo4j выигрывает не потому, что он самый простой в эксплуатации, а потому что лучше закрывает production-риск scientific KG.

Главные плюсы:

- **Concurrent writes: 5/5.** Harness показал 300 успешных записей из 300 и 0 lost writes.
- **GRAFBLAS-class algorithms: 4/5.** Экосистема graph algorithms сильнее, чем у большинства кандидатов, даже если не является прямой заменой каждого NetworkX/GRAFBLAS-паттерна.
- **UDF support: 5/5.** Процедуры и расширения дают ясный путь для domain-specific graph logic.
- **ACID transactions: 5/5.** Транзакционная модель подходит для ingestion, retries и auditability.
- **Multi-process safety: 5/5.** Production-клиенты и транзакции соответствуют будущему worker/queue режиму лучше, чем embedded/shared-state варианты.
- **Advanced documentation: 5/5.** Документация и operational knowledge уменьшают риск следующего milestone.

## 6. Tradeoffs

Выбор Neo4j имеет цену:

- **Operational complexity: 2/5.** Появляется отдельный сервис, deployment surface, backup/restore, monitoring, credentials и lifecycle management.
- **License posture: mixed.** Нужно держать edition/license constraints видимыми при production-планировании.
- **Migration cost from NetworkX: 3/5.** Требуется переписать часть graph logic в Cypher, транзакционные операции и Neo4j driver patterns.
- **Graph-vector ergonomics: 4/5.** Neo4j силён, но не всегда проще специализированных graph-vector систем.
- **Не мгновенная замена.** M066 закрывает decision risk, но не делает production migration.

Эти tradeoffs ниже риска lost writes и неполной транзакционной модели для будущей ingestion-нагрузки.

## 7. Migration plan, lessons и следующие milestones

План миграции:

1. Составить schema mapping: article, citation, table, figure, judge/evidence, queue/work-state и provenance nodes/relationships.
2. Переписать NetworkX-зависимые graph paths в Cypher queries и Neo4j driver transactions.
3. Ввести transaction wrapper для ingestion steps: idempotency keys, retry boundaries, rollback semantics и audit state.
4. Перенести queue/worker graph touches на Neo4j transactions только после отдельной verification slice.
5. Сохранить safety defaults: no production import, no graph writes и no external connectivity без явного override.
6. Проверить migration evidence на real fixture corpus до любого production включения.

Уроки M066:

- 12-критериальный выбор был недостаточен: advanced concurrency and transaction evidence должен входить в binding GraphDB decision.
- Простая миграция от NetworkX не компенсирует lost writes.
- ADR должен явно supersede прежнее binding-решение, а не оставлять два конкурирующих источника правды.

Следующие milestones:

- **M064 queue work** должен проектироваться вокруг Neo4j transactions как целевой production GraphDB path.
- **PostgreSQL conditional path** остаётся только как future option через Apache AGE, если consolidation станет важнее Neo4j-функций.
- **M062-fd-v2-verification** остаётся upstream quality gate для embedding/input evidence перед graph ingestion.
