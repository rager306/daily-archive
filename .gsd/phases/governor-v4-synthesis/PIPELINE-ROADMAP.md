# Pipeline Roadmap: Event DAG + Semantic Tests (M371)

## 1. Дерево событий (DAG) — как всё сходится в snapshot

```
                    ┌─────────────────────────────────────────────────┐
                    │              CONTENT (sha256)                    │
                    │   PDF bytes OR HTML bytes OR metadata only      │
                    └─────────┬───────────────────────┬───────────────┘
                              │                       │
                    ┌─────────▼────────┐    ┌────────▼─────────┐
                    │  S1: PARSE       │    │  S2: FIGURES     │
                    │  GROBID / HTML   │    │  pdffigures2/DOM │
                    │  → ParsedArticle │    │  → FigureRef[]   │
                    └────────┬─────────┘    └────────┬─────────┘
                             │                       │
                    ┌────────▼────────┐    ┌────────▼─────────┐
                    │  S3: CROP       │    │                   │
                    │  PyMuPDF/HTTP   │    │                   │
                    │  → PNG files    │    │                   │
                    └────────┬─────────┘    └────────┬─────────┘
                             │                       │
                    ┌────────▼────────────────────────▼─────────────┐
                    │           S4: INGEST (graph write)             │
                    │  Paper + Section + Citation + Reference nodes  │
                    │  + hasPart + CITES + FROM_SOURCE edges         │
                    └────────┬──────────────────────┬────────────────┘
                             │                      │
             ┌───────────────▼──────────┐  ┌────────▼──────────────┐
             │  S5: EXTRACT             │  │  S6a: SUMMARY ABS     │
             │  RuleBasedExtractor      │  │  LLM (MiniMax-M2.7)   │
             │  → Entity + MENTIONS     │  │  → Summary + Quality  │
             └───────────────┬──────────┘  │    Signal + Glossary  │
                             │             └────────┬──────────────┘
                             │                      │
                             │             ┌────────▼──────────────┐
                             │             │  S6b: SUMMARY SECTIONS │
                             │             │  LLM (один промпт)     │
                             │             │  → N Summary(sec)     │
                             │             └────────┬──────────────┘
                             │                      │
                    ┌────────▼──────────────────────▼──────────────┐
                    │         S7: FIGURE VLM (MiniMax-M3)           │
                    │  PNG + caption + context → Figure description │
                    └────────┬─────────────────────────────────────┘
                             │
                    ┌────────▼─────────────────────────────────────┐
                    │  S8: SEMANTIC ENRICHMENT (from LLM JSON)      │
                    │  Method/Dataset/Metric/Term typed nodes       │
                    │  ← REUSE S6a JSON, no new LLM call            │
                    └────────┬─────────────────────────────────────┘
                             │
                    ┌────────▼─────────────────────────────────────┐
                    │  S9: ENTITY RESOLUTION (Healing)              │
                    │  canonical_vid, merge, silence duplicates     │
                    └────────┬─────────────────────────────────────┘
                             │
                    ┌────────▼─────────────────────────────────────┐
                    │  S10: CONTEXT / PAGEINDEX                     │
                    │  Section → hasSummary → Summary               │
                    │  reading order, retrieval_eligible            │
                    └────────┬─────────────────────────────────────┘
                             │
                    ┌────────▼─────────────────────────────────────┐
                    │  S11: ENRICH (OpenAlex + Crossref)            │
                    │  topics, venue, authors, DOI                  │
                    └────────┬─────────────────────────────────────┘
                             │
                    ┌────────▼─────────────────────────────────────┐
                    │  S12: CLUSTER (ConceptCluster)                │
                    │  WCC on typed subgraphs → communities         │
                    └────────┬─────────────────────────────────────┘
                             │
                    ┌────────▼─────────────────────────────────────┐
                    │  S13: GRAPH FINALIZE                          │
                    │  ◄═══ ALL EVENTS CONVERGE HERE ═══►          │
                    │  export_snapshot → hub.sgsnap                 │
                    └──────────────────────────────────────────────┘
```

**Точка схождения:** S13 (snapshot) — все 13 этапов должны завершиться до экспорта.
**Параллельность:** S1 и S2 независимы (могут работать параллельно).
**Блокировки:** S8 требует S6a (JSON reuse). S9 требует S5. S10 требует S4+S5+S6. S12 требует S9.

