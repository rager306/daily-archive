# `research_graph` Migration Waves

## Strategy

Do not rename the whole tree at once. Create `src/research_graph/` and migrate bounded contexts in waves. Each wave:

1. Runs GitNexus impact for edited/moved symbols.
2. Copies/moves canonical implementation into `research_graph`.
3. Adds `Formerly:` breadcrumb in the new file.
4. Rewrites internal imports and tests to `research_graph.*`.
5. Archives old `arxiv_archive` files under `archive/package-rename-waves/wave-XX/`.
6. Runs direct old-import search.
7. Runs targeted pytest and `py_compile`.
8. Runs `gitnexus_detect_changes`.
9. Writes a wave manifest.

## Wave 00 — skeleton and package guard

Create:

```text
src/research_graph/__init__.py
```

Possibly create empty packages:

```text
research_graph.corpus
research_graph.papers
research_graph.graph
research_graph.staging
research_graph.identity
research_graph.llm
research_graph.evaluation
research_graph.repair
research_graph.workflows
```

Update `pyproject.toml` only enough to include the new package under existing setuptools discovery. Do not remove `arxiv_archive` yet.

Checks:

```bash
uv run python - <<'PY'
import research_graph
print(research_graph.__name__)
PY
python3 -m py_compile src/research_graph/__init__.py
```

## Wave 01 — already-canonical artifacts and LLM config

Move from current canonical `arxiv_archive` subpackages into `research_graph`:

```text
src/arxiv_archive/artifacts/metrics.py          -> src/research_graph/papers/artifacts/metrics.py
src/arxiv_archive/artifacts/minimax_boundary.py -> src/research_graph/papers/artifacts/minimax_boundary.py
src/arxiv_archive/artifacts/reducer.py          -> src/research_graph/papers/artifacts/reducer.py
src/arxiv_archive/artifacts/assets.py           -> src/research_graph/papers/assets.py
src/arxiv_archive/artifacts/evidence_bridge.py  -> src/research_graph/papers/evidence.py
src/arxiv_archive/llm/provider_config.py        -> src/research_graph/llm/provider_config.py
```

Archive old current files under:

```text
archive/package-rename-waves/wave-01/src/arxiv_archive/...
```

Also preserve M086 archived old shims as historical input, but do not reintroduce them.

Checks:

```bash
uv run pytest tests/test_article_artifact_metrics.py tests/test_m050_article_artifact_reducer.py tests/test_article_assets.py tests/test_property_article_assets.py tests/test_article_evidence_bridge.py tests/test_property_article_evidence_bridge.py tests/test_article_artifact_minimax.py tests/test_llm_provider_config.py -q
python3 -m py_compile src/research_graph/papers/artifacts/metrics.py src/research_graph/papers/artifacts/minimax_boundary.py src/research_graph/papers/artifacts/reducer.py src/research_graph/papers/assets.py src/research_graph/papers/evidence.py src/research_graph/llm/provider_config.py
```

## Wave 02 — artifact worker and artifact models

Move the higher-risk article artifact runtime/model files:

```text
src/arxiv_archive/article_artifact_worker.py  -> src/research_graph/papers/artifacts/worker.py
src/arxiv_archive/article_artifacts.py        -> src/research_graph/papers/artifacts/models.py
src/arxiv_archive/article_batch_validation.py -> src/research_graph/papers/artifacts/batch_validation.py
```

Checks:

```bash
uv run pytest tests/test_m050_article_artifact_worker.py tests/test_m050_e2e_pipeline.py tests/test_article_artifacts_cli.py tests/test_m023_artifact_scaffold_gate.py tests/test_article_artifact_minimax.py -q
python3 -m py_compile src/research_graph/papers/artifacts/worker.py src/research_graph/papers/artifacts/models.py src/research_graph/papers/artifacts/batch_validation.py
```

No live MiniMax/GLM calls.

## Wave 03 — corpus ingestion and parsing

Move source ingestion/parsing modules:

```text
src/arxiv_archive/ingestion/* -> src/research_graph/corpus/ingestion/*
src/arxiv_archive/parsing/*   -> src/research_graph/corpus/parsing/*
src/arxiv_archive/arxiv_client.py -> src/research_graph/corpus/sources/arxiv_client.py
src/arxiv_archive/semantic_scholar.py -> src/research_graph/corpus/sources/semantic_scholar.py
src/arxiv_archive/source_asset_manifest.py -> src/research_graph/corpus/sources/source_asset_manifest.py
```

Checks:

```bash
uv run pytest tests/test_article_loader.py tests/test_pdf_downloader.py tests/test_parser_page_index.py tests/test_evidence_paths.py -q
```

## Wave 04 — paper indexing/retrieval

Move paper-specific indexing and retrieval:

```text
src/arxiv_archive/article_links_dedup.py      -> src/research_graph/papers/links.py
src/arxiv_archive/article_page_index.py       -> src/research_graph/papers/page_index.py
src/arxiv_archive/article_retrieval_tables.py -> src/research_graph/papers/retrieval_tables.py
src/arxiv_archive/indexing/navigation.py      -> src/research_graph/papers/page_index_navigation.py  # if still needed
src/arxiv_archive/indexing/page_index.py      -> src/research_graph/papers/page_index_core.py        # if still needed
```

Checks discovered by import search around `page_index`, `article_page_index`, `article_links_dedup`, and `article_retrieval_tables`.

## Wave 05 — LLM helpers

Move provider-adjacent helper modules:

```text
src/arxiv_archive/minimax_structured.py -> src/research_graph/llm/minimax_structured.py
src/arxiv_archive/minimax_usage.py      -> src/research_graph/llm/minimax_usage.py
```

Checks: no live provider calls; MiniMax/GLM env policy remains namespaced.

## Wave 06 — graph readiness

Move graph group only after a dedicated slice checks boundaries:

```text
graph_readiness.py                      -> research_graph.graph.readiness
graph_readiness_review.py               -> research_graph.graph.review
graph_readiness_export.py               -> research_graph.graph.export
graph_readiness_manifest.py             -> research_graph.graph.manifest
graph_readiness_persistence.py          -> research_graph.graph.persistence
graph_readiness_retrieval_validation.py -> research_graph.graph.retrieval_validation
rlm_graph_traversal.py                  -> research_graph.graph.traversal
ladybug_client.py                       -> research_graph.graph.client
```

Mandatory review check for relevant work:

```bash
uv run python -m research_graph.graph.review --review-dir <review-dir> --events <events.jsonl> --validate-only --require-completed-review
```

## Wave 07 — staging and identity

Move:

```text
src/arxiv_archive/staging/* -> src/research_graph/staging/*
src/arxiv_archive/identity/* -> src/research_graph/identity/*
```

Keep staging separate from graph readiness: staging assembles candidates; graph validates readiness.

## Wave 08 — evaluation and repair

Move extraction/evaluation/repair modules:

```text
scientific_extraction.py
extraction_benchmark.py
dspy_extraction.py
evaluation.py
scoring.py
evidence.py
hybrid_retrieval.py
embedder.py
bounded_chunk_repair.py
chunk_import_contract.py
chunk_repair_contract.py
```

Do not introduce DSPy optimizer/live provider behavior during layout moves.

## Wave 09 — workflows and CLI

Move last:

```text
rlm_workflow.py                -> research_graph.workflows.rlm
validation_batch_*             -> research_graph.workflows.validation_batch
universal_kb_*                 -> research_graph.workflows.universal_kb
cli.py                         -> research_graph.cli
```

CLI entrypoints in `pyproject.toml`, if added later, should point at `research_graph.cli`.

## Wave 10 — retire `src/arxiv_archive`

Only after all direct imports are gone:

```bash
rg -n "arxiv_archive" src tests scripts pyproject.toml
```

Expected remaining references should be archive manifests, migration docs, and historical breadcrumbs only.

At this point, remove empty `src/arxiv_archive/` from the runtime package and keep archived history under:

```text
archive/package-rename-waves/
archive/package-layout-shims/
```
