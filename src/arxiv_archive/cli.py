"""CLI for arxiv-archive."""

import asyncio
import json
import os
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import UTC, date, datetime
from pathlib import Path
from typing import Annotated, Any, Literal

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
from arxiv_archive.embedder import Embedder  # noqa: E402
from arxiv_archive.keyword_extractor import KeywordExtractor  # noqa: E402
from arxiv_archive.scoring import ScoredPaper, ScoringEngine  # noqa: E402
from arxiv_archive.validation_batch_state import (  # noqa: E402
    build_contract_response,
    read_batch_state,
)
from arxiv_archive.validation_batch_workflow import (  # noqa: E402
    initialize_validation_batch,
    preflight_validation_batch,
    run_validation_batch_scan,
    validation_batch_state_preview,
    write_source_preflight_run,
)

PREFERENCES_PATH = Path.home() / ".research" / "self" / "preferences.json"
SESSIONS_DIR = Path.home() / ".research" / "ops" / "sessions"
QUEUE_DIR = Path.home() / ".research" / "ops" / "queue"
ANALYSIS_DIR = Path.home() / ".research" / "analysis"
PAPERS_DIR = Path.home() / ".research" / "papers"

CATEGORIES = ["cs.AI", "cs.LG", "cs.CL", "cs.CV", "cs.IR", "cs.KG", "cs.SI"]

DailyAnalysisStatus = Literal["done", "empty"]
QueueStateStatus = Literal["running", "done", "empty", "failed"]


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
`uv run python -m arxiv_archive --date YYYY-MM-DD`. Hermes should inspect the
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
  uv run python -m arxiv_archive --date YYYY-MM-DD
  uv run python -m arxiv_archive --date YYYY-MM-DD --json

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

validation_batch_app = typer.Typer(
    add_completion=False,
    context_settings={"help_option_names": ["-h", "--help"], "max_content_width": 120},
    help="Contract-only validation batch commands for M007.",
    no_args_is_help=True,
)
app.add_typer(validation_batch_app, name="validation-batch")


def _echo_validation_batch_response(response: dict[str, Any], *, as_json: bool) -> None:
    if as_json:
        typer.echo(json.dumps(response, indent=2, sort_keys=True))
        return
    typer.echo(
        " | ".join(
            [
                f"status: {response['status']}",
                f"command: {response.get('command', 'validation-batch')}",
                str(response.get("boundary", "No production KG import; validation-batch commands are operational diagnostics only.")),
            ]
        )
    )


@validation_batch_app.command("contract")
def validation_batch_contract(
    json_output: Annotated[
        bool,
        typer.Option(
            "--json",
            help="Print the validation-batch contract response as JSON.",
        ),
    ] = False,
) -> None:
    """Print the M007 validation-batch contract response without doing work."""
    _echo_validation_batch_response(
        build_contract_response("validation-batch contract", status="contract_only"),
        as_json=json_output,
    )


def _validation_batch_stub(command: str, *, batch_id: str, as_json: bool) -> None:
    response = build_contract_response(f"validation-batch {command}", status="not_implemented")
    response["batch_id"] = batch_id
    _echo_validation_batch_response(response, as_json=as_json)
    raise typer.Exit(1)


@validation_batch_app.command("init")
def validation_batch_init(
    batch_id: Annotated[str, typer.Option("--batch-id", help="Validation batch identifier.")],
    manifest_path: Annotated[
        Path,
        typer.Option("--manifest-path", help="Input validation manifest JSON."),
    ],
    output_dir: Annotated[
        Path,
        typer.Option("--output-dir", help="Directory where batch artifacts will be written."),
    ],
    limit: Annotated[int | None, typer.Option("--limit", help="Optional maximum papers to select.")] = None,
    json_output: Annotated[bool, typer.Option("--json", help="Print JSON response.")] = False,
) -> None:
    """Initialize a validation batch state and selection manifest."""
    result = initialize_validation_batch(
        manifest_path=manifest_path,
        batch_id=batch_id,
        output_dir=output_dir,
        limit=limit,
    )
    response = validation_batch_state_preview(result["state"])
    response.update(
        {
            "status": "initialized",
            "state_path": str(result["state_path"]),
            "selection_manifest_path": str(result["selection_manifest_path"]),
            "real_source_acquisition_performed": False,
            "real_scan_performed": False,
        }
    )
    _echo_validation_batch_response(response, as_json=json_output)


