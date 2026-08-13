# ADR-060: Scientific Figure Understanding Pipeline

## Status
Proposed (2026-08-12)

## Context

Scientific figure understanding — извлечение и семантическая интерпретация фигур, графиков, диаграмм и схем из научных статей (PDF) — требует **модульного pipeline**, а не single-shot VLM вызова. Исследование (M387 S07 post-close research, MEM767) показывает:

### Что работает (SOTA 2024–2026)
1. **Joint encoding** (OmniScience): figure + caption + citing sentence вместе → точность 0.77→0.96
2. **Two-stage** (ChartLlama/DePlot): classify type → per-type extraction. Chart→table→reasoning превосходит прямой VQA
3. **Grounded crop** (GROBID bbox + PyMuPDF): лучше, чем whole-page vision
4. **Docling**: `DocumentFigureClassifier`, `do_picture_description`, chart-to-table (`PictureItem.meta.tabular_chart`, 2026)

### Что не работает
1. **Single-shot VLM**: одна модель пытается одновременно классифицировать И описать И извлечь данные
2. **Schema-mismatch**: одна JSON schema (components/relationships) применяется ко всем типам — неверна для charts
3. **Isolated image без caption**: даёт слабый результат
4. **Compound figures**: много-панельные фигуры анализируются как один blob

### Текущее состояние daily-archive

**Уже есть и хорошо:**
- `FigureSemanticContext` (da-domain/src/figure_context.rs): несёт caption, containing_section (с текстом), reference_excerpts (citing sentences), article_summary, bbox, page_index — **это уже OmniScience-паттерн**
- `FigureContextStage` (application): готовит контекст из GROBID структурного TEI
- `FigureAnalysisUseCase` (application): VLM вызов с grounded prompt
- `VisualAnalysisType` enum: Flowchart, Plot, Diagram, Table, Screenshot, Photograph, Other
- GROBID structural profile: figure coordinates + captions (teiCoordinates)
- OpenAI-compatible multimodal adapter с JSON extraction + canonical output bytes
- Qwen3-VL binding (figure-qa-primary → novita/qwen3-vl-235b-a22b-instruct)

**Не хватает (конкретные пробелы):**
1. **Single-shot**: классификация и extraction совмещены в одном вызове
2. **Нет per-type schema**: диаграммная schema (components/relationships) применяется к charts, где нужны axes/data-points
3. **Нет chart-to-table**: для Plot/Chart типов нужен structured numeric extraction
4. **Нет compound decomposition**: много-панельные фигуры не разбиваются
5. **Нет Docling sidecar**: DocumentFigureClassifier + picture_description как второй источник кандидатов
6. **Multimodal adapter**: 9router нестабильно форвардит `enable_thinking` для reasoning моделей

## Decision

### Архитектура: two-stage figure understanding FSM

```
FigureSemanticContext (existing)
  │
  ▼
┌─────────────────────────────────┐
│ Stage 1: Figure Classification  │  ← cheap VLM call, thinking OFF, ≤200 tokens
│ Input: crop + caption + citing   │
│ Output: FigureTypeClassification │
│   { type: chart|diagram|photo|.. │
│     subpanels: [bbox, bbox, ..], │
│     confidence: f32 }             │
└──────────┬──────────────────────┘
           │
     ┌─────┴─────┬──────────┬──────────┐
     ▼           ▼          ▼          ▼
  Chart       Diagram     Photo     Compound
     │           │          │          │
     ▼           ▼          ▼          ▼
┌─────────┐ ┌─────────┐ ┌────────┐ ┌──────────────┐
│Stage 2a │ │Stage 2b │ │Stage 2c│ │Stage 2d      │
│Chart-   │ │Component│ │Caption │ │Decompose →   │
│to-Table│ │Analysis │ │+ Tags  │ │recurse 2a-2c │
│(axes,   │ │(comps,  │ │        │ │per subpanel  │
│ legend, │ │ rels,   │ │        │ │              │
│ data)   │ │ findings│ │        │ │              │
└────┬────┘ └────┬────┘ └───┬────┘ └──────┬───────┘
     │           │          │             │
     ▼           ▼          ▼             ▼
  Per-type FigureAnalysis candidate (non-authoritative, D127=false)
```

