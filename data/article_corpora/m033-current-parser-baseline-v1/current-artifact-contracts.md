# M033 S01 T02: Current Artifact Contracts

This map records the current daily-archive artifact contracts that external parser outputs must preserve or improve.

| Stage | Inputs | Outputs | Key contract | Primary artifact status | Downstream consumers |
|---|---|---|---|---|---|
| catalog_intake | `data/article_catalog/index.json`<br>`data/article_catalog/catalog.json` | `selection.json` | requested refs<br>catalog_backed_count<br>typed blockers<br>metadata-only safety flags | `data/article_corpora/m031-catalog-backed-replay-v1/selection.json` exists | source_acquisition<br>loader_evidence |
| source_acquisition | `selection.json`<br>`data/article_catalog/**/article.json`<br>`local source artifacts` | `source-acquisition-summary.json`<br>`source-acquisition-diagnostics.jsonl`<br>`source-acquisition-report.md` | captured local paths<br>sha256<br>byte size<br>terminal blockers<br>no network fetch | `data/article_corpora/m031-catalog-backed-replay-v1/source-acquisition-summary.json` exists | loader_evidence<br>parser_conversion |
| loader_evidence | `selection.json`<br>`source-acquisition-summary.json` | `loader-evidence-summary.json`<br>`loader-evidence-diagnostics.jsonl`<br>`loader-evidence-report.md` | loaded rows<br>metadata-only loaded rows<br>loader blockers<br>failed=0 | `data/article_corpora/m031-catalog-backed-replay-v1/loader-evidence-summary.json` exists | parser_conversion |
| parser_conversion | `loader-evidence-summary.json`<br>`captured local source artifacts` | `conversion-quality/conversion-quality-summary.json`<br>`parser-conversion-closeout-summary.json`<br>`converted-text/*.txt` | parser-ready converted text only for usable artifacts<br>refusal diagnostics<br>hashes<br>no graph/chunk claims | `data/article_corpora/m031-catalog-backed-replay-v1/parser-conversion-closeout-summary.json` exists | chunk_evidence |
| chunk_evidence | `parser-conversion-closeout-summary.json`<br>`converted text path/hash` | `chunk-evidence/chunk-evidence-summary.json`<br>`chunk-evidence-closeout-summary.json` | stable chunk ids<br>section anchors<br>source spans<br>zero-chunk refusals | `data/article_corpora/m031-catalog-backed-replay-v1/chunk-evidence-closeout-summary.json` exists | graph_readiness_handoff<br>no_write_import_boundary |
| graph_readiness_handoff | `chunk-evidence summary`<br>`review corpus` | `graph-readiness-review/*.md`<br>`independent-review-summary.md` | pending independent review<br>require completed review before import eligibility | `data/article_corpora/m031-catalog-backed-replay-v1/graph-readiness-review/independent-review-summary.md` exists | no_write_import_boundary |
| no_write_import_boundary | `graph readiness packets`<br>`chunk evidence closeout` | `import-boundary-rehearsal/import-boundary-summary.json`<br>`s05-closeout-summary.json`<br>`m031-continuity-audit.json` | accepted_count=0<br>import_eligible_count=0<br>ladybugdb_written=false<br>production_import_attempted=false | `data/article_corpora/m031-catalog-backed-replay-v1/s05-closeout-summary.json` exists | milestone validation<br>M033 comparison baseline |

## External comparison implications

- GROBID/OpenDataLoader outputs must be evaluated against provenance, blockers, diagnostics, and no-write safety flags, not only text quality.
- Improvements are most valuable where the current baseline is weak: layout, tables, figures/captions, bibliography, reading order, section hierarchy, and coordinate/source spans.
- No external parser output is import-ready by default.
