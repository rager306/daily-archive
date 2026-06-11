# M057 Deferred Decisions

## Scope

Этот файл фиксирует решения, намеренно отложенные из M057 в M059. Они не являются скрытыми TODO и не разрешают production use. Production import is disabled.

## 1. Chart extraction через PlotExtract

**Decision:** отложить chart extraction до M059.

**Why:** M057 уже закрыл первый diagnostic content-graph pass через citation, table_similarity и figure_similarity. Chart extraction требует отдельного качества: axis detection, legend parsing, series extraction, figure-to-chart disambiguation и проверка ошибок на реальных PDF artefacts. Если смешать это с S04, итоговый ADR стал бы менее проверяемым.

**M059 entry condition:** использовать M057 combined graph как baseline, затем добавить chart-specific edges только после отдельного extraction report и тестов.

**Authorization:** chart extraction production use is disabled в M057.

## 2. Marker full re-extraction (5-PDF → 166-PDF)

**Decision:** отложить полную Marker re-extraction 166 PDF до M059. M057 S01-fix доказал, что env fix работает (1 PDF реально извлечён), но полная re-extraction слишком дорогая.

**Status of env fix:** DONE в M057 S01-fix. Downgrade `transformers` с 5.8.1 на 4.57.6 (`uv add 'transformers>=4.45.2,<5'`) восстанавливает `transformers.onnx` и `find_pruneable_heads_and_indices`. Smoke-тест 2605.28617v1 прошёл за 5:41, выдал 94715 chars markdown.

**Why full extraction is deferred:** стоимость 8-15 часов single-threaded (или 2-4 часа с 4-way parallelism) для 166 PDF. ROI пограничный — большая часть diagnostic evidence уже получена через OpenDataLoader + fd.

**M059 entry condition:** запустить Marker на 5-10 PDF (расширить sample), измерить реальное quality delta и cost-benefit. Только после этого решать, делать ли полный 166-PDF re-extraction.

**Authorization:** Marker full re-extraction production use is disabled в M057.

## 3. Safety defaults

Все пять safety defaults остаются false:

- `graph_writes_authorized`: false
- `production_import_authorized`: false
- `fact_promotion_authorized`: false
- `external_network_authorized`: false
- `llm_calls_authorized`: false

Локальный fd endpoint для M057: `http://127.0.0.1:8000`. Внешняя сеть не разрешена.
