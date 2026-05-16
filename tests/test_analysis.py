"""Contract tests for the S02 in-memory daily analysis surface.

These tests intentionally describe the next slice behavior before the
implementation exists. They must not call live arXiv, YAKE, or persistence.
"""

from __future__ import annotations

import subprocess
from dataclasses import replace
from datetime import date, datetime
from pathlib import Path
from typing import Any

import pytest

from arxiv_archive.arxiv_client import ArxivPaper
from arxiv_archive.scoring import ScoredPaper

RUN_DATE = date(2026, 5, 14)


def make_paper(index: int) -> ArxivPaper:
    """Build a minimal arXiv paper fixture with stable sortable fields."""
    return ArxivPaper(
        id=f"2605.{index:05d}",
        title=f"Paper {index}",
        abstract=f"Abstract for paper {index}",
        authors=[f"Author {index}"],
        published=RUN_DATE,
        updated=RUN_DATE,
        categories=["cs.AI"],
        pdf_url=f"https://arxiv.org/pdf/2605.{index:05d}.pdf",
    )


def make_scored(paper: ArxivPaper, score: float, keywords: list[str] | None = None) -> ScoredPaper:
    """Build a scored-paper fixture without invoking the production scorer."""
    return ScoredPaper(
        paper=paper,
        semschol=None,
        keywords=keywords or ["agent", "archive"],
        score=score,
        breakdown={"fixture": score},
    )


class FakeArxivClient:
    papers: list[ArxivPaper] = []
    calls: list[dict[str, Any]] = []

    def fetch_papers(
        self,
        start_date: date,
        end_date: date | None = None,
        categories: list[str] | None = None,
    ) -> list[ArxivPaper]:
        self.calls.append(
            {
                "start_date": start_date,
                "end_date": end_date,
                "categories": categories,
            }
        )
        return list(self.papers)


class FakeKeywordExtractor:
    calls: list[tuple[str, str]] = []

    def extract_for_paper(self, title: str, abstract: str) -> list[str]:
        self.calls.append((title, abstract))
        return [title.lower().replace(" ", "-"), "archive"]


class FakeScoringEngine:
    calls: list[tuple[ArxivPaper, object, list[str]]] = []

    def score(self, paper: ArxivPaper, semschol: object, keywords: list[str]) -> ScoredPaper:
        self.calls.append((paper, semschol, keywords))
        # Deliberately make lower-index papers score higher than higher-index papers
        # so the test fails if run_analysis preserves fetch order instead of sorting.
        numeric_id = int(paper.id.split(".")[-1])
        return make_scored(paper, score=100.0 - numeric_id, keywords=keywords)


@pytest.fixture(autouse=True)
def reset_fakes() -> None:
    FakeArxivClient.papers = []
    FakeArxivClient.calls = []
    FakeKeywordExtractor.calls = []
    FakeScoringEngine.calls = []


@pytest.fixture
def patch_analysis_components(monkeypatch: pytest.MonkeyPatch) -> None:
    import arxiv_archive.cli as cli

    monkeypatch.setattr(cli, "ArxivClient", FakeArxivClient)
    monkeypatch.setattr(cli, "KeywordExtractor", FakeKeywordExtractor)
    monkeypatch.setattr(cli, "ScoringEngine", FakeScoringEngine)

    def fail_if_persisted(*_args: object, **_kwargs: object) -> Path:
        raise AssertionError("run_analysis() must not call save_session(); persistence belongs to S03")

    monkeypatch.setattr(cli, "save_session", fail_if_persisted)


def test_run_analysis_returns_done_daily_analysis_sorted_and_capped(
    patch_analysis_components: None,
) -> None:
    from arxiv_archive.cli import DailyAnalysis, run_analysis

    # More than 10 and intentionally reversed to prove score-desc sorting and top-10 capping.
    FakeArxivClient.papers = [make_paper(index) for index in range(12, 0, -1)]

    analysis = run_analysis(RUN_DATE)

    assert isinstance(analysis, DailyAnalysis)
    assert analysis.run_date == RUN_DATE
    assert analysis.status == "done"
    assert analysis.papers_fetched == 12
    assert [paper.score for paper in analysis.papers] == sorted(
        [paper.score for paper in analysis.papers], reverse=True
    )
    assert [paper.paper.id for paper in analysis.papers[:3]] == ["2605.00001", "2605.00002", "2605.00003"]
    assert len(analysis.top_papers) == 10
    assert analysis.top_papers == analysis.papers[:10]
    assert isinstance(analysis.analysis_timestamp, datetime)

    assert FakeArxivClient.calls == [
        {
            "start_date": RUN_DATE,
            "end_date": RUN_DATE,
            "categories": ["cs.AI", "cs.LG", "cs.CL", "cs.CV", "cs.IR", "cs.KG", "cs.SI"],
        }
    ]
    assert len(FakeKeywordExtractor.calls) == 12
    assert len(FakeScoringEngine.calls) == 12


