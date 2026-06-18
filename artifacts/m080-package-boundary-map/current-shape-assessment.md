# M080 Current Shape Assessment

## Inventory facts

Generated inventory found:

- 68 top-level Python modules in `src/arxiv_archive/` excluding `__init__.py` and `__main__.py`.
- 9 existing subpackages: `assets`, `chunking`, `identity`, `indexing`, `ingestion`, `llm`, `parsing`, `quality`, and `staging`.
- 147 internal import edges.
- 0 AST parse errors.

## Already structured

These areas already have package boundaries and should not be disrupted first:

- `arxiv_archive.llm`: newly created in M079 for provider config and future MiniMax/GLM/Z.ai adapter work.
- `arxiv_archive.parsing`: already contains parser structure code and is imported by several top-level modules.
- `arxiv_archive.ingestion`: already centralizes loader-style behavior and appears among most imported targets.
- `arxiv_archive.indexing`: already holds navigation/page-index style support.
- `arxiv_archive.quality`: already groups maintainability/threshold/report logic.
- `arxiv_archive.chunking`: already exists and should absorb chunk-specific top-level modules only after impact checks.

## Mismatch: article artifact modules are top-level

Top-level modules such as:

- `article_artifact_metrics.py`
- `article_artifact_minimax.py`
- `article_artifact_reducer.py`
- `article_artifact_worker.py`
- `article_artifacts.py`
- `article_assets.py`
- `article_evidence_bridge.py`

represent one bounded context but are spread flat at package root. A future `arxiv_archive.artifacts` package is a strong candidate, but this should be phased because `article_artifacts.py` and `article_artifact_minimax.py` have incoming imports.

## Mismatch: graph_readiness modules are top-level

Top-level modules such as:

- `graph_readiness.py`
- `graph_readiness_export.py`
- `graph_readiness_extraction_gate.py`
- `graph_readiness_manifest.py`
- `graph_readiness_persistence.py`
- `graph_readiness_retrieval_validation.py`
- `graph_readiness_review.py`

are a clear domain cluster. They should eventually become `arxiv_archive.graph_readiness.*`, but `graph_readiness.py` is among the most imported targets and should not be moved without compatibility shims and targeted tests.

## Mismatch: universal KB and queue modules are top-level

Top-level modules such as:

- `universal_kb_contracts.py`
- `universal_kb_queue.py`
- `universal_kb_sidecar_boundary.py`
- `universal_kb_smoke.py`
- `universal_kb_substrate_rehearsal.py`

mix contracts, queue behavior, rehearsal/smoke utilities, and sidecar boundary logic. A future split may need more than one package, likely `arxiv_archive.queue` and `arxiv_archive.universal_kb` or `arxiv_archive.kb`.

## Mismatch: extraction and benchmark modules are top-level

Top-level modules such as:

- `extraction_benchmark.py`
- `scientific_extraction.py`
- `dspy_extraction.py`
- `evaluation.py`

should not all be grouped blindly. `dspy_extraction.py` is optimizer/provider-adjacent and may belong closer to `llm` or future optimizer code, while `extraction_benchmark.py` is benchmark/gate infrastructure.

## Mismatch: validation batch workflow modules are top-level

Top-level modules such as:

- `validation_batch_provenance.py`
- `validation_batch_state.py`
- `validation_batch_workflow.py`
- `validation_logging.py`

look like a workflow/runtime cluster. Because `validation_batch_state.py` has incoming imports, this group should be moved later with a shim and workflow tests.

## Defer

Defer these moves until explicit impact analysis and tests exist:

- `full_text.py`: most imported target in inventory.
- `universal_kb_contracts.py`: shared contract module with multiple incoming imports.
- `graph_readiness.py`: central graph-readiness entry point.
- `models_registry.py`: model/provider registry with LLM relevance and existing tests.
- `page_index.py` / `article_page_index.py`: naming overlap with `indexing.page_index` requires careful consolidation.

## LLM conclusion

The M079 LLM package boundary is the right pattern: create a package, move implementation into canonical path, leave old top-level path as compatibility shim, and add tests proving old/new import equivalence. Future package moves should copy that pattern instead of doing broad find-and-replace.

## Assessment

The repo-level `src/` layout is sound. The remaining structure debt is internal package organization: too many domain modules live directly under `src/arxiv_archive/`. The safe next step is not moving more files immediately; it is a target package map and phased migration order based on incoming imports and test coverage.
