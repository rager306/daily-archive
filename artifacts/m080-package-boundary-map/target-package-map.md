# M080 Target Package Map

## Principles

- Keep repo-level `src/` layout unchanged.
- Move modules only by bounded context.
- Preserve old import paths with compatibility shims.
- Add tests proving old and new import paths resolve to the same canonical objects.
- Move high-incoming-import modules last.
- Do not combine semantic refactors with file moves.

## Target packages

### `arxiv_archive.llm` — already started

Status: exists after M079.

Canonical examples:

- `arxiv_archive.llm.provider_config`

Future candidates:

- `minimax_structured.py` -> `arxiv_archive.llm.minimax_structured`
- `models_registry.py` -> `arxiv_archive.llm.models_registry` or `arxiv_archive.models.registry` after impact review
- `dspy_extraction.py` -> defer; may belong to optimizer/extraction instead of LLM core

Risk: medium. Provider/model code has environment and benchmark implications.

### `arxiv_archive.artifacts`

Candidate modules:

- `article_artifacts.py`
- `article_artifact_metrics.py`
- `article_artifact_minimax.py`
- `article_artifact_reducer.py`
- `article_artifact_worker.py`
- `article_assets.py`
- `article_evidence_bridge.py`

Suggested target paths:

- `arxiv_archive.artifacts.model`
- `arxiv_archive.artifacts.metrics`
- `arxiv_archive.artifacts.minimax_boundary`
- `arxiv_archive.artifacts.reducer`
- `arxiv_archive.artifacts.worker`
- `arxiv_archive.artifacts.assets`
- `arxiv_archive.artifacts.evidence_bridge`

Risk: medium. `article_artifacts.py` and `article_artifact_minimax.py` have incoming imports. Move one file at a time with shims.

### `arxiv_archive.graph_readiness`

Candidate modules:

- `graph_readiness.py`
- `graph_readiness_export.py`
- `graph_readiness_extraction_gate.py`
- `graph_readiness_manifest.py`
- `graph_readiness_persistence.py`
- `graph_readiness_retrieval_validation.py`
- `graph_readiness_review.py`

Suggested target paths:

- `arxiv_archive.graph_readiness.core`
- `arxiv_archive.graph_readiness.export`
- `arxiv_archive.graph_readiness.extraction_gate`
- `arxiv_archive.graph_readiness.manifest`
- `arxiv_archive.graph_readiness.persistence`
- `arxiv_archive.graph_readiness.retrieval_validation`
- `arxiv_archive.graph_readiness.review`

Risk: high. `graph_readiness.py` is among most imported targets. Start with leaf modules only after impact analysis.

### `arxiv_archive.queue`

Candidate modules:

- `universal_kb_queue.py`
- `queue_replay.py`

Suggested target paths:

- `arxiv_archive.queue.universal_kb`
- `arxiv_archive.queue.replay`

Risk: medium. Queue payload metadata has recent requirements and tests; move only after targeted queue tests pass.

### `arxiv_archive.kb` or `arxiv_archive.universal_kb`

Candidate modules:

- `universal_kb_contracts.py`
- `universal_kb_sidecar_boundary.py`
- `universal_kb_smoke.py`
- `universal_kb_substrate_rehearsal.py`

Suggested target paths:

- `arxiv_archive.kb.contracts`
- `arxiv_archive.kb.sidecar_boundary`
- `arxiv_archive.kb.smoke`
- `arxiv_archive.kb.substrate_rehearsal`

Risk: high for `universal_kb_contracts.py` because it has multiple incoming imports. Prefer contracts-first API design before moving.

### `arxiv_archive.extraction`

Candidate modules:

- `extraction_benchmark.py`
- `scientific_extraction.py`
- `evaluation.py`
- `candidate_locators.py`

Suggested target paths:

- `arxiv_archive.extraction.benchmark`
- `arxiv_archive.extraction.scientific`
- `arxiv_archive.extraction.evaluation`
- `arxiv_archive.extraction.candidate_locators`

Risk: medium. Benchmark gates are recent and should keep deterministic fixture guarantees.

### `arxiv_archive.validation`

Candidate modules:

- `validation_batch_provenance.py`
- `validation_batch_state.py`
- `validation_batch_workflow.py`
- `validation_logging.py`

Suggested target paths:

- `arxiv_archive.validation.provenance`
- `arxiv_archive.validation.state`
- `arxiv_archive.validation.workflow`
- `arxiv_archive.validation.logging`

Risk: medium-high. Workflow modules should move after artifact/queue boundaries stabilize.

### `arxiv_archive.retrieval`

Candidate modules:

- `hybrid_retrieval.py`
- `keyword_extractor.py`
- `embedder.py`
- `article_retrieval_tables.py`

Suggested target paths:

- `arxiv_archive.retrieval.hybrid`
- `arxiv_archive.retrieval.keywords`
- `arxiv_archive.retrieval.embeddings`
- `arxiv_archive.retrieval.tables`

Risk: medium. Retrieval code crosses indexing, graph, and embedding concerns.

### Existing packages to keep

- `arxiv_archive.assets`
- `arxiv_archive.chunking`
- `arxiv_archive.identity`
- `arxiv_archive.indexing`
- `arxiv_archive.ingestion`
- `arxiv_archive.llm`
- `arxiv_archive.parsing`
- `arxiv_archive.quality`
- `arxiv_archive.staging`

Do not flatten or rename these packages in the package-layout cleanup.

## Compatibility shim pattern

For a move from:

```text
src/arxiv_archive/foo.py
```

to:

```text
src/arxiv_archive/domain/foo.py
```

use old path as shim:

```python
"""Compatibility shim for arxiv_archive.domain.foo."""

from arxiv_archive.domain.foo import *
```

For modules with important public names, prefer explicit imports and `__all__`, as M079 did for `llm_provider_config.py`.

## First safe candidates

1. Leaf article artifact helper modules with low incoming imports.
2. LLM-adjacent modules after provider config settles.
3. Extraction benchmark helpers after benchmark tests are stable.

## Defer until later

- `full_text.py`
- `universal_kb_contracts.py`
- `graph_readiness.py`
- `models_registry.py`
- `page_index.py` / `article_page_index.py` consolidation

These are central enough that moves should be dedicated milestones with explicit GitNexus impact analysis and broad targeted tests.