### Per-type JSON schemas (DATA-driven, not hardcoded)

```yaml
# data/ontology/figure_schemas.yaml
chart_extraction:
  type: object
  properties:
    chart_type: {enum: [bar, line, scatter, pie, box, heatmap, other]}
    x_axis: {label: string, unit: string}
    y_axis: {label: string, unit: string}
    legend: [{label: string, color: string}]
    key_data_points: [{label: string, x: string, y: string}]
    trend: string

diagram_analysis:
  type: object
  properties:
    components: [{name, role, visual_evidence}]
    relationships: [{source_component, relation, target_component}]
    flow_direction: {enum: [left_to_right, top_to_bottom, circular, other]}

photo_caption:
  type: object
  properties:
    description: string
    visible_objects: [string]
    scale_bar: {present: bool, value: string}

compound_decomposition:
  type: object
  properties:
    subpanels: [{label: string, bbox: [x,y,w,h], type: string}]
    layout: {enum: [grid_horizontal, grid_vertical, freeform]}
```

### Docling как optional sidecar (candidate-only)

Docling добавляется как **optional adapter** (feature-gated, как `pdf-classifier`):
- `DocumentFigureClassifier` → второй opinion на figure type (не замена GROBID)
- `do_picture_description` → candidate description (candidate-only, не graph truth)
- `PictureItem.meta.tabular_chart` → chart-to-table extraction (2026 capability)
- GROBID остаётся primary grounder (picture F1 0.63 у Docling — слабее)

### Модель routing (DATA-driven, models.yaml)

```yaml
# Stage 1: classification — cheap, fast, thinking OFF
- binding_id: figure-classify-primary
  model_id: ninerouter-qwen3-vl-235b-openai
  description: "Stage 1 figure classification (thinking OFF, ≤200 tokens)"

# Stage 2a: chart extraction — structured, thinking OFF
- binding_id: figure-chart-extract-primary
  model_id: ninerouter-qwen3-vl-235b-openai
  description: "Stage 2a chart-to-table extraction (thinking OFF)"

# Stage 2b: diagram analysis — existing binding
- binding_id: figure-qa-primary
  model_id: ninerouter-qwen3-vl-235b-openai
  description: "Stage 2b diagram component analysis"
```

### enable_thinking через chat_template_kwargs

Qwen3-VL требует `enable_thinking` через `extra_body.chat_template_kwargs`, не top-level.
Multimodal adapter должен корректно форвардить этот параметр через 9router.

## Consequences

- **Positive**: per-type extraction даёт структурированные данные для graph ingestion (chart data → Metric nodes, diagram components → Method nodes)
- **Positive**: two-stage снижает hallucination (классификация дешёвая, extraction targeted)
- **Positive**: compound decomposition решает много-панельные фигуры
- **Negative**: больше VLM вызовов (2+ per figure вместо 1) — bounded budget в policy
- **Negative**: Docling sidecar добавляет Python зависимость (D237 conflict — нужен Rust port или sidecar API как GROBID)

## D127 / authority locks

- Stage outputs — **candidate-only**, никогда graph truth
- `import_eligible=false`, `authoritative=false`
- Chart data points не mint canonical Metric nodes без отдельного review
- Docling candidates — второй opinion, не замена GROBID grounding

## Implementation priority

1. Stage 1 classifier (per-type routing)
2. Chart-to-table schema + extraction (highest value — structured numeric data)
3. Compound figure decomposition
4. Docling sidecar (optional, Python→API bridge as GROBID)
5. CharXiv-style evaluation suite (data/figure_tasks/)
