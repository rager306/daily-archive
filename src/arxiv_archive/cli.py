"""CLI for arxiv-archive."""

import argparse
import json
import os
from datetime import date
from pathlib import Path

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


def run_pipeline(run_date: date) -> None:
    """Run the arxiv archive pipeline for a given date.

    Fetches papers, extracts keywords, scores them, and saves the session.

    Args:
        run_date: The date to fetch papers for.
    """
    # Load preferences (for future use in scoring)
    _preferences = load_preferences()

    # Create components
    client = ArxivClient()
    extractor = KeywordExtractor()
    scorer = ScoringEngine()

    # Fetch papers
    papers = client.fetch_papers(
        start_date=run_date,
        end_date=run_date,
        categories=CATEGORIES,
    )

    # Extract keywords and score
    scored_papers = []
    for paper in papers:
        keywords = extractor.extract_for_paper(paper.title, paper.abstract)
        scored = scorer.score(paper, semschol=None, keywords=keywords)
        scored_papers.append(scored)

    # Sort by score descending and take top 10
    scored_papers.sort(key=lambda x: x.score, reverse=True)
    top10 = scored_papers[:10]

    # Save session
    session_path = save_session(run_date, len(papers), top10)

    # Print summary
    print(f"Fetched {len(papers)} papers, selected top {len(top10)}")  # noqa: T201
    print(f"Session saved to {session_path}")  # noqa: T201


def main() -> None:
    """Main CLI entry point."""
    parser = argparse.ArgumentParser(prog="arxiv-archive", description="Arxiv Archive CLI")
    parser.add_argument(
        "command",
        nargs="?",
        default="run",
        help="Command to run (default: run)",
    )
    parser.add_argument(
        "--date",
        type=date.fromisoformat,
        required=True,
        help="Run date in YYYY-MM-DD format",
    )

    args = parser.parse_args()

    if args.command == "run":
        run_pipeline(args.date)
    else:
        parser.print_help()
