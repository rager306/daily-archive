# Package Rename Wave 02 Manifest

Milestone: M090-4dcb1y

Purpose: move article artifact runtime, model contract, and batch validation modules from the old `arxiv_archive` top level into `research_graph.papers.artifacts`.

## Moves

| Old path | New canonical path | Archive path | Status |
|---|---|---|---|
| `src/arxiv_archive/article_artifact_worker.py` | `src/research_graph/papers/artifacts/worker.py` | `archive/package-rename-waves/wave-02/src/arxiv_archive/article_artifact_worker.py` | archived implementation |
| `src/arxiv_archive/article_artifacts.py` | `src/research_graph/papers/artifacts/models.py` | `archive/package-rename-waves/wave-02/src/arxiv_archive/article_artifacts.py` | archived implementation |
| `src/arxiv_archive/article_batch_validation.py` | `src/research_graph/papers/artifacts/batch_validation.py` | `archive/package-rename-waves/wave-02/src/arxiv_archive/article_batch_validation.py` | archived implementation |

## Breadcrumb rule

Each new canonical module contains a `Formerly: src/arxiv_archive/...` breadcrumb.

## Intentional breakage

The moved `arxiv_archive.article_artifact_worker`, `arxiv_archive.article_artifacts`, and `arxiv_archive.article_batch_validation` import paths are no longer runtime canonical paths after this wave. Internal code and tests should import from `research_graph.papers.artifacts.*`.

## Verification contract

- direct old import search for moved module paths is clean in `src`, `tests`, and `scripts`;
- artifact model/property/modular, worker, e2e, CLI, scaffold, and related tests pass;
- `python3 -m py_compile` passes for moved modules and affected importers;
- import smoke for wave-02 `research_graph` modules passes;
- GitNexus detect_changes is reviewed.
