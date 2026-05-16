"""CLI for arxiv-archive."""

import json
import os
from dataclasses import dataclass
from datetime import UTC, date, datetime
from pathlib import Path
from typing import Annotated, Literal

import typer

# Load .env before any module that might need API keys
_env_path = Path(__file__).parent.parent.parent / ".env"
if _env_path.exists():
    for line in _env_path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" in line:
            key, _, value = line.partition("=")
            os.environ.setdefault(key.strip(), value.strip())

from arxiv_archive.arxiv_client import ArxivClient  # noqa: E402
from arxiv_archive.keyword_extractor import KeywordExtractor  # noqa: E402
from arxiv_archive.scoring import ScoredPaper, ScoringEngine  # noqa: E402

PREFERENCES_PATH = Path.home() / ".research" / "self" / "preferences.json"
SESSIONS_DIR = Path.home() / ".research" / "ops" / "sessions"

CATEGORIES = ["cs.AI", "cs.LG", "cs.CL", "cs.CV", "cs.IR", "cs.KG", "cs.SI"]

DailyAnalysisStatus = Literal["done", "empty"]


@dataclass(frozen=True)
class DailyAnalysis:
    """Normalized in-memory analysis result for one arXiv archive day."""

    run_date: date
    status: DailyAnalysisStatus
    papers_fetched: int
    papers: list[ScoredPaper]
    top_papers: list[ScoredPaper]
    analysis_timestamp: datetime


AGENT_CONTRACT_HELP = """Daily arXiv archive CLI for research agents.

Purpose: fetch a day's arXiv papers, analyze/score them against research interests,
and archive the selected papers for later review.

Hermes / cron usage: invoke the stable public entrypoint with
`uv run python -m arxiv_archive run --date YYYY-MM-DD`. Hermes should inspect the
same stdout/stderr and exit codes that cron sees.

Artifacts documented for the M001 contract:
- ~/research/ops/sessions/YYYY-MM-DD.json: future Hermes session/state summary.
- ~/research/analysis/YYYY-MM-DD/overview.json: future analysis overview artifact.
- ~/research/papers/: future downloaded paper archive.
Current implementation still writes the legacy markdown session under
~/.research/ops/sessions until later storage slices replace it.

Status/state meanings for future state files: running means work is in progress,
done means papers were archived, empty means no matching papers were found, and
failed means the run stopped before producing a complete archive.

Exit codes: 0 success/help, 1 runtime failure, 2 command-line usage or validation error.

Examples:
  uv run python -m arxiv_archive --help
  uv run python -m arxiv_archive run --help
  uv run python -m arxiv_archive run --date YYYY-MM-DD
  uv run python -m arxiv_archive run --date YYYY-MM-DD --json

Out of scope / non-goals for M001: Telegram delivery, Graphify integration,
Surprise Me ranking, preference learning, PDF conversion/download behavior, and LLM
summarization changes. Those belong to later slices.
"""

app = typer.Typer(
    add_completion=False,
    context_settings={"help_option_names": ["-h", "--help"], "max_content_width": 120},
    help=AGENT_CONTRACT_HELP,
    no_args_is_help=True,
)


def load_preferences() -> dict:
    """Load preferences from ~/.research/self/preferences.json.

    Returns:
        Dictionary of preferences. Empty dict if file doesn't exist.
    """
    if not PREFERENCES_PATH.exists():
        return {}
    with open(PREFERENCES_PATH) as f:
        return json.load(f)


def save_session(
    run_date: date, papers_fetched: int, top10: list[ScoredPaper]
) -> Path:
    """Save session summary to ~/.research/ops/sessions/{date}.md.

    Args:
        run_date: The date of the run.
        papers_fetched: Number of papers fetched.
        top10: List of top 10 scored papers.

    Returns:
        Path to the saved session file.
    """
    SESSIONS_DIR.mkdir(parents=True, exist_ok=True)
    filepath = SESSIONS_DIR / f"{run_date.isoformat()}.md"

    lines = [
        f"# Arxiv Archive Session — {run_date.isoformat()}",
        "",
        f"**Papers fetched:** {papers_fetched}",
        f"**Top papers selected:** {len(top10)}",
        "",
        "## Top 10 Papers",
        "",
    ]

    for i, scored in enumerate(top10, 1):
        paper = scored.paper
        lines.append(f"### {i}. {paper.title}")
        lines.append("")
        lines.append(f"**arXiv ID:** {paper.id}")
        lines.append(f"**Authors:** {', '.join(paper.authors)}")
        lines.append(f"**Categories:** {', '.join(paper.categories)}")
        lines.append(f"**Published:** {paper.published.isoformat()}")
        lines.append(f"**Score:** {scored.score:.3f}")
        lines.append(f"**Keywords:** {', '.join(scored.keywords[:10])}")
        lines.append("")
        lines.append(f"**Abstract:** {paper.abstract[:500]}...")
        lines.append("")

    with open(filepath, "w") as f:
        f.write("\n".join(lines))

    return filepath


