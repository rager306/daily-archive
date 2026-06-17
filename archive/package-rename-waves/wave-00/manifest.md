# Package Rename Wave 00 Manifest

Milestone: M088-umuqj7

Status: skeleton-only.

Purpose: create the target `research_graph` package namespace and bounded-context subpackages before moving implementation modules out of `arxiv_archive`.

## Created packages

```text
src/research_graph/
src/research_graph/corpus/
src/research_graph/corpus/ingestion/
src/research_graph/corpus/parsing/
src/research_graph/corpus/sources/
src/research_graph/papers/
src/research_graph/papers/artifacts/
src/research_graph/graph/
src/research_graph/staging/
src/research_graph/identity/
src/research_graph/llm/
src/research_graph/evaluation/
src/research_graph/repair/
src/research_graph/workflows/
```

## Scope

No implementation modules moved in wave 00.

No `arxiv_archive` files were archived by this wave. Prior archive waves remain under `archive/package-layout-shims/`.

No import rewrites from `arxiv_archive.*` to `research_graph.*` were performed, except new guard tests that intentionally import the skeleton.

## Verification contract

- `import research_graph` succeeds.
- Key subpackage imports succeed.
- Guard tests verify expected skeleton packages exist.
- `python3 -m py_compile` passes for all skeleton files.

## Next wave

Move already-canonical low-risk modules from `arxiv_archive.artifacts` and `arxiv_archive.llm` into `research_graph.papers` and `research_graph.llm` with archive manifests and `Formerly:` breadcrumbs.
