# M184 Test Contract Alignment

## Verdict

**Pilot seam test contract: active.**

## Stable contracts

1. `build_current_catalog_index_selection(index_path)` builds an `article-corpus-selection.v00.01` payload from canonical index rows with `article_ref` and `source_code`.
2. `verify_article_catalog.run([program])` creates a temporary selection from `DEFAULT_INDEX` and delegates to `run_core` with `--catalog`, `--index`, `--selection`, `--validate-only`, `--require-index`, and `--check-index-titles`.
3. `verify_article_catalog.run(explicit_argv)` delegates explicit arguments unchanged to `run_core`.
4. Tests must not require network or mutate canonical catalog data.

## Why this is enough

The pilot extracts one small application helper and keeps historical CLI behavior intact. The tests assert behavior at the helper and wrapper boundary, not private implementation details.