---

## 2. Семантические тесты (не формальные!)

### Принцип: тест проверяет СМЫСЛ, а не просто `result.is_ok()`

| Этап | Формальный тест (ПЛОХО) | Семантический тест (ХОРОШО) |
|------|------------------------|---------------------------|
| S1 | `assert_eq!(parsed.sections.len(), 85)` | `assert!(parsed.sections.iter().any(\|s\| s.title.contains("Introduction")))` |
| S5 | `assert!(entity_count > 0)` | `assert!(entities.iter().any(\|e\| e.surface.contains("GRPO") && e.entity_type == EntityType::Method))` |
| S6a | `assert!(summary.text.len() > 100)` | `assert!(summary.contains("GEPA") && summary.contains("6%"))` |
| S6b | `assert_eq!(section_summaries.len(), 22)` | `assert!(section_summaries.iter().any(\|s\| s.summary.contains("reinforcement learning")))` |
| S7 | `assert!(figure.description.len() > 0)` | `assert!(figure.description.contains("flowchart") \|\| figure.description.contains("architecture"))` |
| S8 | `assert!(method_count > 0)` | `assert!(methods.iter().any(\|m\| m.name == "GRPO" \|\| m.name == "MIPROv2"))` |
| S9 | `assert_eq!(duplicate_count, 0)` | `assert!(entities.iter().all(\|e\| !e.canonical_vid.is_empty()))` |
| S10 | `assert!(edge_count > 0)` | `assert!(sections.iter().all(\|s\| s.summary.is_some()))` |
| S12 | `assert!(clusters.len() > 0)` | `assert!(clusters.iter().any(\|c\| c.terms.contains("reinforcement learning")))` |

---

## 3. Roadmap — что получаем на каждом шаге

