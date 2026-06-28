# M194 Expected Correction Map

## Verdict contract

M194 corrects active architecture documentation references only. It does not rewrite historical artifacts, source migration breadcrumbs, or milestone trajectory context.

## Target files

- `doc/architecture/m030_module_function_readiness.json`
- `doc/architecture/m030_module_function_readiness.md`
- `doc/architecture/m030_next_implementation_roadmap.json`
- `doc/architecture/m030_next_implementation_roadmap.md`
- `doc/architecture/m030_pipeline_module_inventory.json`
- `doc/architecture/m030_pipeline_module_inventory.md`
- `doc/architecture/m030_process_continuity_audit.json`
- `doc/architecture/m030_requirement_module_matrix.json`
- `doc/architecture/m030_requirement_module_matrix.md`

## Exact replacements

Apply only to target files:

| Old | New |
|---|---|
| `uv run python -m arxiv_archive.graph_readiness_review --review-dir <review-dir> --events <events.jsonl> --validate-only --require-completed-review` | `uv run python -m research_graph.infrastructure.graph.readiness.review --review-dir <review-dir> --events <events.jsonl> --validate-only --require-completed-review` |
| `python module arxiv_archive.graph_readiness_review` | `python module research_graph.infrastructure.graph.readiness.review` |
| `arxiv_archive.graph_readiness_review validate-only` | `research_graph.infrastructure.graph.readiness.review validate-only` |
| `src/arxiv_archive/graph_readiness_review.py` | `src/research_graph/infrastructure/graph/readiness/review.py` |
| `src/arxiv_archive/graph_readiness_review.py:generate_review_bundles` | `src/research_graph/infrastructure/graph/readiness/review.py:generate_review_bundles` |
| `src/arxiv_archive/graph_readiness_review.py:select_review_papers` | `src/research_graph/infrastructure/graph/readiness/review.py:select_review_papers` |
| `src/arxiv_archive/graph_readiness_review.py:render_review_bundle` | `src/research_graph/infrastructure/graph/readiness/review.py:render_review_bundle` |
| `src/arxiv_archive/graph_readiness_review.py:validate_review_artifacts` | `src/research_graph/infrastructure/graph/readiness/review.py:validate_review_artifacts` |

Note: replacements should be applied longest-first so function-qualified paths do not become partially rewritten twice.

## No-touch exclusions

Do not edit:

- `.gsd/**`
- `archive/**`
- `mutants/**`
- `artifacts/**`
- `data/article_corpora/m031-*`
- `data/article_corpora/m033-*`
- `data/architecture-assessment/m19*`
- `src/research_graph/infrastructure/graph/readiness/review.py` migration breadcrumb

## Expected labels

- `active_reference_targets_identified=true`
- `expected_correction_map_written=true`
- `historical_artifacts_excluded=true`
- `source_breadcrumb_excluded=true`
- `source_code_edited=false`
- `runtime_shim_added=false`
- `import_eligible=false`
- `graph_ready=false`
- `production_import_attempted=false`
- `ladybugdb_written=false`
- `optimizer_enabled=false`

## Stop conditions

Stop before edits if:

- a target file is missing;
- a JSON target is not parseable before edits;
- active target refs differ from this map;
- a required old reference appears only in historical exclusions;
- an edit would touch source code or a migration breadcrumb;
- an edit would touch `.gsd`, archives, mutants, artifacts, or M031/M033 corpus evidence.