def test_run_analysis_returns_empty_without_scoring_or_persistence(
    patch_analysis_components: None,
) -> None:
    from arxiv_archive.cli import run_analysis

    FakeArxivClient.papers = []

    analysis = run_analysis(RUN_DATE)

    assert analysis.run_date == RUN_DATE
    assert analysis.status == "empty"
    assert analysis.papers_fetched == 0
    assert analysis.papers == []
    assert analysis.top_papers == []
    assert isinstance(analysis.analysis_timestamp, datetime)
    assert FakeKeywordExtractor.calls == []
    assert FakeScoringEngine.calls == []


def test_run_analysis_propagates_dependency_failures(monkeypatch: pytest.MonkeyPatch) -> None:
    import arxiv_archive.cli as cli

    class ExplodingClient:
        def __init__(self) -> None:
            raise RuntimeError("arXiv client unavailable")

    monkeypatch.setattr(cli, "ArxivClient", ExplodingClient)

    with pytest.raises(RuntimeError, match="arXiv client unavailable"):
        cli.run_analysis(RUN_DATE)


def test_run_analysis_fails_on_malformed_fetched_paper(
    patch_analysis_components: None,
) -> None:
    from arxiv_archive.cli import run_analysis

    malformed = replace(make_paper(1), abstract=None)  # type: ignore[arg-type]
    FakeArxivClient.papers = [malformed]

    with pytest.raises((TypeError, AttributeError)):
        run_analysis(RUN_DATE)


def test_cli_run_outputs_done_summary_without_live_dependencies(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    import arxiv_archive.cli as cli

    analysis = cli.DailyAnalysis(
        run_date=RUN_DATE,
        status="done",
        papers_fetched=2,
        papers=[make_scored(make_paper(1), 9.0), make_scored(make_paper(2), 8.0)],
        top_papers=[make_scored(make_paper(1), 9.0), make_scored(make_paper(2), 8.0)],
        analysis_timestamp=datetime(2026, 5, 14, 12, 0, 0),
    )
    monkeypatch.setattr(cli, "run_analysis", lambda run_date: analysis)

    cli.run("2026-05-14")

    output = capsys.readouterr().out
    assert "status: done" in output
    assert "date: 2026-05-14" in output
    assert "papers fetched: 2" in output
    assert "top papers: 2" in output


def test_cli_run_outputs_empty_summary_without_live_dependencies(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    import arxiv_archive.cli as cli

    analysis = cli.DailyAnalysis(
        run_date=RUN_DATE,
        status="empty",
        papers_fetched=0,
        papers=[],
        top_papers=[],
        analysis_timestamp=datetime(2026, 5, 14, 12, 0, 0),
    )
    monkeypatch.setattr(cli, "run_analysis", lambda run_date: analysis)

    cli.run("2026-05-14")

    output = capsys.readouterr().out
    assert "status: empty" in output
    assert "date: 2026-05-14" in output
    assert "papers fetched: 0" in output
    assert "top papers: 0" in output


def test_cli_malformed_date_fails_typer_validation() -> None:
    result = subprocess.run(
        [
            "uv",
            "run",
            "python",
            "-m",
            "arxiv_archive",
            "--date",
            "not-a-date",
        ],
        check=False,
        cwd=Path(__file__).resolve().parents[1],
        text=True,
        capture_output=True,
        timeout=30,
    )

    assert result.returncode == 2
    combined_output = f"{result.stdout}\n{result.stderr}"
    assert "date must be in YYYY-MM-DD format" in combined_output
    assert "empty" not in combined_output.lower()
