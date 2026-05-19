"""Thirty-paper deviation scan helpers for M006.

This module runs a Markdown-based structure-aware scan over a selected corpus and
emits redacted metrics only. It intentionally does not serialize raw Markdown,
chunk text, embeddings, vectors, or production KG write signals.
"""

from __future__ import annotations

import json
import statistics
from pathlib import Path
from typing import Any

from arxiv_archive.chunk_import_contract import validate_import_ready_package, validation_to_dict
from arxiv_archive.structure_aware_chunking import build_structure_aware_package_for_paper

SAFETY_FLAGS: dict[str, bool] = {
    "raw_text_included": False,
    "chunk_text_included": False,
    "raw_binary_included": False,
    "base64_included": False,
    "embeddings_included": False,
    "vectors_included": False,
    "secrets_included": False,
    "optimizer_traces_included": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
}


def build_thirty_paper_deviation_scan(
    *,
    manifest_path: str | Path,
    source_acquisition_summary_path: str | Path | None = None,
    baseline_summary_path: str | Path | None = None,
    run_id: str = "m006-s03-thirty-paper-deviation",
) -> dict[str, Any]:
    """Build redacted 30-paper deviation summary and per-paper diagnostics."""
    manifest = _read_json_object(Path(manifest_path))
    source_summary = _read_optional_json_object(source_acquisition_summary_path)
    baseline_summary = _read_optional_json_object(baseline_summary_path)
    records: list[dict[str, Any]] = []
    for paper in manifest.get("papers", []):
        if not isinstance(paper, dict):
            continue
        normalized = _normalize_paper_record(paper)
        source_path = _selected_source_path(normalized)
        package = build_structure_aware_package_for_paper(normalized, run_id=run_id).to_contract()
        validation = validation_to_dict(validate_import_ready_package(package))
        records.append(_paper_diagnostic(package=package, validation=validation, manifest_paper=paper, source_path=source_path))
    return {
        "schema_version": "m006-thirty-paper-deviation-summary.v1",
        "milestone": "M006-638rza",
        "slice": "S03",
        "run_id": run_id,
        "paper_count": len(records),
        "m005_overlap_count": manifest.get("m005_overlap_count"),
        "expansion_count": manifest.get("expansion_count"),
        "source_readiness": _source_readiness(source_summary),
        "aggregate": _aggregate(records),
        "baseline_comparison": _baseline_comparison(records=records, baseline_summary=baseline_summary),
        "outliers": _outliers(records),
        "records": records,
        **SAFETY_FLAGS,
    }


