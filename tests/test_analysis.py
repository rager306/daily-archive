"""Contract tests for the S02 in-memory daily analysis surface.

These tests intentionally describe the next slice behavior before the
implementation exists. They must not call live arXiv, YAKE, or persistence.
"""

from __future__ import annotations

import json
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



def make_s03_done_analysis() -> "DailyAnalysis":
    """Build a complete S03 DailyAnalysis fixture with scored and null-enriched papers."""
    import arxiv_archive.cli as cli
    from arxiv_archive.semantic_scholar import SemanticScholarPaper

    enriched = make_scored(make_paper(1), 9.5, keywords=["agent", "graph"])
    object.__setattr__(
        enriched,
        "semschol",
        SemanticScholarPaper(
            arxiv_id=enriched.paper.id,
            title=enriched.paper.title,
            citation_count=42,
            year=2026,
            venue="FixtureConf",
        ),
    )
    missing_semschol = make_scored(make_paper(2), 7.25, keywords=["archive"])

    return cli.DailyAnalysis(
        run_date=RUN_DATE,
        status="done",
        papers_fetched=2,
        papers=[enriched, missing_semschol],
        top_papers=[enriched, missing_semschol],
        analysis_timestamp=datetime(2026, 5, 14, 12, 30, 45),
    )


def make_s03_empty_analysis() -> "DailyAnalysis":
    """Build an empty S03 DailyAnalysis fixture without invoking live dependencies."""
    import arxiv_archive.cli as cli

    return cli.DailyAnalysis(
        run_date=RUN_DATE,
        status="empty",
        papers_fetched=0,
        papers=[],
        top_papers=[],
        analysis_timestamp=datetime(2026, 5, 14, 12, 30, 45),
    )


def make_s04_scored_analysis() -> "DailyAnalysis":
    """Build an S04 fixture with overlapping categories, keywords, and breakdown keys."""
    import arxiv_archive.cli as cli
    from arxiv_archive.semantic_scholar import SemanticScholarPaper

    graph_paper = replace(
        make_paper(1),
        categories=["cs.AI", "cs.CL"],
        title="Graph Agent Memory",
    )
    retrieval_paper = replace(
        make_paper(2),
        categories=["cs.AI", "cs.IR"],
        title="Retrieval Calibration",
    )
    graph_scored = ScoredPaper(
        paper=graph_paper,
        semschol=SemanticScholarPaper(
            arxiv_id=graph_paper.id,
            title=graph_paper.title,
            citation_count=12,
            year=2026,
            venue="FixtureConf",
        ),
        keywords=["agent", "graph"],
        score=9.5,
        breakdown={"citations": 2.0, "preference": 4.0},
    )
    retrieval_scored = ScoredPaper(
        paper=retrieval_paper,
        semschol=None,
        keywords=["agent", "retrieval"],
        score=7.25,
        breakdown={"citations": 6.0, "preference": 8.0},
    )

    return cli.DailyAnalysis(
        run_date=RUN_DATE,
        status="done",
        papers_fetched=2,
        papers=[graph_scored, retrieval_scored],
        top_papers=[graph_scored, retrieval_scored],
        analysis_timestamp=datetime(2026, 5, 14, 12, 30, 45),
    )