def run_analysis(run_date: date) -> DailyAnalysis:
    """Build the normalized in-memory analysis result for one run date.

    This boundary performs fetch, keyword extraction, scoring, and sorting only.
    It intentionally does not persist session or JSON artifacts; storage belongs
    to later slices.

    Args:
        run_date: The date to fetch papers for.

    Returns:
        Normalized daily analysis with all scored papers and the top 10 subset.
    """
    # Load preferences to preserve the existing pipeline boundary for future
    # scoring use, even though the current ScoringEngine does not consume them.
    _preferences = load_preferences()

    client = ArxivClient()
    extractor = KeywordExtractor()
    scorer = ScoringEngine()

    papers = client.fetch_papers(
        start_date=run_date,
        end_date=run_date,
        categories=CATEGORIES,
    )

    scored_papers: list[ScoredPaper] = []
    for paper in papers:
        if not isinstance(paper.title, str) or not isinstance(paper.abstract, str):
            raise TypeError("fetched paper title and abstract must be strings")
        keywords = extractor.extract_for_paper(paper.title, paper.abstract)
        scored = scorer.score(paper, semschol=None, keywords=keywords)
        scored_papers.append(scored)

    scored_papers.sort(key=lambda x: x.score, reverse=True)
    top_papers = scored_papers[:10]
    status: DailyAnalysisStatus = "done" if papers else "empty"

    return DailyAnalysis(
        run_date=run_date,
        status=status,
        papers_fetched=len(papers),
        papers=scored_papers,
        top_papers=top_papers,
        analysis_timestamp=datetime.now(UTC),
    )


def run_pipeline(run_date: date) -> None:
    """Run the legacy arxiv archive pipeline for a given date and save a session.

    This compatibility wrapper preserves the pre-S02 persistence behavior for any
    direct callers while sharing the normalized analysis boundary.

    Args:
        run_date: The date to fetch papers for.
    """
    analysis = run_analysis(run_date)

    # Save session
    session_path = save_session(
        analysis.run_date,
        analysis.papers_fetched,
        analysis.top_papers,
    )

    # Print legacy summary
    print(  # noqa: T201
        f"Fetched {analysis.papers_fetched} papers, selected top {len(analysis.top_papers)}"
    )
    print(f"Session saved to {session_path}")  # noqa: T201


@app.command(help=AGENT_CONTRACT_HELP)
def run(
    run_date: Annotated[
        str,
        typer.Option(
            "--date",
            help="Run date in YYYY-MM-DD format.",
        ),
    ],
    json_output: Annotated[
        bool,
        typer.Option(
            "--json",
            help=(
                "Documented Hermes option for future machine-readable output; "
                "currently runs the legacy pipeline without JSON persistence."
            ),
        ),
    ] = False,
) -> None:
    """Run the daily arXiv archive pipeline for one YYYY-MM-DD date."""
    try:
        parsed_date = date.fromisoformat(run_date)
    except ValueError as exc:
        raise typer.BadParameter("date must be in YYYY-MM-DD format") from exc

    if json_output:
        typer.echo(
            "--json is documented for Hermes but JSON persistence is not implemented in M001.",
            err=True,
        )
    analysis = run_analysis(parsed_date)
    typer.echo(
        " | ".join(
            [
                f"status: {analysis.status}",
                f"date: {analysis.run_date.isoformat()}",
                f"papers fetched: {analysis.papers_fetched}",
                f"top papers: {len(analysis.top_papers)}",
            ]
        )
    )


def main() -> None:
    """Main CLI entry point."""
    app()
