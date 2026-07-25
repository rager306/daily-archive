"""M255 S03: optional PaperSummarizer wire into summary stage."""

from __future__ import annotations

from research_graph.infrastructure.llm.summarizer import PaperSummary
from research_graph.workflows.composition.paper_summary_stage import (
    DEFAULT_SUMMARY_STAGE_ENABLED,
    run_optional_paper_summary_stage,
    should_run_summary_stage,
)


class _FakeSummarizer:
    def __init__(self) -> None:
        self.calls = 0
        self.last_diagnostics = {
            "binding_id": "paper-summary-generate-default",
            "model_name": "agnes-ai/agnes-2.0-flash",
            "role": "default",
            "import_eligible": False,
        }

    def summarize(self, title: str, abstract: str) -> PaperSummary:
        self.calls += 1
        del title, abstract
        return PaperSummary(
            headline="H",
            what_it_does="Does A. Does B.",
            why_it_matters="Matters.",
            analogy="Think of it like a map.",
        )


def test_default_enabled_false() -> None:
    assert DEFAULT_SUMMARY_STAGE_ENABLED is False
    assert should_run_summary_stage(enabled=None) is False


def test_disabled_returns_none_without_call() -> None:
    fake = _FakeSummarizer()
    out = run_optional_paper_summary_stage(
        paper_id="p1",
        title="T",
        abstract="A",
        enabled=False,
        summarizer=fake,
    )
    assert out is None
    assert fake.calls == 0


def test_enabled_builds_ready_package() -> None:
    fake = _FakeSummarizer()
    pkg = run_optional_paper_summary_stage(
        paper_id="p1",
        title="T",
        abstract="A",
        enabled=True,
        summarizer=fake,
        role="default",
        binding_id="paper-summary-generate-default",
        model_name="agnes-ai/agnes-2.0-flash",
    )
    assert pkg is not None
    assert fake.calls == 1
    assert pkg.stage_status == "ready_for_review"
    assert pkg.import_eligible is False
    assert pkg.graph_writes_allowed is False
    assert pkg.headline == "H"
    assert pkg.binding_id == "paper-summary-generate-default"
    assert pkg.model_name == "agnes-ai/agnes-2.0-flash"
