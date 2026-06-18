# Formerly: src/arxiv_archive/validation_batch_workflow.py

"""Validation batch workflow helpers for M007.

These helpers implement deterministic local state/artifact preparation only. They
inspect manifest/source paths and write redacted batch preflight artifacts. They
do not acquire sources, convert PDFs, run scans, import KG facts, or write to
LadybugDB.
"""

from __future__ import annotations

import json
from dataclasses import asdict, replace
from pathlib import Path
from typing import Any

from research_graph.corpus.ingestion.loader import load_article_source
from research_graph.corpus.sources.thirty_paper_deviation_scan import build_thirty_paper_deviation_scan
from research_graph.workflows.validation.batch_state import (
    ScanArtifactPaths,
    SelectedPaper,
    SourceReadiness,
    ValidationBatchState,
    batch_state_to_dict,
    build_batch_diagnostics,
    default_safety_flags,
    write_batch_state,
)
from research_graph.workflows.validation.logging import ValidationLogger, sanitize_event_details
from scripts.run_quality_gate import run_quality_gate

VALIDATION_SMOKE_REVIEW_SCHEMA_VERSION = "m025-validation-smoke-review.v1"

SELECTION_ROLE_ALIASES = {
    "m005_baseline_overlap": "baseline_overlap",
    "baseline_overlap": "baseline_overlap",
    "deterministic_expansion": "deterministic_expansion",
    "retry": "retry",
    "repaired": "repaired",
    "excluded": "excluded",
    "manual_review_target": "manual_review_target",
}


def load_validation_manifest(path: str | Path) -> dict[str, Any]:
    manifest_path = Path(path)
    payload = json.loads(manifest_path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"expected JSON object at {manifest_path}")
    if not isinstance(payload.get("papers"), list):
        raise ValueError(f"manifest at {manifest_path} must contain a papers list")
    return payload


def batch_artifact_dir(root_dir: str | Path, batch_id: str) -> Path:
    return Path(root_dir) / batch_id


def selected_papers_from_manifest(manifest: dict[str, Any], *, limit: int | None = None) -> tuple[SelectedPaper, ...]:
    papers: list[SelectedPaper] = []
    for raw_paper in manifest.get("papers", []):
        if not isinstance(raw_paper, dict):
            continue
        paper_id = str(raw_paper["paper_id"])
        source_paths = {
            str(key): str(value)
            for key, value in (raw_paper.get("source_paths") or {}).items()
            if value is not None
        }
        role = SELECTION_ROLE_ALIASES.get(str(raw_paper.get("selection_role", "")), "manual_review_target")
        papers.append(
            SelectedPaper(
                paper_id=paper_id,
                rank=raw_paper.get("rank"),
                selection_role=role,
                risk_tags=tuple(str(value) for value in raw_paper.get("risk_tags", ())),
                source_paths=source_paths,
                notes=tuple(str(value) for value in raw_paper.get("notes", ())),
            )
        )
    papers.sort(key=lambda paper: (paper.rank is None, paper.rank if paper.rank is not None else 999_999, paper.paper_id))
    if limit is not None:
        papers = papers[:limit]
    return tuple(papers)


