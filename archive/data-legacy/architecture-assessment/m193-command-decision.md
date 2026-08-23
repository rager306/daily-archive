# M193 Command Decision

## Decision

Use the current-layout graph-readiness review command:

```bash
uv run python -m research_graph.infrastructure.graph.readiness.review \
  --review-dir <review-dir> \
  --events <events.jsonl> \
  --validate-only \
  --require-completed-review
```

Retire the historical runtime command without adding a shim:

```bash
uv run python -m arxiv_archive.graph_readiness_review ...
```

## GSD decision

Recorded in GSD decision register as **D108**.

## Rationale

- `src/research_graph/infrastructure/graph/readiness/review.py` is the canonical current-layout module.
- `tests/test_research_graph_package_skeleton.py::test_wave_18_archives_graph_readiness_without_runtime_shims` explicitly verifies the old `src/arxiv_archive` runtime module does not exist.
- The canonical current-layout module exposes `--validate-only` and `--require-completed-review` semantics.
- Adding an `arxiv_archive` runtime shim would violate the package-rename governance.

## Boundary

This decision resolves command-layout drift only. It does not promote:

- import eligibility;
- semantic KG readiness;
- graph import readiness;
- production graph persistence readiness;
- LadybugDB production write readiness;
- production retrieval quality;
- DSPy/RLM optimizer readiness.

## Downstream instruction

Future graph-readiness review post-checks should use the current-layout command above. If existing docs mention `arxiv_archive.graph_readiness_review`, treat them as historical references unless a future milestone explicitly changes package-skeleton governance.
