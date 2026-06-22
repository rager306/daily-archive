"""Validation batch CLI commands."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Annotated, Any

import typer

from research_graph.infrastructure.papers.artifacts.batch_validation import (
    run_article_batch_validation_report,
)
from research_graph.workflows.validation.batch_provenance import (
    build_artifact_freshness_report,
    read_validation_cli_provenance_log,
    select_provenance_entry,
    write_artifact_freshness_report,
)
from research_graph.workflows.validation.batch_state import (
    build_contract_response,
    read_batch_state,
)
from research_graph.workflows.validation.batch_workflow import (
    initialize_validation_batch,
    preflight_validation_batch,
    run_validation_batch_scan,
    validation_batch_state_preview,
    write_source_preflight_run,
)

validation_batch_app = typer.Typer(
    add_completion=False,
    context_settings={"help_option_names": ["-h", "--help"], "max_content_width": 120},
    help="Contract-only validation batch commands for M007.",
    no_args_is_help=True,
)


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
    milestone_id: Annotated[
        str | None,
        typer.Option("--milestone-id", help="Active milestone id to stamp into validation scan artifacts."),
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
        milestone_id=milestone_id,
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


@validation_batch_app.command("verify-artifacts")
def validation_batch_verify_artifacts(
    provenance_log: Annotated[
        Path,
        typer.Option("--provenance-log", help="Validation CLI provenance JSONL log."),
    ],
    run_id: Annotated[str | None, typer.Option("--run-id", help="Specific provenance run id to verify.")] = None,
    batch_id: Annotated[str | None, typer.Option("--batch-id", help="Batch id used to select the newest matching run.")] = None,
    command: Annotated[str | None, typer.Option("--command", help="Command label used to select the newest matching run.")] = None,
    report_path: Annotated[
        Path | None,
        typer.Option("--report-path", help="Optional path where the freshness report JSON should be written."),
    ] = None,
    json_output: Annotated[bool, typer.Option("--json", help="Print JSON response.")] = False,
) -> None:
    """Verify that recorded validation-batch artifacts still match provenance hashes."""
    try:
        entries = read_validation_cli_provenance_log(provenance_log)
        entry = select_provenance_entry(entries, run_id=run_id, batch_id=batch_id, command=command)
        report = build_artifact_freshness_report(entry)
        if report_path is not None:
            write_artifact_freshness_report(report, report_path)
            report["report_path"] = str(report_path)
    except (OSError, ValueError) as exc:
        response = {
            "status": "invalid_provenance",
            "verdict": "invalid_provenance",
            "error": str(exc),
            "production_import_attempted": False,
            "ladybugdb_written": False,
        }
        _echo_validation_batch_response(response, as_json=json_output)
        raise typer.Exit(1) from exc
    _echo_validation_batch_response(report, as_json=json_output)
    if report.get("verdict") != "fresh":
        raise typer.Exit(1)


@validation_batch_app.command("article-report")
def validation_batch_article_report(
    output_dir: Annotated[
        Path,
        typer.Option("--output-dir", help="Directory where S07 article report artifacts will be written."),
    ],
    manifest_path: Annotated[
        Path | None,
        typer.Option("--manifest-path", help="S07 article batch manifest JSON with metadata-only documents."),
    ] = None,
    state_path: Annotated[
        Path | None,
        typer.Option("--state-path", help="Existing validation batch-state.json to adapt into article metadata rows."),
    ] = None,
    provenance_log: Annotated[
        Path | None,
        typer.Option("--provenance-log", help="Optional validation CLI provenance JSONL path."),
    ] = None,
    limit: Annotated[int, typer.Option("--limit", help="Maximum documents to select, capped at ten.")] = 10,
    json_output: Annotated[bool, typer.Option("--json", help="Print JSON response.")] = False,
) -> None:
    """Write the S07 metadata-only 10-document article validation report."""
    try:
        response = run_article_batch_validation_report(
            manifest_path=manifest_path,
            state_path=state_path,
            output_dir=output_dir,
            provenance_log_path=provenance_log,
            limit=limit,
        )
    except ValueError as exc:
        response = {
            "status": "invalid_article_report_request",
            "command": "validation-batch article-report",
            "error": str(exc),
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
        }
        _echo_validation_batch_response(response, as_json=json_output)
        raise typer.Exit(2) from exc
    _echo_validation_batch_response(response, as_json=json_output)
    if response.get("exit_code") != 0:
        raise typer.Exit(1)


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




def register(app: typer.Typer) -> None:
    app.add_typer(validation_batch_app, name="validation-batch")