def write_thirty_paper_deviation_run(scan: dict[str, Any], output_dir: str | Path) -> dict[str, Path]:
    """Write bounded summary JSON and per-paper diagnostics JSONL."""
    output = Path(output_dir)
    output.mkdir(parents=True, exist_ok=True)
    summary = {key: value for key, value in scan.items() if key != "records"}
    summary_path = output / "thirty-paper-deviation-summary.json"
    diagnostics_path = output / "thirty-paper-deviation-diagnostics.jsonl"
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    with diagnostics_path.open("w", encoding="utf-8") as handle:
        for record in scan.get("records", []):
            handle.write(json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n")
    return {"summary_path": summary_path, "diagnostics_path": diagnostics_path}


def _paper_diagnostic(
    *,
    package: dict[str, Any],
    validation: dict[str, Any],
    manifest_paper: dict[str, Any],
    source_path: Path | None,
) -> dict[str, Any]:
    diagnostics = package.get("diagnostics", {})
    markdown_char_count = source_path.stat().st_size if source_path is not None and source_path.exists() and source_path.is_file() else 0
    chunk_count = len(package.get("chunks", []))
    return {
        "schema_version": "m006-thirty-paper-deviation-diagnostic.v1",
        "paper_id": package.get("paper_id"),
        "rank": manifest_paper.get("rank"),
        "selection_role": manifest_paper.get("selection_role"),
        "risk_tags": list(manifest_paper.get("risk_tags", [])),
        "source_artifact": str(package.get("source_artifact")),
        "markdown_byte_size": markdown_char_count,
        "valid_package": validation.get("valid_package"),
        "import_ready": validation.get("import_ready"),
        "import_eligible_chunk_count": validation.get("import_eligible_chunk_count"),
        "refused_chunk_count": validation.get("refused_chunk_count"),
        "element_count": len(package.get("elements", [])),
        "chunk_count": chunk_count,
        "annotation_count": len(package.get("annotations", [])),
        "chunks_per_10k_bytes": round((chunk_count / markdown_char_count) * 10000, 4) if markdown_char_count else 0.0,
        "counts_by_state": diagnostics.get("counts_by_state", {}),
        "counts_by_route": diagnostics.get("counts_by_route", {}),
        "counts_by_chunk_type": diagnostics.get("counts_by_chunk_type", {}),
        "refusal_counts": diagnostics.get("refusal_counts", {}),
        "annotation_counts_by_type": diagnostics.get("annotation_counts_by_type", {}),
        "source_span_coverage": diagnostics.get("source_span_coverage"),
        "parent_reference_resolution_rate": diagnostics.get("parent_reference_resolution_rate"),
        "outlier_flags": [],
        **SAFETY_FLAGS,
    }


def _aggregate(records: list[dict[str, Any]]) -> dict[str, Any]:
    chunk_counts = [int(record["chunk_count"]) for record in records]
    byte_sizes = [int(record["markdown_byte_size"]) for record in records]
    return {
        "paper_count": len(records),
        "valid_package_count": sum(1 for record in records if record["valid_package"]),
        "import_ready_count": sum(1 for record in records if record["import_ready"]),
        "import_eligible_chunk_count": sum(int(record["import_eligible_chunk_count"] or 0) for record in records),
        "refused_chunk_count": sum(int(record["refused_chunk_count"] or 0) for record in records),
        "element_count": sum(int(record["element_count"]) for record in records),
        "chunk_count": sum(chunk_counts),
        "annotation_count": sum(int(record["annotation_count"]) for record in records),
        "markdown_byte_size_total": sum(byte_sizes),
        "chunk_count_min": min(chunk_counts) if chunk_counts else 0,
        "chunk_count_max": max(chunk_counts) if chunk_counts else 0,
        "chunk_count_mean": round(statistics.mean(chunk_counts), 4) if chunk_counts else 0.0,
        "counts_by_state": _merge_record_counts(records, "counts_by_state"),
        "counts_by_route": _merge_record_counts(records, "counts_by_route"),
        "counts_by_chunk_type": _merge_record_counts(records, "counts_by_chunk_type"),
        "refusal_counts": _merge_record_counts(records, "refusal_counts"),
        "selection_role_counts": _selection_role_counts(records),
    }


def _baseline_comparison(*, records: list[dict[str, Any]], baseline_summary: dict[str, Any] | None) -> dict[str, Any]:
    current = _aggregate(records)
    if not baseline_summary:
        return {"baseline_available": False, "current_chunk_count": current["chunk_count"]}
    baseline_aggregate = baseline_summary.get("aggregate", baseline_summary)
    baseline_chunk_count = int(baseline_aggregate.get("total_chunk_count", baseline_aggregate.get("chunk_count", 0)) or 0)
    baseline_paper_count = int(baseline_summary.get("paper_count", baseline_aggregate.get("paper_count", 0)) or 0)
    return {
        "baseline_available": True,
        "baseline_paper_count": baseline_paper_count,
        "current_paper_count": current["paper_count"],
        "baseline_chunk_count": baseline_chunk_count,
        "current_chunk_count": current["chunk_count"],
        "chunk_count_delta": current["chunk_count"] - baseline_chunk_count,
        "baseline_import_eligible_chunk_count": int(
            baseline_aggregate.get("total_import_eligible_chunk_count", baseline_aggregate.get("import_eligible_chunk_count", 0)) or 0
        ),
        "current_import_eligible_chunk_count": current["import_eligible_chunk_count"],
    }


def _outliers(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    if not records:
        return []
    chunk_counts = sorted(int(record["chunk_count"]) for record in records)
    median = statistics.median(chunk_counts)
    high_threshold = max(median * 2, median + 25)
    outliers: list[dict[str, Any]] = []
    for record in records:
        flags: list[str] = []
        chunk_count = int(record["chunk_count"])
        if chunk_count == 0:
            flags.append("zero_chunks")
        if chunk_count >= high_threshold and chunk_count > 0:
            flags.append("high_chunk_count")
        if int(record.get("import_eligible_chunk_count") or 0) > 0:
            flags.append("unexpected_import_eligible_chunks")
        if record.get("counts_by_route", {}).get("table_extraction", 0) >= 10:
            flags.append("table_heavy")
        if record.get("counts_by_route", {}).get("claim_extraction", 0) >= 25:
            flags.append("claim_candidate_heavy")
        if flags:
            outliers.append({"paper_id": record["paper_id"], "flags": flags, "chunk_count": chunk_count})
            record["outlier_flags"] = flags
    return outliers


def _normalize_paper_record(paper: dict[str, Any]) -> dict[str, Any]:
    paper_id = str(paper["paper_id"])
    paths = paper.get("required_paths") if isinstance(paper.get("required_paths"), list) else []
    source_paths = paper.get("source_paths") if isinstance(paper.get("source_paths"), dict) else {}
    research_workspace = source_paths.get("research_workspace")
    full_text = source_paths.get("research_full_text_md")
    normalized_paths = [str(path) for path in paths]
    if full_text:
        normalized_paths.append(str(full_text))
    if research_workspace:
        normalized_paths.append(str(research_workspace))
    fallback_workspace = Path("/root/.research/papers") / paper_id
    normalized_paths.append(str(fallback_workspace))
    return {
        "paper_id": paper_id,
        "title": paper.get("title"),
        "categories": paper.get("categories", []),
        "required_paths": list(dict.fromkeys(normalized_paths)),
        "source_artifacts": paper.get("source_artifacts", []),
    }


def _selected_source_path(paper: dict[str, Any]) -> Path | None:
    for raw_path in paper.get("required_paths", []):
        path = Path(str(raw_path))
        if path.name == "full_text.md" and path.exists():
            return path
        full_text_path = path / "full_text.md"
        if path.is_dir() and full_text_path.exists():
            return full_text_path
    return None


def _source_readiness(source_summary: dict[str, Any] | None) -> dict[str, Any]:
    if not source_summary:
        return {"available": False}
    return {
        "available": True,
        "paper_count": source_summary.get("paper_count"),
        "ready_for_markdown_scan_count": source_summary.get("ready_for_markdown_scan_count"),
        "still_missing_markdown_count": source_summary.get("still_missing_markdown_count"),
        "available_pdf_count": source_summary.get("available_pdf_count"),
    }


def _selection_role_counts(records: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for record in records:
        role = str(record.get("selection_role") or "unknown")
        counts[role] = counts.get(role, 0) + 1
    return dict(sorted(counts.items()))


def _merge_record_counts(records: list[dict[str, Any]], key: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for record in records:
        source = record.get(key, {})
        if not isinstance(source, dict):
            continue
        for count_key, value in source.items():
            counts[str(count_key)] = counts.get(str(count_key), 0) + int(value)
    return dict(sorted(counts.items()))


def _read_json_object(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object at {path}")
    return value


def _read_optional_json_object(path: str | Path | None) -> dict[str, Any] | None:
    if path is None:
        return None
    return _read_json_object(Path(path))
