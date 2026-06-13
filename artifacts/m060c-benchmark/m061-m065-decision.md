# M061-M065 graph library decision

Этот документ фиксирует практический выбор библиотек после M060c S01/S02. Он дополняет ADR-016 и applicability matrix; он не является разрешением на запись графа или production import.

## Safety defaults

- `graph_writes_authorized=false`
- `production_import_authorized=false`
- `fact_promotion_authorized=false`
- `external_network_enabled=false`
- `llm_calls_enabled=false`

Safety statements для сканеров:

- Graph writes are not authorized.
- Production import is not authorized.
- Fact promotion is not authorized.
- External network default is disabled.
- LLM calls default is disabled.

Любые локальные проверки должны использовать `127.0.0.1`.

## Краткое правило

- **NetworkX** остаётся основной библиотекой для представления графа, read-only control-plane операций, регрессионных тестов и понятной диагностики.
- **igraph** принят как supplementary backend для algorithm-heavy операций в M060b и M061, когда ускорение перекрывает стоимость конверсии.
- **rustworkx** принят как optional supplementary backend для BFS/path hot spots, если он доступен и есть parity/fallback через NetworkX.
- **graph-tool, PyG, DGL, NetworkX-Temporal** сейчас не используются в runtime.
- **GraphScope** откладывается до M063 как возможный кандидат только в рамках GraphDB-selection comparison.

## Выбор по milestone

| Milestone | Library choice | Почему | Ограничение |
|---|---|---|---|
| M060b (intermediate layer) | NetworkX primary; igraph supplementary; rustworkx optional for traversal/path hot spots | NetworkX даёт читаемую каноническую модель, а igraph/rustworkx дают benchmark-backed ускорение для тяжёлых read-only операций. | Любая конверсия должна иметь NetworkX parity/fallback. |
| M061 (2-hop BFS) | NetworkX correctness baseline; igraph for algorithm-heavy adjacent analysis; rustworkx for BFS/path hot spots when available | M061 наиболее чувствителен к алгоритмической задержке; S01 показал, что igraph и rustworkx быстрее на тяжёлых операциях. | Ускоритель не должен менять семантику BFS и не должен становиться write path. |
| M062 (fd hardening) | NetworkX primary | fd hardening требует прозрачности, стабильных diagnostics и read-only поведения больше, чем максимальной скорости. | igraph/rustworkx допустимы только для измеренного hot path. |
| M063 (GraphDB selection) | NetworkX as semantic control; igraph/rustworkx as in-process comparators; GraphScope only as deferred candidate | GraphDB выбор должен сравнивать persistence/query substrate отдельно от in-process algorithm libraries. | igraph/rustworkx не являются GraphDB; GraphScope не разрешён как production path. |
| M064+ (production) | NetworkX control plane; igraph/rustworkx only after explicit production gate | Production должен сначала доказать packaging, fallback, parity и observability. | Production import is not authorized by this document. |

## Deferred libraries

| Library | Current decision | Revisit trigger |
|---|---|---|
| graph-tool | Defer / do not adopt now | Только если M061+ докажет, что pip-installable igraph/rustworkx не держат latency target. |
| PyG | Out of scope | Только при явном GNN/ML requirement. |
| DGL | Out of scope | Только при явном GNN/heterograph requirement. |
| NetworkX-Temporal | Defer | Только если графовая модель станет time-sliced, а не просто typed evidence graph. |
| GraphScope | Defer except M063 evaluation | Только если M063 GraphDB-selection требует distributed analytics comparison. |

## Decision summary

M060b и M061 получают igraph как принятый supplementary backend для algorithm-heavy операций. NetworkX остаётся основной библиотекой для read-only ops, contract clarity и проверки корректности. rustworkx разрешён как optional supplementary backend для BFS/path hot spots when available. Все остальные библиотеки отложены до появления отдельного требования или milestone gate.
