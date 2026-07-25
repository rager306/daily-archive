"""Article artifacts CLI commands."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Annotated, Any

import typer

from research_graph.infrastructure.papers.artifacts.minimax_boundary import (
    MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
    build_article_artifact_minimax_request,
    validate_article_artifact_minimax_response,
)
from research_graph.infrastructure.papers.artifacts.models import (
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
    _echo_article_artifacts_response(
        _article_artifacts_boundary_payload("contract_only"), as_json=json_output
    )


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
        raise typer.BadParameter(
            f"generated fixture manifest failed contract validation with {len(diagnostics)} diagnostics"
        )
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
                    "source_schema_version": "redacted-article-structure.v1",
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


def _merge_minimax_helper_candidates(
    manifest: dict[str, Any], candidates: tuple[dict[str, Any], ...]
) -> int:
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
        raise typer.BadParameter(
            f"merged helper manifest failed contract validation with {len(diagnostics)} diagnostics"
        )
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
    helper_request = build_article_artifact_minimax_request(
        structure, max_candidates=max_helper_candidates
    )
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
            "helper_validation_status": helper_result.diagnostics.get(
                "response_validation_status", "invalid"
            ),
            "merged_candidate_count": merged_count,
            "provider_candidate_count": helper_result.diagnostics.get(
                "provider_candidate_count", 0
            ),
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
        typer.Option(
            "--output-dir",
            help="Directory where redacted article-artifact manifests will be written.",
        ),
    ],
    json_output: Annotated[bool, typer.Option("--json", help="Print JSON response.")] = False,
    helper: Annotated[
        str,
        typer.Option(
            "--helper", help="Optional helper mode: deterministic, none, or minimax-mock."
        ),
    ] = "deterministic",
    helper_response: Annotated[
        Path | None,
        typer.Option(
            "--helper-response", help="JSON MiniMax content blocks for --helper minimax-mock."
        ),
    ] = None,
    max_helper_candidates: Annotated[
        int,
        typer.Option(
            "--max-helper-candidates",
            help="Maximum helper candidates accepted from MiniMax mock output.",
        ),
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
        raise typer.BadParameter(
            f"generated fixture manifest failed contract validation with {len(diagnostics)} diagnostics"
        )
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


def register(app: typer.Typer) -> None:
    app.add_typer(article_artifacts_app, name="article-artifacts")
