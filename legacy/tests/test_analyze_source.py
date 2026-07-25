"""M200 S01: analyze_source typed tracer contract tests (no network/LLM)."""

from __future__ import annotations

from research_graph.application.analyze_source import (
    AnalyzeSourceRequest,
    AnalyzeSourceUseCase,
    analyze_source,
)
from research_graph.application.profiles.paper import PAPER_STAGE_ORDER

_TEXT = [
    "transformers enable attention mechanisms for reasoning",
    "attention improves transformers reasoning quality",
    "transformers need attention to reason over long context",
]


def test_empty_text_parts_returns_empty_without_stages() -> None:
    result = analyze_source("arxiv:test.empty", [])
    assert result.status == "empty"
    assert result.stage_names == ()
    assert result.pipeline_context is None
    assert result.diagnostic == "empty_text_parts"
    assert result.safety["graph_writes_authorized"] is False
    assert result.safety["production_import_authorized"] is False


def test_whitespace_only_text_parts_empty() -> None:
    result = AnalyzeSourceUseCase().run(
        AnalyzeSourceRequest(source_id="s", text_parts=["  ", "\n"])
    )
    assert result.status == "empty"


def test_analyze_source_runs_all_paper_stages() -> None:
    result = analyze_source("arxiv:2605.18747", _TEXT)
    assert result.status == "done"
    assert result.source_id == "arxiv:2605.18747"
    assert result.stage_names == PAPER_STAGE_ORDER
    assert result.pipeline_context is not None
    # text_parts seed + each stage writes an output key
    assert "text_parts" in result.stage_output_keys
    for stage in PAPER_STAGE_ORDER:
        assert stage in result.stage_output_keys
    assert result.diagnostic is None
    assert result.safety["graph_writes_authorized"] is False
    assert result.safety["fact_promotion_authorized"] is False


def test_analyze_source_uses_orchestrator_path() -> None:
    """Inject a spy orchestrator factory to prove L4 seam is used."""
    calls: list[str] = []

    class _SpyOrch:
        def __init__(self, pipeline):
            self.pipeline = pipeline

        def run(self, context):
            calls.append("run")
            # Delegate to real SyncDispatch path
            from research_graph.application.orchestrator import PipelineOrchestrator, SyncDispatch

            return PipelineOrchestrator(pipeline=self.pipeline, dispatch=SyncDispatch()).run(
                context
            )

    uc = AnalyzeSourceUseCase(orchestrator_factory=lambda p: _SpyOrch(p))
    result = uc.run(AnalyzeSourceRequest(source_id="spy", text_parts=_TEXT))
    assert result.status == "done"
    assert calls == ["run"]