@validation_batch_app.command("preflight")
def validation_batch_preflight(
    state_path: Annotated[
        Path,
        typer.Option("--state-path", help="Existing validation batch-state.json path."),
    ],
    output_dir: Annotated[
        Path | None,
        typer.Option("--output-dir", help="Directory where preflight artifacts will be written."),
    ] = None,
    json_output: Annotated[bool, typer.Option("--json", help="Print JSON response.")] = False,
) -> None:
    """Run source preflight over an initialized validation batch state."""
    state = read_batch_state(state_path)
    preflighted = preflight_validation_batch(state)
    target_dir = output_dir if output_dir is not None else state_path.parent
    paths = write_source_preflight_run(preflighted, target_dir)
    response = validation_batch_state_preview(preflighted)
    response.update(
        {
            "status": "preflighted",
            "state_path": str(paths["state_path"]),
            "summary_path": str(paths["summary_path"]),
            "diagnostics_path": str(paths["diagnostics_path"]),
            "real_source_acquisition_performed": False,
            "real_scan_performed": False,
        }
    )
    _echo_validation_batch_response(response, as_json=json_output)


@validation_batch_app.command("scan")
def validation_batch_scan(
    state_path: Annotated[
        Path,
        typer.Option("--state-path", help="Source-ready validation batch-state.json path."),
    ],
    output_dir: Annotated[
        Path,
        typer.Option("--output-dir", help="Directory where scan artifacts will be written."),
    ],
    structure_baseline_path: Annotated[
        Path | None,
        typer.Option("--structure-baseline-path", help="M005/S03 structure-aware baseline JSON."),
    ] = None,
    mixed_benchmark_path: Annotated[
        Path | None,
        typer.Option("--mixed-benchmark-path", help="M005/S06 mixed benchmark JSON for context only."),
    ] = None,
    json_output: Annotated[bool, typer.Option("--json", help="Print JSON response.")] = False,
) -> None:
    """Run a redacted validation-batch scan over a source-ready batch."""
    state = read_batch_state(state_path)
    result = run_validation_batch_scan(
        state,
        output_dir,
        structure_baseline_path=structure_baseline_path,
        mixed_benchmark_path=mixed_benchmark_path,
    )
    response = validation_batch_state_preview(result["state"])
    response.update(
        {
            "status": "scanned",
            "state_path": str(result["state_path"]),
            "summary_path": str(result["summary_path"]),
            "diagnostics_path": str(result["diagnostics_path"]),
            "delta_report_path": str(result["delta_report_path"]),
            "outlier_report_path": str(result["outlier_report_path"]),
            "real_source_acquisition_performed": False,
            "real_scan_performed": True,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        }
    )
    _echo_validation_batch_response(response, as_json=json_output)


@validation_batch_app.command("review")
def validation_batch_review(
    batch_id: Annotated[str, typer.Option("--batch-id", help="Validation batch identifier.")],
    json_output: Annotated[bool, typer.Option("--json", help="Print JSON response.")] = False,
) -> None:
    """Contract-only stub for future review handoff."""
    _validation_batch_stub("review", batch_id=batch_id, as_json=json_output)


@validation_batch_app.command("resume")
def validation_batch_resume(
    batch_id: Annotated[str, typer.Option("--batch-id", help="Validation batch identifier.")],
    json_output: Annotated[bool, typer.Option("--json", help="Print JSON response.")] = False,
) -> None:
    """Contract-only stub for future resumable batch execution."""
    _validation_batch_stub("resume", batch_id=batch_id, as_json=json_output)


@app.callback(invoke_without_command=True)
def root_callback(
    ctx: typer.Context,
    run_date: Annotated[
        str | None,
        typer.Option(
            "--date",
            help="Run date in YYYY-MM-DD format.",
        ),
    ] = None,
    json_output: Annotated[
        bool,
        typer.Option(
            "--json",
            help="Write Hermes-readable session JSON and daily analysis artifacts after analysis succeeds.",
        ),
    ] = False,
) -> None:
    """Preserve the legacy root --date entrypoint while supporting subcommands."""
    if ctx.invoked_subcommand is not None:
        return
    if run_date is None:
        typer.echo(ctx.get_help())
        raise typer.Exit()
    run(run_date=run_date, json_output=json_output)


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


def _serialize_date(value: date) -> str:
    """Serialize a date using the Rust-portable YYYY-MM-DD form."""
    return value.isoformat()


def _serialize_analysis_timestamp(value: datetime) -> str:
    """Serialize analysis timestamps without Python-specific datetime objects.

    UTC-aware datetimes are normalized to a compact `YYYY-MM-DDTHH:MM:SSZ`
    value. Other datetimes preserve their ISO timezone information when present;
    naive fixtures remain naive ISO strings for backwards-compatible tests.
    """
    if value.tzinfo is not None and value.utcoffset() is not None:
        utc_value = value.astimezone(UTC).replace(microsecond=0)
        return utc_value.isoformat().replace("+00:00", "Z")
    return value.replace(microsecond=0).isoformat()


