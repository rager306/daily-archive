# Naming Decision: `research_graph`

## Status

Accepted for migration planning.

## Decision

Use `research_graph` as the target top-level importable Python package, replacing `arxiv_archive` through staged migration waves.

## Why not `arxiv_archive`

`arxiv_archive` describes an input source and early archive-shaped scope. It does not describe the current system, which now includes:

- corpus ingestion and parsing;
- paper evidence/artifact extraction;
- graph-readiness contracts;
- staging/import boundaries;
- LLM helper boundaries;
- evaluation and quality gates;
- workflow orchestration.

The name also creates confusion when the project handles non-arXiv sources or graph-specific outputs.

## Why not `daily_archive`

`daily_archive` matches the repository name better but remains vague and operational. It sounds like a scheduled archive job, not a research-evidence-to-graph system.

## Why not multiple packages immediately

Multiple top-level packages may be a future outcome, but they add premature packaging complexity while boundaries are still being cleaned up. The current safer path is one coherent package with explicit bounded-context subpackages.

## Chosen shape

```text
src/research_graph/
├── corpus/
├── papers/
├── graph/
├── staging/
├── identity/
├── llm/
├── evaluation/
├── repair/
├── workflows/
└── cli.py
```

## Compatibility stance

The project cleanup direction after M086 is archive-first, not indefinite compatibility shims. Old `arxiv_archive` files should be archived wave-by-wave once their `research_graph` replacement is verified.

Canonical modules must include breadcrumbs:

```text
Formerly: src/arxiv_archive/<old_path>.py
```

## Revisit criteria

Revisit this decision only if:

- the project becomes a library family with independently versioned distributions;
- a production user requires long-lived `arxiv_archive` import compatibility;
- `research_graph` conflicts with an external package or naming policy;
- graph output stops being the primary organizing outcome.
