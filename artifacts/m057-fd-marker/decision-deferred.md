# M057 Deferred Decisions

## Scope

Этот файл фиксирует решения, намеренно отложенные из M057 в M059. Они не являются скрытыми TODO и не разрешают production use. Production import is not authorized.

## 1. Chart extraction через PlotExtract

**Decision:** отложить chart extraction до M059.

**Why:** M057 уже закрыл первый diagnostic content-graph pass через citation, table_similarity и figure_similarity. Chart extraction требует отдельного качества: axis detection, legend parsing, series extraction, figure-to-chart disambiguation и проверка ошибок на реальных PDF artefacts. Если смешать это с S04, итоговый ADR стал бы менее проверяемым.

**M059 entry condition:** использовать M057 combined graph как baseline, затем добавить chart-specific edges только после отдельного extraction report и тестов.

**Authorization:** chart extraction production use is not authorized в M057.

## 2. Marker re-extraction

**Decision:** отложить Marker re-extraction до M059.

**Why:** текущий environment не готов из-за проблемы `transformers.onnx`. M057 поэтому опирается на OpenDataLoader tables/figures и fd embeddings. Это достаточно для content graph v1, но недостаточно для окончательного сравнения Marker против OpenDataLoader.

**M059 entry condition:** исправить `transformers.onnx` environment issue, повторить Marker extraction, сравнить Marker tables/figures с OpenDataLoader outputs и только после этого решать, какие Marker artefacts можно включать в graph-readiness gate v2.

**Authorization:** Marker re-extraction production use is not authorized в M057.

## 3. Safety defaults

Все пять safety defaults остаются false:

- `graph_writes_authorized`: false
- `production_import_authorized`: false
- `fact_promotion_authorized`: false
- `external_network_authorized`: false
- `llm_calls_authorized`: false

Локальный fd endpoint для M057: `http://127.0.0.1:8000`. Внешняя сеть не разрешена.