def patch_s04_artifact_dirs(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> tuple[Path, Path]:
    """Patch S04 artifact roots and fail loudly until PAPERS_DIR is exposed."""
    import arxiv_archive.cli as cli

    analysis_dir = tmp_path / "analysis"
    papers_dir = tmp_path / "papers"
    assert hasattr(cli, "PAPERS_DIR"), "S04 contract requires arxiv_archive.cli.PAPERS_DIR"
    monkeypatch.setattr(cli, "ANALYSIS_DIR", analysis_dir)
    monkeypatch.setattr(cli, "PAPERS_DIR", papers_dir)
    return analysis_dir, papers_dir


def test_s03_write_session_json_persists_full_daily_analysis_contract(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    import arxiv_archive.cli as cli

    sessions_dir = tmp_path / "sessions"
    monkeypatch.setattr(cli, "SESSIONS_DIR", sessions_dir)
    analysis = make_s03_done_analysis()

    path = cli.write_session_json(analysis)

    assert path == sessions_dir / "2026-05-14.json"
    payload = json.loads(path.read_text())
    assert payload["date"] == "2026-05-14"
    assert payload["status"] == "done"
    assert payload["analysis_timestamp"] == "2026-05-14T12:30:45"
    assert payload["papers_fetched"] == 2
    assert payload["paper_count"] == 2
    assert payload["top_paper_count"] == 2

    assert len(payload["papers"]) == 2
    first, second = payload["papers"]
    assert set(first) >= {
        "id",
        "title",
        "abstract",
        "authors",
        "published",
        "updated",
        "categories",
        "pdf_url",
        "keywords",
        "score",
        "breakdown",
        "semschol",
    }
    assert first["published"] == "2026-05-14"
    assert first["updated"] == "2026-05-14"
    assert isinstance(first["score"], float)
    assert isinstance(first["breakdown"]["fixture"], float)
    assert first["semschol"] == {
        "arxiv_id": "2605.00001",
        "title": "Paper 1",
        "citation_count": 42,
        "year": 2026,
        "venue": "FixtureConf",
    }
    assert second["semschol"] is None
    assert "pdfUrl" not in first


def test_s03_write_daily_artifacts_persists_papers_scored_and_overview(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    import arxiv_archive.cli as cli

    analysis_dir = tmp_path / "analysis"
    monkeypatch.setattr(cli, "ANALYSIS_DIR", analysis_dir)
    analysis = make_s03_done_analysis()

    output_dir = cli.write_daily_artifacts(analysis)

    day_dir = analysis_dir / "2026-05-14"
    assert output_dir == day_dir
    papers = json.loads((day_dir / "papers.json").read_text())
    scored = json.loads((day_dir / "scored.json").read_text())
    overview = json.loads((day_dir / "overview.json").read_text())

    assert [paper["id"] for paper in papers] == ["2605.00001", "2605.00002"]
    assert "score" not in papers[0]
    assert [paper["score"] for paper in scored] == [9.5, 7.25]
    assert scored[1]["semschol"] is None
    assert overview["date"] == "2026-05-14"
    assert overview["status"] == "done"
    assert overview["papers_fetched"] == 2
    assert overview["paper_count"] == 2
    assert overview["top_paper_count"] == 2
    assert isinstance(overview["categories"], list)
    assert isinstance(overview["keywords"], list)
    assert isinstance(overview["top_papers"], list)
    assert isinstance(overview["score_breakdown"], dict)


def test_s04_write_daily_artifacts_persists_per_paper_raw_and_scored_json(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    import arxiv_archive.cli as cli

    _analysis_dir, papers_dir = patch_s04_artifact_dirs(monkeypatch, tmp_path)
    analysis = make_s04_scored_analysis()

    cli.write_daily_artifacts(analysis)

    for scored in analysis.papers:
        paper_dir = papers_dir / scored.paper.id
        paper_payload = json.loads((paper_dir / "paper.json").read_text())
        scored_payload = json.loads((paper_dir / "scored.json").read_text())

        assert paper_payload["id"] == scored.paper.id
        assert paper_payload["paper_id"] == scored.paper.id
        assert paper_payload["title"] == scored.paper.title
        assert paper_payload["abstract"] == scored.paper.abstract
        assert paper_payload["authors"] == scored.paper.authors
        assert paper_payload["published"] == "2026-05-14"
        assert paper_payload["updated"] == "2026-05-14"
        assert paper_payload["categories"] == scored.paper.categories
        assert paper_payload["pdf_url"] == scored.paper.pdf_url
        assert "score" not in paper_payload
        assert "keywords" not in paper_payload
        assert "breakdown" not in paper_payload
        assert "semschol" not in paper_payload

        assert scored_payload["id"] == scored.paper.id
        assert scored_payload["score"] == scored.score
        assert scored_payload["keywords"] == scored.keywords
        assert scored_payload["breakdown"] == scored.breakdown
        assert "semschol" in scored_payload


def test_s04_write_daily_artifacts_populates_overview_aggregates(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    import arxiv_archive.cli as cli

    analysis_dir, _papers_dir = patch_s04_artifact_dirs(monkeypatch, tmp_path)
    analysis = make_s04_scored_analysis()

    cli.write_daily_artifacts(analysis)

    overview = json.loads((analysis_dir / "2026-05-14" / "overview.json").read_text())

    assert overview["date"] == "2026-05-14"
    assert overview["status"] == "done"
    assert overview["papers_fetched"] == 2
    assert overview["paper_count"] == 2
    assert overview["top_paper_count"] == 2
    assert overview["categories"] == [
        {"category": "cs.AI", "count": 2},
        {"category": "cs.CL", "count": 1},
        {"category": "cs.IR", "count": 1},
    ]
    assert overview["keywords"] == [
        {"keyword": "agent", "count": 2},
        {"keyword": "graph", "count": 1},
        {"keyword": "retrieval", "count": 1},
    ]
    assert [paper["id"] for paper in overview["top_papers"]] == ["2605.00001", "2605.00002"]
    assert overview["top_papers"][0]["score"] == 9.5
    assert overview["top_papers"][0]["breakdown"] == {"citations": 2.0, "preference": 4.0}
    assert overview["score_breakdown"] == {
        "citations": {"min": 2.0, "max": 6.0, "mean": 4.0},
        "preference": {"min": 4.0, "max": 8.0, "mean": 6.0},
    }


def test_s04_empty_day_overview_aggregates_are_empty_without_dividing_by_zero(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    import arxiv_archive.cli as cli

    analysis_dir, _papers_dir = patch_s04_artifact_dirs(monkeypatch, tmp_path)
    analysis = make_s03_empty_analysis()

    cli.write_daily_artifacts(analysis)

    overview = json.loads((analysis_dir / "2026-05-14" / "overview.json").read_text())
    assert overview["status"] == "empty"
    assert overview["papers_fetched"] == 0
    assert overview["paper_count"] == 0
    assert overview["top_paper_count"] == 0
    assert overview["categories"] == []
    assert overview["keywords"] == []
    assert overview["top_papers"] == []
    assert overview["score_breakdown"] == {}


def test_s03_empty_day_persistence_writes_same_json_files_with_empty_arrays(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    import arxiv_archive.cli as cli

    sessions_dir = tmp_path / "sessions"
    analysis_dir = tmp_path / "analysis"
    monkeypatch.setattr(cli, "SESSIONS_DIR", sessions_dir)
    monkeypatch.setattr(cli, "ANALYSIS_DIR", analysis_dir)
    analysis = make_s03_empty_analysis()

    session_path = cli.write_session_json(analysis)
    day_dir = cli.write_daily_artifacts(analysis)

    session_payload = json.loads(session_path.read_text())
    papers = json.loads((day_dir / "papers.json").read_text())
    scored = json.loads((day_dir / "scored.json").read_text())
    overview = json.loads((day_dir / "overview.json").read_text())

    assert session_payload["status"] == "empty"
    assert session_payload["papers_fetched"] == 0
    assert session_payload["papers"] == []
    assert session_payload["top_papers"] == []
    assert papers == []
    assert scored == []
    assert overview["status"] == "empty"
    assert overview["papers_fetched"] == 0
    assert overview["paper_count"] == 0
    assert overview["top_paper_count"] == 0
    assert overview["categories"] == []
    assert overview["keywords"] == []
    assert overview["top_papers"] == []
    assert overview["score_breakdown"] == {}


def test_s03_cli_json_run_invokes_json_writers_and_keeps_status_stdout(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    import arxiv_archive.cli as cli

    analysis = make_s03_done_analysis()
    calls: list[tuple[str, object]] = []

    monkeypatch.setattr(cli, "run_analysis", lambda run_date: analysis)
    monkeypatch.setattr(
        cli,
        "write_session_json",
        lambda result: calls.append(("session", result)) or Path("/tmp/session.json"),
    )
    monkeypatch.setattr(
        cli,
        "write_daily_artifacts",
        lambda result: calls.append(("artifacts", result)) or Path("/tmp/analysis/2026-05-14"),
    )

    cli.run("2026-05-14", json_output=True)

    output = capsys.readouterr().out
    assert calls == [("session", analysis), ("artifacts", analysis)]
    assert "status: done" in output
    assert "date: 2026-05-14" in output
    assert "papers fetched: 2" in output
    assert "top papers: 2" in output
