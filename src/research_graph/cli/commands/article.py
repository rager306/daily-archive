"""Single-article CLI: acquire + no-write readiness pipeline."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Annotated, Literal

import typer

from research_graph.workflows.composition.single_article_pipeline import (
    SingleArticleRunRequest,
    run_single_article_pipeline,
)

article_app = typer.Typer(
    add_completion=False,
    help=(
        "Single-article no-write pipeline: resolve arXiv HTML/PDF (or local file), "
        "run load→structure→candidate→projection→promotion readiness package. "
        "Does not authorize graph import or writes."
    ),
)


@article_app.command("run")
def article_run(
    source: Annotated[
        str,
        typer.Argument(
            help=(
                "arXiv id/URL (abs|pdf|html) or local file path "
                "(.html/.md/.txt). Example: https://arxiv.org/html/2607.13104v1"
            )
        ),
    ],
    output_dir: Annotated[
        Path,
        typer.Option(
            "--output-dir",
            "-o",
            help="Work directory for acquired sources and readiness artifacts.",
        ),
    ] = Path("artifacts/single-article"),
    mode: Annotated[
        Literal["auto", "html", "pdf", "local"],
        typer.Option("--mode", help="Source mode: auto|html|pdf|local."),
    ] = "auto",
    prefer: Annotated[
        Literal["html", "pdf"],
        typer.Option("--prefer", help="When mode=auto for remote arXiv, prefer html or pdf body."),
    ] = "html",
    also_pdf: Annotated[
        bool,
        typer.Option("--also-pdf/--no-also-pdf", help="Also download PDF alongside HTML."),
    ] = True,
    review_completed: Annotated[
        bool,
        typer.Option(
            "--review-completed/--no-review-completed",
            help="Whether review post-check is treated as completed for pilot eligibility.",
        ),
    ] = True,
    json_output: Annotated[
        bool,
        typer.Option("--json", help="Print machine-readable package JSON to stdout."),
    ] = False,
) -> None:
    """Acquire one article and emit a fail-closed graph-data readiness package."""
    work_dir = output_dir.expanduser().resolve()
    work_dir.mkdir(parents=True, exist_ok=True)
    try:
        result = run_single_article_pipeline(
            SingleArticleRunRequest(
                source=source,
                work_dir=work_dir,
                mode=mode,
                prefer=prefer,
                also_pdf=also_pdf,
                review_completed=review_completed,
                repo_root=Path.cwd(),
            )
        )
    except (FileNotFoundError, ValueError) as exc:
        typer.echo(f"error: {exc}", err=True)
        raise typer.Exit(code=2) from exc
    except Exception as exc:  # noqa: BLE001 - CLI boundary
        typer.echo(f"failed: {type(exc).__name__}: {exc}", err=True)
        raise typer.Exit(code=1) from exc

    payload = result.to_dict()
    if json_output:
        typer.echo(json.dumps(payload, indent=2, sort_keys=True))
    else:
        pkg = result.readiness.package
        typer.echo(f"paper_id: {result.paper_id}")
        typer.echo(f"verdict: {pkg.verdict}")
        typer.echo(f"sources: {len(result.local_sources)}")
        for row in result.local_sources:
            typer.echo(f"  - {row['kind']}: {row['path']} ({row['origin']})")
        if result.package_path:
            typer.echo(f"package: {result.package_path}")
        if result.continuity_report_path:
            typer.echo(f"continuity: {result.continuity_report_path}")
        typer.echo("import_eligible: false")
        typer.echo("graph_writes_allowed: false")
        for source_row in pkg.sources:
            typer.echo(
                f"readiness[{source_row.paper_id}]: "
                f"load={source_row.load_ok} structure={source_row.structure_ok} "
                f"pilot_eligible={source_row.pilot_eligible} "
                f"chunks={source_row.chunk_count} blockers={list(source_row.blockers)}"
            )


def register(app: typer.Typer) -> None:
    app.add_typer(article_app, name="article")
