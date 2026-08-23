# M193 GitNexus Command Scope

## Verdict

**M193 scope is current-layout graph-readiness review command resolution without restoring `arxiv_archive` runtime shims.**

## GitNexus evidence

| Evidence | Result |
|---|---|
| Query: `graph_readiness_review arxiv_archive research_graph module validate-only require-completed-review output_contract_completed` | Found canonical graph-readiness review surfaces and package-skeleton retirement tests. |
| Context: `Function:src/research_graph/infrastructure/graph/readiness/review.py:main` | Canonical current-layout CLI entrypoint. Calls `generate_review_bundles` and `validate_review_artifacts`. |
| Impact: `review.py:main` | LOW, impactedCount=1, affected_processes=0. |
| Context: `Function:src/research_graph/infrastructure/graph/readiness/review.py:validate_review_artifacts` | Completed-review validator used by CLI validate-only mode. |
| Impact: `validate_review_artifacts` | LOW, impactedCount=2, affected_processes=0. |

## Package skeleton evidence

`tests/test_research_graph_package_skeleton.py::test_wave_18_archives_graph_readiness_without_runtime_shims` asserts:

- old runtime path `src/arxiv_archive/graph_readiness_review.py` does not exist;
- archived copy exists under `archive/package-rename-waves/wave-18/`;
- canonical current module exists at `src/research_graph/infrastructure/graph/readiness/review.py`;
- canonical module imports as `research_graph.infrastructure.graph.readiness.review`.

Live check:

- `gsd_exec[c70093e9-b965-4626-9740-1c2eecba029b]`: `1 passed, 21 deselected`.

## Canonical command

Use current-layout command:

```bash
uv run python -m research_graph.infrastructure.graph.readiness.review \
  --review-dir <review-dir> \
  --events <events.jsonl> \
  --validate-only \
  --require-completed-review
```

## Retired command

Do not restore or shim:

```bash
uv run python -m arxiv_archive.graph_readiness_review ...
```

The historical command is retired with the `arxiv_archive` runtime package. M193 may update governance artifacts and command guidance, but should not add `src/arxiv_archive` compatibility shims.

## Scope rule

M193 resolves command-layout drift only. It does not promote import eligibility, graph readiness, production persistence, production retrieval quality, or optimizer readiness.
