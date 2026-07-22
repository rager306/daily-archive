# Formerly: src/arxiv_archive/cli.py

"""CLI for arxiv-archive."""

import asyncio
import hashlib
import json
import os
import tempfile
from collections import Counter, defaultdict
from datetime import UTC, date, datetime
from pathlib import Path
from typing import Annotated, Any, Literal

import typer

CLI_ENV_PATH = Path(__file__).parent.parent.parent / ".env"


def apply_cli_env_config(path: str | Path | None = None) -> None:
    """Apply dotenv values explicitly at CLI/process boundaries.

    Importing :mod:`research_graph.cli` must remain library-safe for async hosts,
    tests, and worker processes. Process entrypoints can call this helper to keep
    the previous CLI behavior without mutating ``os.environ`` at import time.
    """
    env_path = Path(path) if path is not None else CLI_ENV_PATH
    if not env_path.exists():
        return
    for line in env_path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" in line:
            key, _, value = line.partition("=")
            os.environ.setdefault(key.strip(), value.strip())


from research_graph.application.analysis import DailyAnalysis, DailyAnalysisStatus  # noqa: E402
from research_graph.application.analyze_day import (  # noqa: E402
    AnalyzeDayError,
    AnalyzeDayUseCase,
)
from research_graph.infrastructure.corpus.sources.arxiv_client import (  # noqa: E402
    ArxivClient,
    ArxivFetchError,
)
from research_graph.infrastructure.evaluation.scoring import (  # noqa: E402  # noqa: F401
    ScoredPaper,
    ScoringEngine,
)
from research_graph.infrastructure.papers.artifacts.batch_validation import (  # noqa: F401
    run_article_batch_validation_report,  # noqa: E402
)
from research_graph.infrastructure.papers.artifacts.minimax_boundary import (  # noqa: E402  # noqa: F401
    MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
    build_article_artifact_minimax_request,
    validate_article_artifact_minimax_response,
)
from research_graph.infrastructure.papers.artifacts.models import (  # noqa: E402  # noqa: F401
    ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION,
    ARTICLE_ARTIFACT_SCHEMA_VERSION,
    ArticleArtifactRunSummary,
    build_article_artifact_diagnostics_summary,
    build_article_artifact_manifest_from_structure,
    build_article_artifact_run_diagnostics_artifact,
    default_safety_flags,
    summarize_article_artifacts,
    to_json,
    validate_article_artifact_manifest,
)
from research_graph.infrastructure.quality import (  # noqa: E402  # noqa: F401
    build_maintainability_report,
    maintainability_report_to_json,
    write_maintainability_report,
)
from research_graph.infrastructure.retrieval.embedder import (  # noqa: E402
    Embedder,
    FdAuthError,
    FdDegradedEmbeddingsError,
)
from research_graph.infrastructure.retrieval.keyword_extractor import (
    KeywordExtractor,  # noqa: E402  # noqa: F401
)
from research_graph.workflows.validation.batch_provenance import (  # noqa: E402  # noqa: F401
    build_artifact_freshness_report,
    read_validation_cli_provenance_log,
    select_provenance_entry,
    write_artifact_freshness_report,
)
from research_graph.workflows.validation.batch_state import (  # noqa: E402  # noqa: F401
    build_contract_response,
    read_batch_state,
)
from research_graph.workflows.validation.batch_workflow import (  # noqa: E402  # noqa: F401
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
ANALYSIS_SCORE_CONCURRENCY = 8

QueueStateStatus = Literal["running", "done", "empty", "failed"]


AGENT_CONTRACT_HELP = """Daily arXiv archive CLI for research agents.

Purpose: fetch a day's arXiv papers, analyze/score them against research interests,
and archive the selected papers for later review.

Hermes / cron usage: invoke the stable public entrypoint with
`uv run python -m research_graph --date YYYY-MM-DD`. Hermes should inspect the
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
  uv run python -m research_graph --help
  uv run python -m research_graph run --help
  uv run python -m research_graph --date YYYY-MM-DD
  uv run python -m research_graph --date YYYY-MM-DD --json

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


# Register subcommand modules
from research_graph.cli.commands import article_artifacts, quality, validation_batch  # noqa: F401

article_artifacts.register(app)
validation_batch.register(app)
quality.register(app)


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


def save_session(run_date: date, papers_fetched: int, top10: list[ScoredPaper]) -> Path:
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


def _write_text_atomic(path: Path, text: str) -> None:
    """Write a small text artifact via same-directory atomic replace."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    try:
        temp_path.write_text(text, encoding="utf-8")
        temp_path.replace(path)
    finally:
        if temp_path.exists():
            temp_path.unlink()


def write_state_json(
    run_date: date,
    status: QueueStateStatus,
    stage: str,
    error: str | None = None,
) -> Path:
    """Write the cron/Hermes queue state file for one daily CLI run."""
    filepath = QUEUE_DIR / f"{run_date.isoformat()}.json"
    payload = {
        "date": run_date.isoformat(),
        "status": status,
        "stage": stage,
        "timestamp": _serialize_analysis_timestamp(datetime.now(UTC)),
    }
    if error:
        payload["error"] = error
    _write_text_atomic(filepath, json.dumps(payload, indent=2, sort_keys=True) + "\n")
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


def _atomic_write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            encoding="utf-8",
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as temp_file:
            temp_file.write(content)
            temp_name = temp_file.name
        Path(temp_name).replace(path)
    except Exception:
        if temp_name is not None:
            Path(temp_name).unlink(missing_ok=True)
        raise


def write_paper_artifacts(scored: ScoredPaper) -> Path:
    """Write reusable per-paper raw and scored JSON artifacts."""
    paper_dir = PAPERS_DIR / scored.paper.id
    paper_dir.mkdir(parents=True, exist_ok=True)
    _atomic_write_text(
        paper_dir / "paper.json",
        json.dumps(_serialize_paper(scored.paper), indent=2, sort_keys=True) + "\n",
    )
    _atomic_write_text(
        paper_dir / "scored.json",
        json.dumps(_serialize_scored_paper(scored), indent=2, sort_keys=True) + "\n",
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
        "top_papers": [_serialize_scored_paper(scored) for scored in analysis.top_papers[:10]],
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

    (day_dir / "papers.json").write_text(
        json.dumps(papers_payload, indent=2, sort_keys=True) + "\n"
    )
    (day_dir / "scored.json").write_text(
        json.dumps(scored_payload, indent=2, sort_keys=True) + "\n"
    )
    (day_dir / "overview.json").write_text(
        json.dumps(overview_payload, indent=2, sort_keys=True) + "\n"
    )
    return day_dir


async def run_analysis_async(run_date: date) -> DailyAnalysis:
    """CLI adapter: delegate day analysis to application-owned AnalyzeDayUseCase.

    M001 contracts (DailyAnalysis shape, state.json on typed I/O failure) stay
    at this boundary. Composition of fetch/score/embed lives in application.
    """
    _preferences = load_preferences()

    use_case = AnalyzeDayUseCase(
        paper_fetch=ArxivClient(),
        keyword_extractor=KeywordExtractor(),
        scorer=ScoringEngine(),
        embedder_factory=Embedder,
        categories=CATEGORIES,
        score_concurrency=ANALYSIS_SCORE_CONCURRENCY,
    )
    try:
        return await use_case.run(run_date)
    except ArxivFetchError as exc:
        write_state_json(run_date, "failed", "fetch", exc.diagnostic)
        raise
    except FdAuthError as exc:
        write_state_json(run_date, "failed", "embed", exc.diagnostic)
        raise
    except FdDegradedEmbeddingsError as exc:
        write_state_json(run_date, "failed", "embed", exc.diagnostic)
        raise
    except AnalyzeDayError as exc:
        write_state_json(run_date, "failed", exc.stage, exc.diagnostic)
        raise


def run_analysis(run_date: date) -> DailyAnalysis:
    """Synchronous compatibility wrapper for daily analysis."""
    try:
        asyncio.get_running_loop()
    except RuntimeError:
        apply_cli_env_config()
        return asyncio.run(run_analysis_async(run_date))
    raise RuntimeError(
        "run_analysis() cannot run inside an active event loop; await run_analysis_async() instead"
    )


async def run_pipeline_async(run_date: date) -> None:
    """Run the legacy pipeline using the async analysis entrypoint."""
    analysis = await run_analysis_async(run_date)

    session_path = save_session(
        analysis.run_date,
        analysis.papers_fetched,
        analysis.top_papers,
    )

    print(  # noqa: T201
        f"Fetched {analysis.papers_fetched} papers, selected top {len(analysis.top_papers)}"
    )
    print(f"Session saved to {session_path}")  # noqa: T201


def run_pipeline(run_date: date) -> None:
    """Synchronous compatibility wrapper for the legacy pipeline."""
    try:
        asyncio.get_running_loop()
    except RuntimeError:
        apply_cli_env_config()
        asyncio.run(run_pipeline_async(run_date))
        return
    raise RuntimeError(
        "run_pipeline() cannot run inside an active event loop; await run_pipeline_async() instead"
    )


async def run_command_async(parsed_date: date, *, json_output: bool = False) -> DailyAnalysis:
    """Run command orchestration around the async analysis entrypoint."""
    write_state_json(parsed_date, "running", "fetch")
    try:
        analysis = await run_analysis_async(parsed_date)
    except ArxivFetchError as exc:
        # Prefer the typed diagnostic; run_analysis_async may already have written
        # stage=fetch, but re-write failed/failed for the outer orchestrator contract.
        write_state_json(parsed_date, "failed", "failed", exc.diagnostic)
        raise
    except (FdAuthError, FdDegradedEmbeddingsError) as exc:
        write_state_json(parsed_date, "failed", "failed", exc.diagnostic)
        raise
    except AnalyzeDayError as exc:
        write_state_json(parsed_date, "failed", "failed", exc.diagnostic)
        raise
    except Exception as exc:
        write_state_json(parsed_date, "failed", "failed", str(exc))
        raise

    write_state_json(parsed_date, analysis.status, "done")
    if json_output:
        write_session_json(analysis)
        write_daily_artifacts(analysis)
    return analysis


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

    apply_cli_env_config()
    analysis = asyncio.run(run_command_async(parsed_date, json_output=json_output))
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