def write_state_json(
    run_date: date,
    status: QueueStateStatus,
    stage: str,
    error: str | None = None,
) -> Path:
    """Write the cron/Hermes queue state file for one daily CLI run."""
    QUEUE_DIR.mkdir(parents=True, exist_ok=True)
    filepath = QUEUE_DIR / f"{run_date.isoformat()}.json"
    payload = {
        "date": run_date.isoformat(),
        "status": status,
        "stage": stage,
        "timestamp": _serialize_analysis_timestamp(datetime.now(UTC)),
    }
    if error:
        payload["error"] = error
    filepath.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    return filepath


def _serialize_paper(paper: Any) -> dict[str, Any]:
    """Serialize an ArxivPaper to JSON-native, portable values."""
    return {
        # S03 tests consume `id`; `paper_id` is included as the explicit public
        # name for consumers that should not rely on the Python dataclass field.
        "id": paper.id,
        "paper_id": paper.id,
        "title": paper.title,
        "abstract": paper.abstract,
        "authors": list(paper.authors),
        "published": _serialize_date(paper.published),
        "updated": _serialize_date(paper.updated),
        "categories": list(paper.categories),
        "pdf_url": paper.pdf_url,
    }


def _serialize_semantic_scholar(semschol: Any | None) -> dict[str, Any] | None:
    """Serialize optional Semantic Scholar enrichment without repr/asdict."""
    if semschol is None:
        return None
    return {
        "arxiv_id": semschol.arxiv_id,
        "title": semschol.title,
        "citation_count": semschol.citation_count,
        "year": semschol.year,
        "venue": semschol.venue,
    }


def _serialize_scored_paper(scored: ScoredPaper) -> dict[str, Any]:
    """Serialize a ScoredPaper with JSON-native scoring metadata."""
    payload = _serialize_paper(scored.paper)
    payload.update(
        {
            "keywords": list(scored.keywords),
            "score": float(scored.score),
            "breakdown": {key: float(value) for key, value in scored.breakdown.items()},
            "semschol": _serialize_semantic_scholar(scored.semschol),
        }
    )
    return payload


def write_session_json(analysis: DailyAnalysis) -> Path:
    """Write the Hermes-readable JSON session summary for a daily analysis."""
    SESSIONS_DIR.mkdir(parents=True, exist_ok=True)
    filepath = SESSIONS_DIR / f"{analysis.run_date.isoformat()}.json"
    payload = {
        "date": analysis.run_date.isoformat(),
        "status": analysis.status,
        "analysis_timestamp": _serialize_analysis_timestamp(analysis.analysis_timestamp),
        "papers_fetched": analysis.papers_fetched,
        "paper_count": len(analysis.papers),
        "top_paper_count": len(analysis.top_papers),
        "papers_count": len(analysis.papers),
        "top_papers_count": len(analysis.top_papers),
        "papers": [_serialize_scored_paper(scored) for scored in analysis.papers],
        "top_papers": [_serialize_scored_paper(scored) for scored in analysis.top_papers],
    }
    filepath.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    return filepath


def write_paper_artifacts(scored: ScoredPaper) -> Path:
    """Write reusable per-paper raw and scored JSON artifacts."""
    paper_dir = PAPERS_DIR / scored.paper.id
    paper_dir.mkdir(parents=True, exist_ok=True)
    (paper_dir / "paper.json").write_text(
        json.dumps(_serialize_paper(scored.paper), indent=2, sort_keys=True) + "\n"
    )
    (paper_dir / "scored.json").write_text(
        json.dumps(_serialize_scored_paper(scored), indent=2, sort_keys=True) + "\n"
    )
    return paper_dir


def _count_payloads(counter: Counter[str], name_key: str, limit: int) -> list[dict[str, Any]]:
    """Return deterministic top-N count payloads sorted by count then name."""
    return [
        {name_key: name, "count": count}
        for name, count in sorted(counter.items(), key=lambda item: (-item[1], item[0]))[:limit]
    ]


