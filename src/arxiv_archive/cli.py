"""CLI for arxiv-archive."""

import asyncio
import hashlib
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

from research_graph.papers.artifacts.minimax_boundary import (  # noqa: E402
    MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
    build_article_artifact_minimax_request,
    validate_article_artifact_minimax_response,
)
from research_graph.papers.artifacts.models import (  # noqa: E402
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
from research_graph.papers.artifacts.batch_validation import run_article_batch_validation_report  # noqa: E402
from arxiv_archive.arxiv_client import ArxivClient  # noqa: E402
from arxiv_archive.embedder import Embedder  # noqa: E402
from arxiv_archive.keyword_extractor import KeywordExtractor  # noqa: E402
from arxiv_archive.quality import (  # noqa: E402
    build_maintainability_report,
    maintainability_report_to_json,
    write_maintainability_report,
)
from arxiv_archive.scoring import ScoredPaper, ScoringEngine  # noqa: E402
from arxiv_archive.validation_batch_provenance import (  # noqa: E402
    build_artifact_freshness_report,
    read_validation_cli_provenance_log,
    select_provenance_entry,
    write_artifact_freshness_report,
)
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

quality_app = typer.Typer(
    add_completion=False,
    context_settings={"help_option_names": ["-h", "--help"], "max_content_width": 120},
    help="Local non-blocking quality diagnostics.",
    no_args_is_help=True,
)
app.add_typer(quality_app, name="quality")

ARTICLE_ARTIFACTS_HELP = """Deterministic article-artifacts commands for pre-KG review scaffolds.

Boundary: fixture-only processing; no production KG import, no LadybugDB writes,
no raw paper text, no binary payloads, no embeddings/vectors, and no model output.
Generated manifests are review-only candidates and are never import eligible.
"""

article_artifacts_app = typer.Typer(
    add_completion=False,
    context_settings={"help_option_names": ["-h", "--help"], "max_content_width": 120},
    help=ARTICLE_ARTIFACTS_HELP,
    no_args_is_help=True,
)
app.add_typer(article_artifacts_app, name="article-artifacts")


@quality_app.command("maintainability")
def quality_maintainability(
    paths: Annotated[
        list[Path] | None,
        typer.Argument(help="Files or directories to scan. Defaults to src/arxiv_archive."),
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


def _echo_article_artifacts_response(response: dict[str, Any], *, as_json: bool) -> None:
    if as_json:
        typer.echo(json.dumps(response, indent=2, sort_keys=True))
        return
    typer.echo(
        " | ".join(
            [
                f"status: {response['status']}",
                f"schema: {response.get('schema_version', ARTICLE_ARTIFACT_SCHEMA_VERSION)}",
                f"artifacts: {response.get('artifact_count', 0)}",
                f"diagnostics: {response.get('diagnostic_count', 0)}",
                "production import: false",
                "ladybugdb written: false",
            ]
        )
    )


def _article_artifacts_boundary_payload(status: str) -> dict[str, Any]:
    return {
        "status": status,
        "schema_version": ARTICLE_ARTIFACT_SCHEMA_VERSION,
        "run_schema_version": ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION,
        "boundary": ARTICLE_ARTIFACTS_HELP.strip(),
        "detector_mode": "deterministic_fixture_only",
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "raw_text_included": False,
        "raw_binary_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "model_outputs_included": False,
        "safety_flags": default_safety_flags(),
    }


@article_artifacts_app.command("contract")
def article_artifacts_contract(
    json_output: Annotated[
        bool,
        typer.Option("--json", help="Print the article-artifacts contract response as JSON."),
    ] = False,
) -> None:
    """Print the article-artifacts no-import boundary and schema versions."""
    _echo_article_artifacts_response(_article_artifacts_boundary_payload("contract_only"), as_json=json_output)


def _load_fixture_structure(input_structure: Path) -> dict[str, Any]:
    try:
        structure = json.loads(input_structure.read_text(encoding="utf-8"))
    except OSError as exc:
        raise typer.BadParameter(f"input structure could not be read: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise typer.BadParameter(f"input structure must be JSON: {exc.msg}") from exc
    if not isinstance(structure, dict):
        raise typer.BadParameter("input structure must be a JSON object")
    return structure


def _load_fixture_manifest(input_structure: Path) -> dict[str, Any]:
    structure = _load_fixture_structure(input_structure)

    try:
        manifest = build_article_artifact_manifest_from_structure(structure)
    except ValueError as exc:
        raise typer.BadParameter(str(exc)) from exc
    diagnostics = validate_article_artifact_manifest(manifest)
    if diagnostics:
        raise typer.BadParameter(f"generated fixture manifest failed contract validation with {len(diagnostics)} diagnostics")
    return manifest


def _hash_file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_minimax_mock_content_blocks(helper_response: Path | None) -> list[dict[str, Any]]:
    if helper_response is None:
        return [
            {
                "type": "tool_use",
                "name": "record_article_artifact_hints",
                "input": {
                    "schema_version": MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
                    "source_schema_version": "m023-redacted-article-structure.v1",
                    "manifest_schema_version": ARTICLE_ARTIFACT_SCHEMA_VERSION,
                    "input_sha256": "mock-response-without-input-hash",
                    "artifact_hints": [],
                    "helper_limit": 24,
                    "minimax_source_of_truth": False,
                    "promoted_to_fact": False,
                    "import_eligible": False,
                },
            }
        ]
    try:
        payload = json.loads(helper_response.read_text(encoding="utf-8"))
    except OSError as exc:
        raise typer.BadParameter(f"helper response could not be read: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise typer.BadParameter(f"helper response must be JSON: {exc.msg}") from exc
    if isinstance(payload, dict) and isinstance(payload.get("content_blocks"), list):
        payload = payload["content_blocks"]
    if not isinstance(payload, list) or not all(isinstance(block, dict) for block in payload):
        raise typer.BadParameter("helper response must be a JSON list of MiniMax content blocks")
    return payload


def _span_lookup(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    spans: dict[str, dict[str, Any]] = {}
    for artifact in manifest.get("artifacts", []):
        if not isinstance(artifact, dict):
            continue
        for span in artifact.get("source_spans", []):
            if isinstance(span, dict) and isinstance(span.get("span_id"), str):
                spans[span["span_id"]] = dict(span)
    return spans


def _helper_spans(span_ids: list[str], spans: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    return [dict(spans[span_id]) for span_id in span_ids if span_id in spans]


def _merge_minimax_helper_candidates(manifest: dict[str, Any], candidates: tuple[dict[str, Any], ...]) -> int:
    if not candidates:
        return 0
    paper_id = str(manifest["paper_id"])
    spans = _span_lookup(manifest)
    helper_artifacts: list[dict[str, Any]] = []
    excluded_uses = [
        "trusted_kg_import",
        "production_ladybugdb_write",
        "embedding_generation",
        "source_of_truth_claim",
    ]
    allowed_uses = ["artifact_review", "candidate_link_review", "provenance_diagnostics"]
    for candidate in candidates:
        source_spans = _helper_spans(candidate.get("evidence_span_ids", []), spans)
        candidate_links = []
        for link in candidate.get("candidate_links", []):
            target_hash = str(link.get("target_ref_hash", ""))
            candidate_links.append(
                {
                    "link_id": str(link["link_id"]),
                    "source_artifact_id": str(link["source_artifact_id"]),
                    "target_ref": f"sha256:{target_hash}",
                    "link_type": str(link["link_type"]),
                    "review_state": "review_required",
                    "source_spans": _helper_spans(link.get("evidence_span_ids", []), spans),
                    "confidence_label": str(candidate.get("confidence_label") or "needs_review"),
                    "diagnostic_codes": list(link.get("diagnostic_codes", [])),
                    "allowed_uses": allowed_uses,
                    "excluded_uses": excluded_uses,
                    "promoted_to_fact": False,
                    "import_eligible": False,
                    "metadata": {"target_ref_hash": target_hash, "helper_evidence_only": True},
                }
            )
        helper_artifacts.append(
            {
                "artifact_id": str(candidate["artifact_id"]),
                "paper_id": paper_id,
                "artifact_type": str(candidate["artifact_type"]),
                "review_state": "review_required",
                "source_refs": [],
                "source_spans": source_spans,
                "section_lineage": None,
                "candidate_links": candidate_links,
                "confidence_label": str(candidate.get("confidence_label") or "needs_review"),
                "detector": str(candidate.get("detector", "minimax_artifact_helper_review_only")),
                "diagnostic_codes": list(candidate.get("diagnostic_codes", [])),
                "metadata": {
                    "helper_mode": "minimax-mock",
                    "helper_evidence_only": True,
                    "raw_model_content_persisted": False,
                    "helper_non_authority": True,
                },
                "safety_flags": default_safety_flags(),
                "allowed_uses": allowed_uses,
                "excluded_uses": excluded_uses,
                "promoted_to_fact": False,
                "import_eligible": False,
            }
        )
    manifest["artifacts"] = [*manifest.get("artifacts", []), *helper_artifacts]
    missing_span_count = int(manifest.get("summary", {}).get("missing_span_count", 0) or 0)
    manifest["summary"] = summarize_article_artifacts(manifest["artifacts"])
    manifest["summary"]["missing_span_count"] = missing_span_count
    manifest["summary"]["diagnostic_summary"] = build_article_artifact_diagnostics_summary(manifest)
    diagnostics = validate_article_artifact_manifest(manifest)
    if diagnostics:
        raise typer.BadParameter(f"merged helper manifest failed contract validation with {len(diagnostics)} diagnostics")
    return len(helper_artifacts)


def _apply_article_artifacts_helper(
    manifest: dict[str, Any],
    *,
    structure: dict[str, Any],
    helper: str,
    helper_response: Path | None,
    max_helper_candidates: int,
) -> dict[str, Any]:
    helper_diagnostics: dict[str, Any] = {
        "helper_mode": helper,
        "helper_request_attempted": False,
        "helper_validation_status": "not_requested",
        "helper_schema_version": MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
        "merged_candidate_count": 0,
        "blocked_import_flags": {
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
            "promoted_to_fact": False,
            "import_eligible": False,
        },
        "raw_response_persisted": False,
        "helper_evidence_only": True,
    }
    if helper in {"deterministic", "none"}:
        return helper_diagnostics
    if helper != "minimax-mock":
        raise typer.BadParameter("helper must be one of: deterministic, none, minimax-mock")
    helper_request = build_article_artifact_minimax_request(structure, max_candidates=max_helper_candidates)
    content_blocks = _load_minimax_mock_content_blocks(helper_response)
    helper_result = validate_article_artifact_minimax_response(
        content_blocks,
        structure=structure,
        max_candidates=max_helper_candidates,
    )
    merged_count = 0
    if helper_result.diagnostics.get("response_validation_status") == "valid":
        merged_count = _merge_minimax_helper_candidates(manifest, helper_result.candidates)
    helper_diagnostics.update(helper_request.diagnostics)
    helper_diagnostics.update(helper_result.diagnostics)
    helper_diagnostics.update(
        {
            "helper_mode": helper,
            "helper_request_attempted": True,
            "helper_validation_status": helper_result.diagnostics.get("response_validation_status", "invalid"),
            "merged_candidate_count": merged_count,
            "provider_candidate_count": helper_result.diagnostics.get("provider_candidate_count", 0),
            "blocked_import_flags": {
                "production_import_attempted": False,
                "ladybugdb_written": False,
                "trusted_kg_import_allowed": False,
                "promoted_to_fact": False,
                "import_eligible": False,
            },
            "raw_response_persisted": False,
            "helper_evidence_only": True,
        }
    )
    manifest.setdefault("helper_diagnostics", {})[helper] = dict(helper_diagnostics)
    return helper_diagnostics


@article_artifacts_app.command("detect")
def article_artifacts_detect(
    input_structure: Annotated[
        Path,
        typer.Option("--input-structure", help="Redacted article structure fixture JSON."),
    ],
    output_dir: Annotated[
        Path,
        typer.Option("--output-dir", help="Directory where redacted article-artifact manifests will be written."),
    ],
    json_output: Annotated[bool, typer.Option("--json", help="Print JSON response.")] = False,
    helper: Annotated[
        str,
        typer.Option("--helper", help="Optional helper mode: deterministic, none, or minimax-mock."),
    ] = "deterministic",
    helper_response: Annotated[
        Path | None,
        typer.Option("--helper-response", help="JSON MiniMax content blocks for --helper minimax-mock."),
    ] = None,
    max_helper_candidates: Annotated[
        int,
        typer.Option("--max-helper-candidates", help="Maximum helper candidates accepted from MiniMax mock output."),
    ] = 24,
) -> None:
    """Generate review-only article artifact manifests from deterministic fixtures."""
    structure = _load_fixture_structure(input_structure)
    try:
        manifest = build_article_artifact_manifest_from_structure(structure)
    except ValueError as exc:
        raise typer.BadParameter(str(exc)) from exc
    diagnostics = validate_article_artifact_manifest(manifest)
    if diagnostics:
        raise typer.BadParameter(f"generated fixture manifest failed contract validation with {len(diagnostics)} diagnostics")
    helper_diagnostics = _apply_article_artifacts_helper(
        manifest,
        structure=structure,
        helper=helper,
        helper_response=helper_response,
        max_helper_candidates=max_helper_candidates,
    )
    output_dir.mkdir(parents=True, exist_ok=True)

    manifest_path = output_dir / f"{manifest['paper_id']}-article-artifacts.json"
    run_summary_path = output_dir / "article-artifacts-run-summary.json"
    diagnostics_path = output_dir / "article-artifacts-diagnostics.json"
    input_hashes = {"input_structure_sha256": _hash_file_sha256(input_structure)}
    output_paths = {
        "manifest": str(manifest_path),
        "run_summary": str(run_summary_path),
        "diagnostics": str(diagnostics_path),
    }
    run_summary = ArticleArtifactRunSummary(
        run_id=str(manifest["run_id"]),
        manifests=(manifest,),
        input_hashes=input_hashes,
        output_paths=output_paths,
    ).to_redacted_dict()
    diagnostics_artifact = build_article_artifact_run_diagnostics_artifact(
        run_id=str(manifest["run_id"]),
        manifests=(manifest,),
        input_hashes=input_hashes,
        output_paths=output_paths,
    )
    run_summary["helper_diagnostics"] = helper_diagnostics
    diagnostics_artifact["helper_diagnostics"] = helper_diagnostics
    manifest_path.write_text(to_json(manifest), encoding="utf-8")
    run_summary_path.write_text(to_json(run_summary), encoding="utf-8")
    diagnostics_path.write_text(to_json(diagnostics_artifact), encoding="utf-8")

    response = _article_artifacts_boundary_payload("detected")
    response.update(
        {
            "input_structure_path": str(input_structure),
            "manifest_path": str(manifest_path),
            "run_summary_path": str(run_summary_path),
            "diagnostics_path": str(diagnostics_path),
            "output_paths": output_paths,
            "input_hashes": input_hashes,
            "run_id": manifest["run_id"],
            "paper_id": manifest["paper_id"],
            "artifact_count": manifest["summary"]["artifact_count"],
            "candidate_link_count": manifest["summary"]["candidate_link_count"],
            "diagnostic_count": len(manifest.get("diagnostics", [])),
            "diagnostics": manifest.get("diagnostics", []),
            "artifact_counts_by_type": manifest["summary"].get("artifact_counts_by_type", {}),
            "review_state_counts": manifest["summary"].get("review_state_counts", {}),
            "provenance_hints": {
                "input_structure_path": str(input_structure),
                "source_refs": manifest.get("source_refs", []),
                "detector": "redacted_fixture_v1",
                "helper_mode": helper_diagnostics["helper_mode"],
            },
            "helper_mode": helper_diagnostics["helper_mode"],
            "helper_validation_status": helper_diagnostics["helper_validation_status"],
            "helper_merged_candidate_count": helper_diagnostics["merged_candidate_count"],
            "helper_request_attempted": helper_diagnostics["helper_request_attempted"],
            "blocked_import_flags": helper_diagnostics["blocked_import_flags"],
            "helper_diagnostics": helper_diagnostics,
            "diagnostic_summary": build_article_artifact_diagnostics_summary(manifest),
            "diagnostic_codes": diagnostics_artifact["diagnostic_codes"],
            "missing_span_count": manifest.get("summary", {}).get("missing_span_count", 0),
            "import_eligible_count": 0,
            "promoted_to_fact_count": 0,
        }
    )
    _echo_article_artifacts_response(response, as_json=json_output)


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
