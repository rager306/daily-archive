# M085 Artifact MiniMax Boundary Package Move Summary

## Canonical path

The pure article artifact MiniMax request/validation boundary now lives at:

```text
arxiv_archive.artifacts.minimax_boundary
src/arxiv_archive/artifacts/minimax_boundary.py
```

## Compatibility shim

The old import path remains available:

```text
arxiv_archive.article_artifact_minimax
src/arxiv_archive/article_artifact_minimax.py
```

The old module explicitly re-exports the public boundary surface: schema/tool constants, request mode, binding id, request/result/work dataclasses, request builder, response validator, classifier request builder, and hint schema helper.

## Boundary clarification

`arxiv_archive.artifacts.minimax_boundary` is still pure request/validation code. It prepares Anthropic-compatible MiniMax forced-tool requests and validates already-received tool-use responses.

It does not perform live HTTP calls. The live/mock transport boundary remains in `article_artifact_worker.py` and was not moved in M085.

LLM provider configuration remains under `arxiv_archive.llm` and was not moved in M085.

## Repo import updates

Updated direct imports to prefer the canonical path in:

- `src/arxiv_archive/rlm_workflow.py`
- `src/arxiv_archive/cli.py`
- `src/arxiv_archive/article_artifact_worker.py`
- `tests/test_article_artifact_minimax.py`
- `tests/test_m050_article_artifact_worker.py`
- `tests/test_m050_e2e_pipeline.py`
- `tests/test_article_artifacts_cli.py`
- `scripts/verify_m023_artifact_scaffold_gate.py`

Added a compatibility test proving the legacy module re-exports representative canonical objects.

## GitNexus blast radius

Before moving MiniMax boundary code, GitNexus impact checks were run:

- `request_article_artifact_classification`: LOW risk; affects `run_document_workflow`.
- `build_article_artifact_minimax_request`: LOW risk; affects CLI helper flow.
- `validate_article_artifact_minimax_response`: LOW risk; affects CLI and workflow flows.
- `ArticleArtifactWorkRequest`: LOW risk; imported by workflow/worker/test code.

## Verification

Fresh targeted tests:

```bash
uv run pytest tests/test_article_artifact_minimax.py tests/test_m050_article_artifact_worker.py tests/test_m050_e2e_pipeline.py tests/test_article_artifacts_cli.py tests/test_m023_artifact_scaffold_gate.py -q
```

Result: **PASS** — 38 passed.

Fresh compile check:

```bash
python3 -m py_compile src/arxiv_archive/artifacts/minimax_boundary.py src/arxiv_archive/article_artifact_minimax.py src/arxiv_archive/article_artifact_worker.py src/arxiv_archive/rlm_workflow.py src/arxiv_archive/cli.py scripts/verify_m023_artifact_scaffold_gate.py
```

Result: **PASS**.

## Boundaries

- no live API calls
- no secrets collected or printed
- no graph writes
- no fact promotion
- no worker move
- no artifact model move
- no provider config move
- no broad package restructure
- no shim removal

## Next candidate

Future artifact moves can target `article_artifact_worker.py` or `article_artifacts.py`, but both are higher-risk than this boundary and should be separate milestones with broader tests.