def initialize_validation_batch(
    *,
    manifest_path: str | Path,
    batch_id: str,
    output_dir: str | Path,
    limit: int | None = None,
) -> dict[str, Any]:
    """Create initial batch state and selection manifest artifacts."""
    manifest = load_validation_manifest(manifest_path)
    selected_papers = selected_papers_from_manifest(manifest, limit=limit)
    artifact_dir = batch_artifact_dir(output_dir, batch_id)
    artifact_dir.mkdir(parents=True, exist_ok=True)
    state = ValidationBatchState(
        batch_id=batch_id,
        phase="initialized",
        selected_papers=selected_papers,
        input_manifests=(str(manifest_path),),
        artifact_paths=ScanArtifactPaths(),
    )
    state_path = write_batch_state(state, artifact_dir / "batch-state.json")
    selection_manifest_path = artifact_dir / "selection-manifest.json"
    selection_manifest = {
        "schema_version": "m007-validation-batch-selection.v1",
        "batch_id": batch_id,
        "paper_count": len(selected_papers),
        "source_manifest": str(manifest_path),
        "selected_papers": [asdict(paper) for paper in selected_papers],
        **default_safety_flags(),
    }
    selection_manifest_path.write_text(json.dumps(selection_manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return {
        "state": state,
        "artifact_dir": artifact_dir,
        "state_path": state_path,
        "selection_manifest_path": selection_manifest_path,
    }


def source_readiness_for_paper(
    paper: SelectedPaper,
    *,
    fallback_root: str | Path = "/root/.research/papers",
    cache_root: str | Path = "/root/.arxiv_cache",
    logger: ValidationLogger | None = None,
) -> SourceReadiness:
    fallback_root_path = Path(fallback_root)
    cache_root_path = Path(cache_root)
    markdown_candidates = (
        paper.source_paths.get("research_full_text_md"),
        paper.source_paths.get("cache_markdown"),
        str(fallback_root_path / paper.paper_id / "full_text.md"),
        str(cache_root_path / f"{paper.paper_id}.md"),
    )
    pdf_candidates = (
        paper.source_paths.get("cache_pdf"),
        paper.source_paths.get("research_pdf"),
        str(cache_root_path / f"{paper.paper_id}.pdf"),
        str(fallback_root_path / paper.paper_id / "paper.pdf"),
    )
    markdown_path = _first_existing_path(*markdown_candidates) or _first_candidate_path(*markdown_candidates)
    pdf_path = _first_existing_path(*pdf_candidates) or _first_candidate_path(*pdf_candidates)

    markdown_result = load_article_source(markdown_path, source_type="markdown", paper_id=paper.paper_id, logger=logger)
    pdf_result = load_article_source(pdf_path, source_type="pdf", paper_id=paper.paper_id, logger=logger)

    markdown_present = markdown_result.sha256 is not None
    markdown_quality_accepted = markdown_result.outcome == "loaded" and markdown_result.failure_reason is None
    pdf_present = pdf_result.sha256 is not None and pdf_result.outcome == "loaded_metadata_only"
    risk_tags = set(paper.risk_tags)
    conversion_failed = "conversion_failed" in risk_tags or ("missing_markdown" in risk_tags and not markdown_present)
    conversion_repaired = "docling_repair" in risk_tags or "conversion_repaired" in risk_tags
    unavailable_source = not markdown_present and not pdf_present
    return SourceReadiness(
        markdown_present=markdown_present,
        markdown_quality_accepted=markdown_quality_accepted,
        pdf_present=pdf_present,
        pdf_missing=not pdf_present,
        conversion_repaired=conversion_repaired,
        conversion_failed=conversion_failed,
        unavailable_source=unavailable_source,
        ready_for_markdown_scan=markdown_present and markdown_quality_accepted,
        loader_provenance_by_role={
            "markdown": _loader_provenance(markdown_result),
            "pdf": _loader_provenance(pdf_result),
        },
    )


def preflight_validation_batch(state: ValidationBatchState) -> ValidationBatchState:
    readiness = {paper.paper_id: source_readiness_for_paper(paper) for paper in state.selected_papers}
    next_phase = "source_ready" if readiness and all(item.ready_for_markdown_scan for item in readiness.values()) else "source_blocked"
    updated = replace(
        state,
        phase=next_phase,
        source_readiness_by_paper=readiness,
    )
    return replace(updated, diagnostics=tuple(build_batch_diagnostics(updated)))


def write_source_preflight_run(state: ValidationBatchState, output_dir: str | Path) -> dict[str, Path]:
    """Write state, summary, and JSONL diagnostics for a preflighted batch."""
    output = Path(output_dir)
    output.mkdir(parents=True, exist_ok=True)
    state_path = write_batch_state(state, output / "batch-state.json")
    summary_path = output / "source-preflight-summary.json"
    diagnostics_path = output / "source-preflight-diagnostics.jsonl"
    summary = build_source_preflight_summary(state)
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    with diagnostics_path.open("w", encoding="utf-8") as handle:
        for diagnostic in state.diagnostics:
            handle.write(json.dumps(diagnostic, sort_keys=True, separators=(",", ":")) + "\n")
    return {"state_path": state_path, "summary_path": summary_path, "diagnostics_path": diagnostics_path}


def build_source_preflight_summary(state: ValidationBatchState) -> dict[str, Any]:
    readiness_values = list(state.source_readiness_by_paper.values())
    diagnostics = list(state.diagnostics)
    return {
        "schema_version": "m007-source-preflight-summary.v1",
        "batch_id": state.batch_id,
        "phase": state.phase,
        "paper_count": len(state.selected_papers),
        "markdown_present_count": sum(1 for item in readiness_values if item.markdown_present),
        "markdown_quality_accepted_count": sum(1 for item in readiness_values if item.markdown_quality_accepted),
        "ready_for_markdown_scan_count": sum(1 for item in readiness_values if item.ready_for_markdown_scan),
        "pdf_present_count": sum(1 for item in readiness_values if item.pdf_present),
        "pdf_missing_count": sum(1 for item in readiness_values if item.pdf_missing),
        "conversion_repaired_count": sum(1 for item in readiness_values if item.conversion_repaired),
        "conversion_failed_count": sum(1 for item in readiness_values if item.conversion_failed),
        "unavailable_source_count": sum(1 for item in readiness_values if item.unavailable_source),
        "diagnostic_count": len(diagnostics),
        "blocker_count": sum(1 for item in diagnostics if item.get("severity") == "blocker"),
        "warning_count": sum(1 for item in diagnostics if item.get("severity") == "warning"),
        **default_safety_flags(),
    }


def validation_batch_state_preview(state: ValidationBatchState) -> dict[str, Any]:
    """Return a compact redacted preview suitable for CLI output."""
    payload = batch_state_to_dict(state)
    return {
        "schema_version": payload["schema_version"],
        "batch_id": payload["batch_id"],
        "phase": payload["phase"],
        "paper_count": len(payload["selected_papers"]),
        "diagnostic_count": len(payload["diagnostics"]),
        **default_safety_flags(),
    }


def run_validation_batch_scan(
    state: ValidationBatchState,
    output_dir: str | Path,
    *,
    structure_baseline_path: str | Path | None = None,
    mixed_benchmark_path: str | Path | None = None,
    run_id: str | None = None,
    milestone_id: str | None = None,
) -> dict[str, Any]:
    """Run the redacted structure-aware deviation scan for a preflighted batch."""
    if state.phase not in {"source_ready", "scan_ready", "scanned", "review_required"}:
        raise ValueError(f"batch must be source_ready or scan_ready before scan, got {state.phase}")
    output = Path(output_dir)
    output.mkdir(parents=True, exist_ok=True)
    lineage = _scan_lineage(state, milestone_id=milestone_id)
    manifest_path = write_validation_scan_manifest(state, output / "validation-scan-manifest.json", lineage=lineage)
    source_summary_path = write_validation_scan_source_readiness(
        state, output / "validation-scan-source-readiness.json", lineage=lineage
    )
    scan = build_thirty_paper_deviation_scan(
        manifest_path=manifest_path,
        source_acquisition_summary_path=source_summary_path,
        baseline_summary_path=mixed_benchmark_path,
        run_id=run_id or f"{state.batch_id}-validation-scan",
    )
    summary_path, diagnostics_path = write_validation_scan_artifacts(scan, output, lineage=lineage)
    delta_path = output / "delta-report.json"
    outlier_path = output / "outlier-report.json"
    delta_report = build_delta_report(
        scan,
        structure_baseline_path=structure_baseline_path,
        mixed_benchmark_path=mixed_benchmark_path,
        lineage=lineage,
    )
    outlier_report = build_outlier_report(scan, lineage=lineage)
    delta_path.write_text(json.dumps(delta_report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    outlier_path.write_text(json.dumps(outlier_report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    scan_diagnostics = tuple(scan_import_gate_diagnostics(scan))
    artifact_paths = ScanArtifactPaths(
        aggregate_summary_json=str(summary_path),
        per_paper_diagnostics_jsonl=str(diagnostics_path),
        delta_report_json=str(delta_path),
        outlier_report_json=str(outlier_path),
        review_summary_md=state.artifact_paths.review_summary_md,
    )
    updated = replace(
        state,
        phase="review_required" if scan_diagnostics else "scanned",
        artifact_paths=artifact_paths,
        diagnostics=tuple(state.diagnostics) + scan_diagnostics,
    )
    state_path = write_batch_state(updated, output / "batch-state.json")
    return {
        "state": updated,
        "state_path": state_path,
        "manifest_path": manifest_path,
        "source_readiness_path": source_summary_path,
        "summary_path": summary_path,
        "diagnostics_path": diagnostics_path,
        "delta_report_path": delta_path,
        "outlier_report_path": outlier_path,
    }


def run_validation_batch_smoke_with_quality_gate(
    state: ValidationBatchState,
    output_dir: str | Path,
    *,
    structure_baseline_path: str | Path | None = None,
    mixed_benchmark_path: str | Path | None = None,
    run_id: str | None = None,
    milestone_id: str | None = None,
    quality_gate_paths: tuple[str | Path, ...] | list[str | Path] | None = None,
    quality_gate_baseline_path: str | Path | None = None,
) -> dict[str, Any]:
    """Run a validation smoke scan and attach non-blocking maintainability diagnostics.

    The maintainability gate is informational: scan pass/fail status is determined
    by the functional validation scan, while quality diagnostics are written next
    to the smoke artifacts for local review.
    """
    output = Path(output_dir)
    scan_result = run_validation_batch_scan(
        state,
        output,
        structure_baseline_path=structure_baseline_path,
        mixed_benchmark_path=mixed_benchmark_path,
        run_id=run_id,
        milestone_id=milestone_id,
    )
    quality_dir = output / "quality"
    quality_report = run_quality_gate(
        paths=quality_gate_paths
        or (
            Path("src/arxiv_archive/validation_batch_workflow.py"),
            Path("scripts/run_quality_gate.py"),
            Path("src/arxiv_archive/quality/riskratchet_adapter.py"),
        ),
        output_dir=quality_dir,
        baseline_path=quality_gate_baseline_path,
    )
    review_path = write_validation_smoke_review(
        scan_result=scan_result,
        quality_report=quality_report,
        output_path=output / "validation-smoke-review.json",
    )
    return {
        **scan_result,
        "quality_gate_report": quality_report,
        "quality_gate_json_path": Path(quality_report["output_paths"]["json"]),
        "quality_gate_human_path": Path(quality_report["output_paths"]["human"]),
        "smoke_review_path": review_path,
    }


def write_validation_smoke_review(
    *,
    scan_result: dict[str, Any],
    quality_report: dict[str, Any],
    output_path: str | Path,
) -> Path:
    """Write the combined local smoke review envelope."""
    path = Path(output_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    state = scan_result.get("state")
    payload = {
        "schema_version": VALIDATION_SMOKE_REVIEW_SCHEMA_VERSION,
        "batch_id": getattr(state, "batch_id", None),
        "phase": getattr(state, "phase", None),
        "functional_smoke": {
            "state_path": str(scan_result["state_path"]),
            "summary_path": str(scan_result["summary_path"]),
            "diagnostics_path": str(scan_result["diagnostics_path"]),
            "delta_report_path": str(scan_result["delta_report_path"]),
            "outlier_report_path": str(scan_result["outlier_report_path"]),
        },
        "maintainability_diagnostic": {
            "diagnostic_only": True,
            "blocking": False,
            "pass_fail_affected": False,
            "status": quality_report.get("status"),
            "tool_status": quality_report.get("tool_status"),
            "summary": quality_report.get("summary", {}),
            "baseline_delta": quality_report.get("baseline_delta", {}),
            "output_paths": quality_report.get("output_paths", {}),
            "quality_gate": quality_report.get("quality_gate", {}),
        },
        **default_safety_flags(),
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def write_validation_scan_manifest(
    state: ValidationBatchState, path: str | Path, *, lineage: dict[str, str | None] | None = None
) -> Path:
    output_path = Path(path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    papers = [_paper_manifest_record(paper) for paper in state.selected_papers]
    payload = {
        "schema_version": "m007-validation-scan-manifest.v1",
        "batch_id": state.batch_id,
        "paper_count": len(papers),
        "m005_overlap_count": sum(1 for paper in papers if paper.get("selection_role") == "m005_baseline_overlap"),
        "expansion_count": sum(1 for paper in papers if paper.get("selection_role") == "deterministic_expansion"),
        "papers": papers,
        **_lineage_payload(lineage),
        **default_safety_flags(),
    }
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output_path


def write_validation_scan_source_readiness(
    state: ValidationBatchState, path: str | Path, *, lineage: dict[str, str | None] | None = None
) -> Path:
    output_path = Path(path)
    readiness_values = list(state.source_readiness_by_paper.values())
    payload = {
        "schema_version": "m007-validation-scan-source-readiness.v1",
        "batch_id": state.batch_id,
        "paper_count": len(state.selected_papers),
        "ready_for_markdown_scan_count": sum(1 for item in readiness_values if item.ready_for_markdown_scan),
        "still_missing_markdown_count": sum(1 for item in readiness_values if not item.ready_for_markdown_scan),
        "available_pdf_count": sum(1 for item in readiness_values if item.pdf_present),
        **_lineage_payload(lineage),
        **default_safety_flags(),
    }
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output_path


def write_validation_scan_artifacts(
    scan: dict[str, Any], output_dir: str | Path, *, lineage: dict[str, str | None] | None = None
) -> tuple[Path, Path]:
    output = Path(output_dir)
    summary = {key: value for key, value in scan.items() if key != "records"}
    summary.update(_lineage_payload(lineage))
    if lineage and lineage.get("milestone_id"):
        summary["milestone"] = lineage["milestone_id"]
    summary["schema_version"] = "m007-validation-scan-summary.v1"
    summary_path = output / "validation-scan-summary.json"
    diagnostics_path = output / "validation-scan-diagnostics.jsonl"
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    with diagnostics_path.open("w", encoding="utf-8") as handle:
        for record in scan.get("records", []):
            handle.write(json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n")
    return summary_path, diagnostics_path


def build_delta_report(
    scan: dict[str, Any],
    *,
    structure_baseline_path: str | Path | None = None,
    mixed_benchmark_path: str | Path | None = None,
    lineage: dict[str, str | None] | None = None,
) -> dict[str, Any]:
    aggregate = scan.get("aggregate", {})
    structure_baseline = _read_optional_json(structure_baseline_path)
    mixed_benchmark = _read_optional_json(mixed_benchmark_path)
    return {
        "schema_version": "m007-validation-delta-report.v1",
        "paper_count": scan.get("paper_count"),
        "current_chunk_count": aggregate.get("chunk_count", 0),
        "current_import_eligible_chunk_count": aggregate.get("import_eligible_chunk_count", 0),
        "structure_aware_baseline": _baseline_delta(aggregate, structure_baseline),
        "mixed_benchmark_context": _mixed_benchmark_context(aggregate, mixed_benchmark),
        "route_share_delta": _share_delta(aggregate.get("counts_by_route", {}), (structure_baseline or {}).get("counts_by_route", {})),
        "refusal_share_delta": _share_delta(aggregate.get("refusal_counts", {}), (structure_baseline or {}).get("refusal_counts", {})),
        **_lineage_payload(lineage),
        **default_safety_flags(),
    }


def build_outlier_report(scan: dict[str, Any], *, lineage: dict[str, str | None] | None = None) -> dict[str, Any]:
    records = scan.get("records", [])
    outliers = scan.get("outliers", [])
    density_by_paper = {
        str(record.get("paper_id")): record.get("chunks_per_10k_bytes", 0.0)
        for record in records
        if isinstance(record, dict)
    }
    enriched_outliers = []
    for outlier in outliers:
        paper_id = str(outlier.get("paper_id"))
        enriched = dict(outlier)
        enriched["chunks_per_10k_bytes"] = density_by_paper.get(paper_id, 0.0)
        enriched_outliers.append(enriched)
    return {
        "schema_version": "m007-validation-outlier-report.v1",
        "outlier_count": len(enriched_outliers),
        "thresholds": {
            "high_chunk_count": "chunk_count >= max(2 * median_chunk_count, median_chunk_count + 25)",
            "claim_candidate_heavy": "claim_extraction route count >= 25",
            "table_heavy": "table_extraction route count >= 10",
            "unexpected_import_eligible_chunks": "import_eligible_chunk_count > 0",
        },
        "outliers": enriched_outliers,
        **_lineage_payload(lineage),
        **default_safety_flags(),
    }


def _scan_lineage(state: ValidationBatchState, *, milestone_id: str | None = None) -> dict[str, str | None]:
    return {"milestone_id": milestone_id, "batch_id": state.batch_id}


def _lineage_payload(lineage: dict[str, str | None] | None) -> dict[str, str]:
    if not lineage:
        return {}
    return {key: value for key, value in lineage.items() if value is not None}


def scan_import_gate_diagnostics(scan: dict[str, Any]) -> list[dict[str, str]]:
    import_eligible = int(scan.get("aggregate", {}).get("import_eligible_chunk_count", 0) or 0)
    if import_eligible == 0:
        return []
    return [
        {
            "severity": "blocker",
            "code": "unexpected_import_eligible_chunks",
            "message": f"Validation scan produced {import_eligible} import-eligible chunks outside a reviewed promotion path.",
            "recommended_action": "Stop automation and run independent review before any KG import work.",
        }
    ]


def build_quota_fill_report(
    state: ValidationBatchState,
    *,
    target_count: int,
    candidate_inventory: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build a redacted quota-fill report for a validation batch."""
    selected_ids = {paper.paper_id for paper in state.selected_papers}
    records = [_quota_fill_record(paper, state.source_readiness_by_paper.get(paper.paper_id)) for paper in state.selected_papers]
    accepted = [record for record in records if record["outcome"] == "accepted_ready"]
    rejected = [record for record in records if record["outcome"] != "accepted_ready"]
    shortage_count = max(target_count - len(accepted), 0)
    replacement_candidates = _replacement_candidates(candidate_inventory, selected_ids, limit=shortage_count)
    return {
        "schema_version": "m008-quota-fill-summary.v1",
        "batch_id": state.batch_id,
        "target_count": target_count,
        "attempted_count": len(records),
        "accepted_count": len(accepted),
        "accepted_ready_count": len(accepted),
        "rejected_count": len(rejected),
        "shortage_count": shortage_count,
        "replacement_candidate_count": len(replacement_candidates),
        "scan_allowed": len(accepted) == target_count and shortage_count == 0,
        "records": records,
        "replacement_candidates": replacement_candidates,
        **default_safety_flags(),
    }


def write_quota_fill_run(report: dict[str, Any], output_dir: str | Path) -> dict[str, Path]:
    """Write quota-fill summary and diagnostics JSONL."""
    output = Path(output_dir)
    output.mkdir(parents=True, exist_ok=True)
    summary_path = output / "quota-fill-summary.json"
    diagnostics_path = output / "quota-fill-diagnostics.jsonl"
    summary = {key: value for key, value in report.items() if key not in {"records", "replacement_candidates"}}
    summary["replacement_candidates"] = report.get("replacement_candidates", [])
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    with diagnostics_path.open("w", encoding="utf-8") as handle:
        for record in report.get("records", []):
            if record.get("outcome") != "accepted_ready":
                handle.write(json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n")
        if int(report.get("shortage_count", 0)) > 0:
            handle.write(
                json.dumps(
                    {
                        "severity": "blocker",
                        "code": "quota_shortage",
                        "message": f"Accepted ready papers {report.get('accepted_ready_count')} below target {report.get('target_count')}.",
                        "recommended_action": "Draw deterministic replacements from replacement_candidates or block scan.",
                    },
                    sort_keys=True,
                    separators=(",", ":"),
                )
                + "\n"
            )
    return {"summary_path": summary_path, "diagnostics_path": diagnostics_path}


def build_bounded_top_up_report(
    state: ValidationBatchState,
    *,
    target_count: int,
    candidate_inventory: dict[str, Any],
    max_candidates_to_consider: int,
) -> dict[str, Any]:
    """Plan deterministic bounded top-up replacements for an underfilled batch.

    This is a planning/reporting helper only: it does not fetch or convert
    sources. Candidate readiness is inferred from redacted inventory metadata.
    """
    quota = build_quota_fill_report(state, target_count=target_count, candidate_inventory=candidate_inventory)
    accepted_ready_count = int(quota["accepted_ready_count"])
    shortage_count = int(quota["shortage_count"])
    selected_ids = {paper.paper_id for paper in state.selected_papers}
    replacements: list[dict[str, Any]] = []
    rejected_candidates: list[dict[str, Any]] = []
    considered_count = 0
    for candidate in candidate_inventory.get("candidates", []):
        if considered_count >= max_candidates_to_consider or len(replacements) >= shortage_count:
            break
        if not isinstance(candidate, dict):
            continue
        paper_id = str(candidate.get("paper_id"))
        if paper_id in selected_ids:
            continue
        considered_count += 1
        record = _top_up_candidate_record(candidate)
        if record["ready_for_markdown_scan"]:
            replacements.append(record)
        else:
            rejected_candidates.append(record)
    final_accepted_ready_count = accepted_ready_count + len(replacements)
    remaining_shortage_count = max(target_count - final_accepted_ready_count, 0)
    scan_allowed = final_accepted_ready_count == target_count and remaining_shortage_count == 0
    return {
        "schema_version": "m009-bounded-top-up-summary.v1",
        "batch_id": state.batch_id,
        "target_count": target_count,
        "initial_accepted_ready_count": accepted_ready_count,
        "initial_shortage_count": shortage_count,
        "max_candidates_to_consider": max_candidates_to_consider,
        "considered_replacement_count": considered_count,
        "accepted_replacement_count": len(replacements),
        "rejected_replacement_count": len(rejected_candidates),
        "final_accepted_ready_count": final_accepted_ready_count,
        "remaining_shortage_count": remaining_shortage_count,
        "scan_allowed": scan_allowed,
        "blocker_count": 0 if scan_allowed else 1,
        "accepted_replacements": replacements,
        "rejected_candidates": rejected_candidates,
        **default_safety_flags(),
    }


def write_bounded_top_up_run(report: dict[str, Any], output_dir: str | Path, *, prefix: str = "top-up") -> dict[str, Path]:
    output = Path(output_dir)
    output.mkdir(parents=True, exist_ok=True)
    summary_path = output / f"{prefix}-summary.json"
    diagnostics_path = output / f"{prefix}-diagnostics.jsonl"
    summary_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    with diagnostics_path.open("w", encoding="utf-8") as handle:
        for record in report.get("rejected_candidates", []):
            handle.write(json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n")
        if int(report.get("remaining_shortage_count", 0)) > 0:
            handle.write(
                json.dumps(
                    {
                        "severity": "blocker",
                        "code": "bounded_top_up_shortage",
                        "message": f"Top-up could not fill target quota within {report.get('max_candidates_to_consider')} candidates.",
                        "recommended_action": "Increase bounded candidate pool, repair sources, or block scan explicitly.",
                    },
                    sort_keys=True,
                    separators=(",", ":"),
                )
                + "\n"
            )
    return {"summary_path": summary_path, "diagnostics_path": diagnostics_path}


def _top_up_candidate_record(candidate: dict[str, Any]) -> dict[str, Any]:
    availability = candidate.get("availability", {}) if isinstance(candidate.get("availability", {}), dict) else {}
    ready = bool(
        availability.get("ready_for_markdown_scan")
        or (availability.get("markdown_present") and availability.get("markdown_quality_accepted", True))
    )
    outcome = "accepted_replacement_ready" if ready else "rejected_replacement_not_source_ready"
    return {
        "paper_id": str(candidate.get("paper_id")),
        "ready_for_markdown_scan": ready,
        "outcome": outcome,
        "availability": availability,
        "risk_tags": list(candidate.get("risk_tags", [])),
        **default_safety_flags(),
    }


def _quota_fill_record(paper: SelectedPaper, readiness: SourceReadiness | None) -> dict[str, Any]:
    if readiness is None:
        outcome = "rejected_no_preflight"
        ready = False
        reason = "No source readiness record exists for this paper."
    elif readiness.ready_for_markdown_scan:
        outcome = "accepted_ready"
        ready = True
        reason = "Markdown is present and accepted for scan."
    else:
        outcome = "rejected_not_source_ready"
        ready = False
        reason = "Paper is not ready for Markdown scan."
    return {
        "paper_id": paper.paper_id,
        "rank": paper.rank,
        "selection_role": paper.selection_role,
        "ready_for_markdown_scan": ready,
        "outcome": outcome,
        "reason": reason,
        "risk_tags": list(paper.risk_tags),
        **default_safety_flags(),
    }


def _replacement_candidates(candidate_inventory: dict[str, Any] | None, selected_ids: set[str], *, limit: int) -> list[dict[str, Any]]:
    if not candidate_inventory or limit <= 0:
        return []
    replacements: list[dict[str, Any]] = []
    for candidate in candidate_inventory.get("candidates", []):
        if not isinstance(candidate, dict):
            continue
        paper_id = str(candidate.get("paper_id"))
        if paper_id in selected_ids:
            continue
        replacements.append(
            {
                "paper_id": paper_id,
                "availability": candidate.get("availability", {}),
                "risk_tags": candidate.get("risk_tags", []),
            }
        )
        if len(replacements) >= limit:
            break
    return replacements


def _paper_manifest_record(paper: SelectedPaper) -> dict[str, Any]:
    selection_role = "m005_baseline_overlap" if paper.selection_role == "baseline_overlap" else paper.selection_role
    source_paths = dict(paper.source_paths)
    source_paths.setdefault("research_workspace", str(Path("/root/.research/papers") / paper.paper_id))
    source_paths.setdefault("research_full_text_md", str(Path("/root/.research/papers") / paper.paper_id / "full_text.md"))
    source_paths.setdefault("cache_markdown", str(Path("/root/.arxiv_cache") / f"{paper.paper_id}.md"))
    source_paths.setdefault("cache_pdf", str(Path("/root/.arxiv_cache") / f"{paper.paper_id}.pdf"))
    return {
        "paper_id": paper.paper_id,
        "rank": paper.rank,
        "selection_role": selection_role,
        "risk_tags": list(paper.risk_tags),
        "source_paths": source_paths,
        "required_paths": list(dict.fromkeys(source_paths.values())),
    }


def _read_optional_json(path: str | Path | None) -> dict[str, Any] | None:
    if path is None:
        return None
    payload = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"expected JSON object at {path}")
    return payload


def _baseline_delta(current: dict[str, Any], baseline: dict[str, Any] | None) -> dict[str, Any]:
    if not baseline:
        return {"available": False}
    baseline_chunk_count = int(baseline.get("chunk_count", 0) or 0)
    current_chunk_count = int(current.get("chunk_count", 0) or 0)
    return {
        "available": True,
        "baseline_name": "M005/S03 structure-aware baseline",
        "baseline_paper_count": baseline.get("paper_count"),
        "current_paper_count": current.get("paper_count"),
        "baseline_chunk_count": baseline_chunk_count,
        "current_chunk_count": current_chunk_count,
        "chunk_count_delta": current_chunk_count - baseline_chunk_count,
        "baseline_import_eligible_chunk_count": int(baseline.get("import_eligible_chunk_count", 0) or 0),
        "current_import_eligible_chunk_count": int(current.get("import_eligible_chunk_count", 0) or 0),
    }


def _mixed_benchmark_context(current: dict[str, Any], benchmark: dict[str, Any] | None) -> dict[str, Any]:
    if not benchmark:
        return {"available": False}
    benchmark_aggregate = benchmark.get("aggregate", benchmark)
    benchmark_chunk_count = int(benchmark_aggregate.get("total_chunk_count", benchmark_aggregate.get("chunk_count", 0)) or 0)
    current_chunk_count = int(current.get("chunk_count", 0) or 0)
    return {
        "available": True,
        "baseline_name": "M005/S06 mixed benchmark context only",
        "benchmark_chunk_count": benchmark_chunk_count,
        "current_chunk_count": current_chunk_count,
        "chunk_count_delta": current_chunk_count - benchmark_chunk_count,
        "benchmark_import_eligible_chunk_count": int(
            benchmark_aggregate.get(
                "total_import_eligible_chunk_count",
                benchmark_aggregate.get("import_eligible_chunk_count", 0),
            )
            or 0
        ),
        "current_import_eligible_chunk_count": int(current.get("import_eligible_chunk_count", 0) or 0),
    }


def _share_delta(current_counts: dict[str, Any], baseline_counts: dict[str, Any]) -> dict[str, dict[str, float]]:
    current_total = sum(int(value) for value in current_counts.values())
    baseline_total = sum(int(value) for value in baseline_counts.values())
    keys = sorted(set(current_counts) | set(baseline_counts))
    return {
        str(key): {
            "baseline_share": round((int(baseline_counts.get(key, 0)) / baseline_total), 4) if baseline_total else 0.0,
            "current_share": round((int(current_counts.get(key, 0)) / current_total), 4) if current_total else 0.0,
            "delta": round(
                ((int(current_counts.get(key, 0)) / current_total) if current_total else 0.0)
                - ((int(baseline_counts.get(key, 0)) / baseline_total) if baseline_total else 0.0),
                4,
            ),
        }
        for key in keys
    }


def _loader_provenance(result: Any) -> dict[str, Any]:
    provenance = dict(result.provenance or {})
    provenance.update(
        {
            "outcome": result.outcome,
            "failure_reason": result.failure_reason,
            "selected_fallback": result.failure_reason,
            "warning_count": result.warning_count,
            "duration_ms": result.duration_ms,
        }
    )
    return sanitize_event_details(provenance)


def _first_candidate_path(*raw_paths: str | None) -> Path:
    for raw_path in raw_paths:
        if raw_path:
            return Path(raw_path)
    raise ValueError("at least one source candidate path is required")


def _first_existing_path(*raw_paths: str | None) -> Path | None:
    for raw_path in raw_paths:
        if not raw_path:
            continue
        path = Path(raw_path)
        if path.exists() and path.is_file():
            return path
    return None


def _markdown_quality_accepted(path: Path | None) -> bool:
    if path is None or not path.exists() or not path.is_file():
        return False
    return path.stat().st_size > 0
