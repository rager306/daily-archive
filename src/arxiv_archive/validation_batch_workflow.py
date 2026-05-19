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

from arxiv_archive.validation_batch_state import (
    ScanArtifactPaths,
    SelectedPaper,
    SourceReadiness,
    ValidationBatchState,
    batch_state_to_dict,
    build_batch_diagnostics,
    default_safety_flags,
    write_batch_state,
)

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


def source_readiness_for_paper(paper: SelectedPaper, *, fallback_root: str | Path = "/root/.research/papers", cache_root: str | Path = "/root/.arxiv_cache") -> SourceReadiness:
    fallback_root_path = Path(fallback_root)
    cache_root_path = Path(cache_root)
    markdown_path = _first_existing_path(
        paper.source_paths.get("research_full_text_md"),
        paper.source_paths.get("cache_markdown"),
        str(fallback_root_path / paper.paper_id / "full_text.md"),
        str(cache_root_path / f"{paper.paper_id}.md"),
    )
    pdf_path = _first_existing_path(
        paper.source_paths.get("cache_pdf"),
        paper.source_paths.get("research_pdf"),
        str(cache_root_path / f"{paper.paper_id}.pdf"),
        str(fallback_root_path / paper.paper_id / "paper.pdf"),
    )
    markdown_present = markdown_path is not None
    markdown_quality_accepted = _markdown_quality_accepted(markdown_path)
    pdf_present = pdf_path is not None
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