def build_overview_payload(analysis: DailyAnalysis) -> dict[str, Any]:
    """Build the populated daily overview artifact for inspection and calibration."""
    category_counts: Counter[str] = Counter()
    keyword_counts: Counter[str] = Counter()
    breakdown_values: dict[str, list[float]] = defaultdict(list)

    for scored in analysis.papers:
        category_counts.update(scored.paper.categories)
        keyword_counts.update(scored.keywords)
        for component, value in scored.breakdown.items():
            breakdown_values[component].append(float(value))

    score_breakdown = {
        component: {
            "min": min(values),
            "max": max(values),
            "mean": sum(values) / len(values),
        }
        for component, values in sorted(breakdown_values.items())
    }

    return {
        "date": analysis.run_date.isoformat(),
        "status": analysis.status,
        "papers_fetched": analysis.papers_fetched,
        "paper_count": len(analysis.papers),
        "top_paper_count": len(analysis.top_papers),
        "categories": _count_payloads(category_counts, "category", 20),
        "keywords": _count_payloads(keyword_counts, "keyword", 30),
        "top_papers": [
            _serialize_scored_paper(scored) for scored in analysis.top_papers[:10]
        ],
        "score_breakdown": score_breakdown,
    }


def write_daily_artifacts(analysis: DailyAnalysis) -> Path:
    """Write daily analysis artifacts for local tools and S04 aggregation."""
    day_dir = ANALYSIS_DIR / analysis.run_date.isoformat()
    day_dir.mkdir(parents=True, exist_ok=True)

    papers_payload = [_serialize_paper(scored.paper) for scored in analysis.papers]
    scored_payload = [_serialize_scored_paper(scored) for scored in analysis.papers]
    overview_payload = build_overview_payload(analysis)

    for scored in analysis.papers:
        write_paper_artifacts(scored)

    (day_dir / "papers.json").write_text(json.dumps(papers_payload, indent=2, sort_keys=True) + "\n")
    (day_dir / "scored.json").write_text(json.dumps(scored_payload, indent=2, sort_keys=True) + "\n")
    (day_dir / "overview.json").write_text(json.dumps(overview_payload, indent=2, sort_keys=True) + "\n")
    return day_dir


async def _process_paper_async(paper, extractor, scorer):
    # Offload CPU-bound extraction and scoring to threadpool
    loop = asyncio.get_running_loop()
    def _extract_and_score():
        if not isinstance(paper.title, str) or not isinstance(paper.abstract, str):
            raise TypeError("fetched paper title and abstract must be strings")
        keywords = extractor.extract_for_paper(paper.title, paper.abstract)
        return scorer.score(paper, semschol=None, keywords=keywords)
    return await loop.run_in_executor(None, _extract_and_score)

def run_analysis(run_date: date) -> DailyAnalysis:
    """Build the normalized in-memory analysis result for one run date."""
    _preferences = load_preferences()

    client = ArxivClient()
    extractor = KeywordExtractor()
    scorer = ScoringEngine()

    papers = client.fetch_papers(
        start_date=run_date,
        end_date=run_date,
        categories=CATEGORIES,
    )

    if not papers:
        from datetime import datetime
        return DailyAnalysis(
            run_date=run_date,
            status="empty",
            analysis_timestamp=datetime.now(UTC),
            papers_fetched=0,
            papers=[],
            top_papers=[]
        )

    # 1. Run extraction and scoring concurrently using asyncio
    async def _process_all():
        tasks = [_process_paper_async(p, extractor, scorer) for p in papers]
        scored_list = await asyncio.gather(*tasks)

        # 2. Embed all abstracts via the Embedder batch API
        embedder = Embedder()
        try:
            abstracts = [p.paper.abstract for p in scored_list]
            embeddings = await embedder.embed_all(abstracts)

            # Attach embeddings back to scored papers
            for scored, emb in zip(scored_list, embeddings, strict=True):
                scored.embedding = emb
        finally:
            await embedder.close()

        return scored_list

    try:
        loop = asyncio.get_running_loop()
        scored_papers = loop.run_until_complete(_process_all())
    except RuntimeError:
        scored_papers = asyncio.run(_process_all())

    scored_papers.sort(key=lambda x: x.score, reverse=True)
    top_papers = scored_papers[:10]

    from datetime import datetime
    return DailyAnalysis(
        run_date=run_date,
        status="done",
        analysis_timestamp=datetime.now(UTC),
        papers_fetched=len(papers),
        papers=scored_papers,
        top_papers=top_papers,
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
                "Write Hermes-readable session JSON and daily analysis artifacts "
                "after analysis succeeds."
            ),
        ),
    ] = False,
) -> None:
    """Run the daily arXiv archive pipeline for one YYYY-MM-DD date."""
    try:
        parsed_date = date.fromisoformat(run_date)
    except ValueError as exc:
        raise typer.BadParameter("date must be in YYYY-MM-DD format") from exc

    write_state_json(parsed_date, "running", "fetch")
    try:
        analysis = run_analysis(parsed_date)
    except Exception as exc:
        write_state_json(parsed_date, "failed", "failed", str(exc))
        raise

    write_state_json(parsed_date, analysis.status, "done")
    if json_output:
        write_session_json(analysis)
        write_daily_artifacts(analysis)
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
