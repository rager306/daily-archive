#!/usr/bin/env python3
"""Compare M057 Marker/Nougat packets with prior OpenDataLoader packets."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from statistics import mean
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MARKER_SUMMARY = (
    ROOT / "artifacts" / "m057-fd-marker" / "marker-extraction" / "summary.json"
)
DEFAULT_JSON_OUTPUT = ROOT / "artifacts" / "m057-fd-marker" / "marker-vs-opendataloader.json"
DEFAULT_MD_OUTPUT = ROOT / "artifacts" / "m057-fd-marker" / "marker-vs-opendataloader.md"

OPEN_DATALOADER_SOURCES: tuple[dict[str, str], ...] = (
    {
        "name": "m055-opendataloader-only",
        "path": "artifacts/m055-parser-benchmark/opendataloader-only",
    },
    {
        "name": "m055deep-opendataloader-20",
        "path": "artifacts/m055deep-parser-benchmark/opendataloader-20",
    },
    {
        "name": "m055deep-opendataloader-correctness",
        "path": "artifacts/m055deep-parser-benchmark/opendataloader-correctness",
    },
    {"name": "m056-wave-1", "path": "artifacts/m056-bfs-graph/wave-1/opendataloader"},
    {"name": "m056-wave-2", "path": "artifacts/m056-bfs-graph/wave-2/opendataloader"},
    {"name": "m056-wave-3", "path": "artifacts/m056-bfs-graph/wave-3/opendataloader"},
    {"name": "m056-wave-4", "path": "artifacts/m056-bfs-graph/wave-4/opendataloader"},
    {"name": "m056-wave-5", "path": "artifacts/m056-bfs-graph/wave-5/opendataloader"},
    {"name": "m056-wave-6", "path": "artifacts/m056-bfs-graph/wave-6/opendataloader"},
    {"name": "m056-missing-17", "path": "artifacts/m056-bfs-graph/missing-17-opendataloader"},
)

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def source_quality_default(source_dir: Path) -> float | None:
    summary_path = source_dir / "summary.json"
    if not summary_path.exists():
        return None
    summary = load_json(summary_path)
    metrics = summary.get("aggregate_correctness_metrics") or {}
    value = metrics.get("table_structure_quality_score")
    if isinstance(value, int | float):
        return float(value)
    return None


def packet_quality(packet: dict[str, Any], source_default: float | None) -> float:
    for key in (
        "table_structure_quality_avg",
        "table_quality_avg",
        "table_structure_quality_score",
    ):
        value = packet.get(key)
        if isinstance(value, int | float):
            return float(value)
    if source_default is not None and packet.get("status") == "success":
        return source_default
    if packet.get("status") == "success" and not packet.get("low_quality_source"):
        return 1.0
    return 0.0


def collect_opendataloader_packets() -> dict[str, dict[str, Any]]:
    best_by_id: dict[str, dict[str, Any]] = {}
    for source in OPEN_DATALOADER_SOURCES:
        source_dir = ROOT / source["path"]
        per_pdf_dir = source_dir / "per-pdf"
        if not per_pdf_dir.exists():
            continue
        default_quality = source_quality_default(source_dir)
        for path in sorted(per_pdf_dir.glob("*.json")):
            packet = load_json(path)
            arxiv_id = str(packet.get("arxiv_id") or path.stem)
            candidate = {
                "arxiv_id": arxiv_id,
                "source": source["name"],
                "status": packet.get("status"),
                "table_count": int(packet.get("table_count") or 0),
                "table_structure_quality_avg": round(packet_quality(packet, default_quality), 3),
                "low_quality_source": bool(packet.get("low_quality_source", False)),
            }
            current = best_by_id.get(arxiv_id)
            if current is None:
                best_by_id[arxiv_id] = candidate
                continue
            candidate_key = (
                candidate["table_structure_quality_avg"],
                candidate["table_count"],
                candidate["status"] == "success",
            )
            current_key = (
                current["table_structure_quality_avg"],
                current["table_count"],
                current["status"] == "success",
            )
            if candidate_key > current_key:
                best_by_id[arxiv_id] = candidate
    return best_by_id


def compare(marker_summary_path: Path = DEFAULT_MARKER_SUMMARY) -> dict[str, Any]:
    marker_summary = load_json(marker_summary_path)
    marker_packets = marker_summary.get("per_pdf", [])
    opendataloader = collect_opendataloader_packets()
    rows: list[dict[str, Any]] = []
    marker_better_count = 0
    compared_count = 0
    quality_deltas: list[float] = []

    for marker in marker_packets:
        arxiv_id = str(marker.get("arxiv_id"))
        baseline = opendataloader.get(arxiv_id)
        marker_quality = float(marker.get("table_structure_quality_avg") or 0.0)
        marker_table_count = int(marker.get("table_count") or 0)
        baseline_quality = (
            float(baseline.get("table_structure_quality_avg") or 0.0) if baseline else 0.0
        )
        baseline_table_count = int(baseline.get("table_count") or 0) if baseline else 0
        delta = round(marker_quality - baseline_quality, 3)
        quality_deltas.append(delta)
        if baseline is not None:
            compared_count += 1
            if marker_quality > baseline_quality:
                marker_better_count += 1
        rows.append(
            {
                "arxiv_id": arxiv_id,
                "marker_status": marker.get("status"),
                "marker_backend": marker.get("backend"),
                "marker_table_count": marker_table_count,
                "marker_table_structure_quality_avg": marker_quality,
                "opendataloader_source": baseline.get("source") if baseline else None,
                "opendataloader_status": baseline.get("status") if baseline else "missing",
                "opendataloader_table_count": baseline_table_count,
                "opendataloader_table_structure_quality_avg": baseline_quality,
                "table_count_delta": marker_table_count - baseline_table_count,
                "quality_delta": delta,
                "marker_better": baseline is not None and marker_quality > baseline_quality,
            }
        )

    marker_better_pct = (
        round((marker_better_count / compared_count) * 100, 3) if compared_count else 0.0
    )
    return {
        "schema_version": "m057-fd-marker.marker-vs-opendataloader.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "marker_summary_path": str(
            marker_summary_path.relative_to(ROOT)
            if marker_summary_path.is_absolute()
            else marker_summary_path
        ),
        "opendataloader_sources": list(OPEN_DATALOADER_SOURCES),
        "summary": {
            "total_marker_pdfs": len(marker_packets),
            "opendataloader_matched_pdfs": compared_count,
            "marker_better_count": marker_better_count,
            "marker_better_percent": marker_better_pct,
            "average_quality_delta": round(mean(quality_deltas), 3) if quality_deltas else 0.0,
            "marker_average_quality": round(
                mean(float(row["marker_table_structure_quality_avg"]) for row in rows), 3
            )
            if rows
            else 0.0,
            "opendataloader_average_quality": round(
                mean(float(row["opendataloader_table_structure_quality_avg"]) for row in rows), 3
            )
            if rows
            else 0.0,
            "marker_total_table_count": sum(int(row["marker_table_count"]) for row in rows),
            "opendataloader_total_table_count": sum(
                int(row["opendataloader_table_count"]) for row in rows
            ),
        },
        "per_pdf": rows,
    }


def render_markdown(report: dict[str, Any]) -> str:
    summary = report["summary"]
    lines = [
        "# M057 S01 Marker vs OpenDataLoader",
        "",
        "This comparison is diagnostic only; graph import is not authorized and production import is not authorized.",
        "",
        "## Aggregate",
        "",
        f"- Marker PDFs: {summary['total_marker_pdfs']}",
        f"- OpenDataLoader matched PDFs: {summary['opendataloader_matched_pdfs']}",
        f"- Marker better: {summary['marker_better_count']} ({summary['marker_better_percent']}%)",
        f"- Average quality delta: {summary['average_quality_delta']}",
        f"- Marker average quality: {summary['marker_average_quality']}",
        f"- OpenDataLoader average quality: {summary['opendataloader_average_quality']}",
        f"- Marker total tables: {summary['marker_total_table_count']}",
        f"- OpenDataLoader total tables: {summary['opendataloader_total_table_count']}",
        "",
        "## Per-PDF Summary",
        "",
        "| arxiv_id | Marker status | Marker q | OpenDataLoader source | OpenDataLoader q | Δ quality | Marker > OpenDataLoader |",
        "|---|---:|---:|---|---:|---:|---:|",
    ]
    for row in sorted(report["per_pdf"], key=lambda item: item["arxiv_id"]):
        lines.append(
            "| {arxiv_id} | {marker_status} | {marker_table_structure_quality_avg:.3f} | {opendataloader_source} | {opendataloader_table_structure_quality_avg:.3f} | {quality_delta:.3f} | {marker_better} |".format(
                **{**row, "opendataloader_source": row["opendataloader_source"] or "missing"}
            )
        )
    lines.append("")
    return "\n".join(lines)


def write_outputs(
    report: dict[str, Any],
    json_output: Path = DEFAULT_JSON_OUTPUT,
    md_output: Path = DEFAULT_MD_OUTPUT,
) -> None:
    json_output.parent.mkdir(parents=True, exist_ok=True)
    json_output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    md_output.write_text(render_markdown(report), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--marker-summary", type=Path, default=DEFAULT_MARKER_SUMMARY)
    parser.add_argument("--json-output", type=Path, default=DEFAULT_JSON_OUTPUT)
    parser.add_argument("--md-output", type=Path, default=DEFAULT_MD_OUTPUT)
    args = parser.parse_args()
    report = compare(args.marker_summary)
    write_outputs(report, args.json_output, args.md_output)
    print(json.dumps(report["summary"], sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