| Шаг |_slice_ | Что на входе | Что на выходе | Семантический тест (главный) | Сходится в |
|-----|--------|-------------|--------------|------------------------------|-----------|
| **S1** | S02 | PDF/HTML bytes | ParsedArticle (sections, body, citations) | Section "Introduction" exists with path_id="1" | → S4, S5, S6 |
| **S2** | S02 | PDF/HTML bytes | FigureRef[] (bbox/URL + caption) | Figure "Figure 3" exists with caption containing "GEPA" | → S3, S7 |
| **S3** | S02 | PDF+bbox / HTML+URL | catalog/figures/*.png | fig_003.png exists and >10KB | → S7 |
| **S4** | S03 | ParsedArticle | Graph: Paper, Section, Citation nodes | MATCH (p:Paper {arxiv_id:"2507.19457"}) returns 1 row | → S5, S6, S10 |
| **S5** | S03 | ParsedArticle body | Entity + MENTIONS edges | Entity "GRPO" exists as type Method | → S8, S9 |
| **S6a** | S03 | abstract_text | Summary(abs) + QualitySignal + Glossary | Summary contains "GEPA" AND "6%" AND "35x" | → S8, S10 |
| **S6b** | S03 | ALL sections + body | N Summary(sec) nodes | Section "Introduction" has summary mentioning paper topic | → S10 |
| **S7** | S03 | PNG + caption + context | Figure node + description | Figure "Figure 3" description contains "flowchart" | → S13 |
| **S8** | S04 | S6a JSON (methods, datasets, metrics, glossary) | Method/Dataset/Metric/Term nodes | Method "GRPO" exists with description | → S9, S12 |
| **S9** | S04 | All entities | canonical_vid set, duplicates merged | No entity has empty canonical_vid | → S12 |
| **S10** | S04 | Sections + Summaries + Entities | hasSummary edges, reading order | Every Section has ≥1 hasSummary edge | → S13 |
| **S11** | S05 | arxiv_id | Paper: topics, venue, authors | Paper.topics contains "cs.CL" | → S13 |
| **S12** | S04 | Entity graph (deduped) | ConceptCluster + MEMBER_OF_CLUSTER | ≥1 cluster with ≥3 members | → S13 |
| **S13** | S05 | All outputs | snapshot .sgsnap | **ALL** semantic tests pass on snapshot | **END** |

---

## 4. Взаимоувязка: что блокирует что

```
S1 Parse ──────► S4 Ingest ──────► S5 Extract ──────► S9 Dedup ──────► S12 Cluster ──┐
                     │                   │                                │           │
                     │                   │                                │           ▼
                     │                   ▼                                │      S13 Snapshot
                     │              S8 Enrich ◄─── reuse ──── S6a Summary ◄── S6a LLM  ▲
                     │                                                       │           │
                     ├──► S6b Sections ◄── LLM ──────────────────────────────┘           │
                     │                                                                   │
S2 Figures ──► S3 Crop ──► S7 Figure VLM ──────────────────────────────────────────────►─┘
                                                                                                │
S11 Enrich (metadata) ──────────────────────────────────────────────────────────────────►─┘
                                                                                                │
S10 Context/PageIndex ◄── S4 + S5 + S6 ────────────────────────────────────────────────►─┘
```

**Критический путь:** S1 → S4 → S5 → S9 → S12 → S13
**LLM путь:** S6a (parallel with S5) → S8 → S9
**Figure путь:** S2 → S3 → S7 (parallel with S1-S6)
**Все сходятся в S13.**

---

## 5. Cache invalidation дерево

```
content_sha256 changed?
├── YES → invalidate ALL stages (new sha256)
└── NO → check each stage version:
    ├── grobid_v2 changed? → invalidate S1 (re-parse)
    ├── extract_v2 changed? → invalidate S5 (re-extract)
    ├── summary_abs_v4 changed? → invalidate S6a (re-LLM)
    ├── summary_sec_v1 changed? → invalidate S6b (re-LLM)
    ├── figvlm_v1 changed? → invalidate S7 (re-VLM)
    ├── enrich_v1 changed? → invalidate S11 (re-enrich)
    └── nothing changed → ALL stages cached, <30s
```

**Правило:** изменение версии одного этапа инвалидирует только downstream зависимости, не весь pipeline.

---

## 6. Семантический тест-сьют (полный)

```rust
// tests/semantic_pipeline_test.rs

#[tokio::test]
async fn test_full_pipeline_2507_19457_semantic() {
    // Run pipeline on hub paper
    let snapshot = run_pipeline("2507.19457").await;

    // S1: Structure
    assert_section_exists(&snapshot, "Introduction");
    assert_section_path_id_starts_with(&snapshot, "1");

    // S5: Entities
    assert_entity_exists(&snapshot, "GRPO", "Method");
    assert_entity_exists(&snapshot, "AIME-2025", "Dataset");

    // S6a: Abstract Summary
    let abs = get_summary(&snapshot, "abstract");
    assert!(abs.contains("GEPA"), "Summary must mention GEPA");
    assert!(abs.contains("6%"), "Summary must contain 6% improvement");
    assert!(abs.contains("35x"), "Summary must mention 35x fewer rollouts");

    // S6b: Section Summaries
    let sec_summaries = get_summaries(&snapshot, "section");
    assert!(sec_summaries.len() >= 5, "Must have ≥5 section summaries");

    // S7: Figure Understanding
    let figures = get_figures(&snapshot);
    assert!(!figures.is_empty(), "Must have figures");
    assert!(figures.iter().any(|f| f.description.contains("flowchart")),
        "A figure must be described as flowchart");

    // S8: Typed Entities
    assert_method_exists(&snapshot, "GRPO");
    assert_method_exists(&snapshot, "MIPROv2");

    // S9: Entity Resolution
    let entities = get_all_entities(&snapshot);
    assert!(entities.iter().all(|e| !e.canonical_vid.is_empty()),
        "All entities must have canonical_vid after dedup");

    // S10: Context/PageIndex
    let sections = get_sections(&snapshot);
    assert!(sections.iter().all(|s| s.has_summary),
        "Every section must have hasSummary edge");

    // S12: ConceptCluster
    let clusters = get_clusters(&snapshot);
    assert!(!clusters.is_empty(), "Must have ≥1 concept cluster");

    // Glossary
    let glossary = get_glossary(&snapshot);
    assert!(glossary.iter().any(|t| t.term == "GRPO"),
        "Glossary must define GRPO");
}
```

**Каждый assert проверяет СМЫСЛ, а не формальный факт.** Тест падает если:
- Summary не содержит конкретных чисел из статьи
- Method "GRPO" не найден (хотя статья именно про это)
- Figure не описана как flowchart (хотя Figure 3 — flowchart)
- Entity без canonical_vid (дедупликация не отработала)
- Glossary не определяет GRPO (LLM не понял статью)
