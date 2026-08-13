# M388 S05 — hot-path seam audit (figure / prompt / ingest)

Date: 2026-08-13  
Locks: D127=false, optimizer=false, D237 Rust-only.

## Seams closed earlier this milestone

| Seam | Status | Evidence |
|------|--------|----------|
| Summary-shaped claim extraction → false figure coverage=0 | **Closed S01** | `extract_claims_from_output` in `da-application`; commit `4dfb875a` |
| GEPA keyword hardcoding in scorer | **Closed S01** | no `prose_lower.contains("gepa")` in product body |
| Stage1 classify unused / no DATA labels | **Closed S02** | `figure_type_labels.yaml` + `FigureClassifyUseCase` `d7edb1e3` |
| Per-type Stage2 schemas missing | **Closed S03 (registry)** | `figure_schemas.yaml` + `FigureSchemaRegistry` `110e0348` |
| PDF triage opaque (None only) | **Closed S04** | `PdfTriageStatus` + policy YAML `39b22f3e` |

## Open seams (this slice targets #1)

| # | Seam | Location | Risk | Fix in S05 |
|---|------|----------|------|------------|
| 1 | **Hardcoded full JSON output schema in analyze** | `crates/da-application/src/figure_analysis.rs:32-90` `output_schema()` | Stage2 DATA registry unused on live path; chart/table schemas never sent to VLM | Wire `FigureClassifyUseCase` + `FigureSchemaService`; request uses selected `json_schema` |
| 2 | Stage1 classify not called from analyze | classify module isolated | Wrong schema for plots/tables if always diagram shape | Call classify on context before complete_structured |
| 3 | Stage2 registry not on analyze path | `FigureSchemaService` only unit-tested | Dead product seam | Select by Stage1 type |
| 4 | Domain `FigureAnalysis` still diagram-shaped (requires components) | `da-domain` `FigureAnalysis::validate` | Chart drafts lack components → validation fail if mapped naively | Map type-specific drafts → synthetic component(s) when needed |
| 5 | Stage fingerprint still uses static schema sha | `figure_analysis_stage.rs` contract | Cache key may not reflect type-specific schema yet | Keep default diagram schema for stage contract fingerprint; per-call sha in provenance from selection |

## Not in S05 (deferred)

| Item | Why later |
|------|-----------|
| Multi-draft domain types per visual type | Broader ontology change; synthetic component preserves single candidate type |
| VLM re-run eval with typed schemas | Needs paid/9router budget; offline unit tests first |
| S06 STATE vs ROADMAP governor projection | GSD hygiene, not product figure path |
| Summary quality S07 | Separate quality wave |

## Dead / stub scan (figure path)

- No `todo!` / `unimplemented!` in `figure_*.rs` application modules (scan 2026-08-13).
- No paper-specific keyword maps remaining in claim extractor production body (S01 guard test).

## Functional fix (S05 T02)

1. `analyze`: Stage1 classify → Stage2 schema select → multimodal request with DATA schema.
2. Flexible draft map into `FigureAnalysis` with citation grounding retained.
3. Unit tests: flowchart caption → `diagram_analysis` schema_id; plot caption → `chart_extraction`; existing grounded tests still pass.
