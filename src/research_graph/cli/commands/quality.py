"""Quality diagnostic CLI commands."""

from __future__ import annotations

from pathlib import Path
from typing import Annotated

import typer

from research_graph.quality import (
    build_maintainability_report,
    maintainability_report_to_json,
    write_maintainability_report,
)

quality_app = typer.Typer(
    add_completion=False,
    context_settings={"help_option_names": ["-h", "--help"], "max_content_width": 120},
    help="Local non-blocking quality diagnostics.",
    no_args_is_help=True,
)


@quality_app.command("maintainability")
def quality_maintainability(
    paths: Annotated[
        list[Path] | None,
        typer.Argument(help="Files or directories to scan. Defaults to src/research_graph."),
    ] = None,
    baseline_path: Annotated[
        Path | None,
        typer.Option("--baseline", help="Optional diagnostic baseline JSON to compare against."),
    ] = None,
    output_path: Annotated[
        Path | None,
        typer.Option("--output", help="Optional JSON artifact path for the diagnostic report."),
    ] = None,
    json_output: Annotated[bool, typer.Option("--json", help="Print the diagnostic report as JSON.")] = False,
) -> None:
    """Run the informational riskratchet maintainability diagnostic."""
    report = build_maintainability_report(paths=paths, baseline_path=baseline_path)
    if output_path is not None:
        write_maintainability_report(report, output_path)
        report["output_path"] = str(output_path)
    if json_output:
        typer.echo(maintainability_report_to_json(report).rstrip())
        return
    summary = report.get("summary", {})
    delta = report.get("baseline_delta", {})
    typer.echo(
        " | ".join(
            [
                f"status: {report['status']}",
                "diagnostic only: true",
                f"functions: {summary.get('total_functions', 0)}",
                f"max score: {summary.get('max_score', 0.0)}",
                f"severity: {summary.get('by_severity', {})}",
                f"baseline delta: {delta.get('max_score_delta')}",
                "blocking: false",
            ]
        )
    )




def register(app: typer.Typer) -> None:
    app.add_typer(quality_app, name="quality")
