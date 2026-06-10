#!/usr/bin/env python3
"""OpenDataLoader correctness validation for M055deep S02.

Reads the M055 OpenDataLoader-only markdown artifacts and emits deterministic
correctness diagnostics for table structure, figure/table captions, and chart-
like extracted images. This script is read-only with respect to parser inputs:
it writes diagnostic JSON only, never writes graph data, never attempts a
production import, and keeps all five safety defaults false.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055deep-parser-benchmark.opendataloader-correctness.v1"
DEFAULT_CORPUS_MANIFEST = Path("artifacts/m055-parser-benchmark/corpus-manifest.json")
DEFAULT_OPENDATALOADER_DIR = Path("artifacts/m055-parser-benchmark/opendataloader-only")
DEFAULT_OUTPUT_DIR = Path("artifacts/m055deep-parser-benchmark/opendataloader-correctness")
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}
CAPTION_RE = re.compile(
    r"\b(?P<label>Figure|Fig\.|Table)\s*(?P<number>[0-9]+|[IVXLC]+)\s*(?P<marker>[:.])\s*(?P<caption>.+)",
    re.IGNORECASE,
)
MARKDOWN_IMAGE_RE = re.compile(r"!\[[^\]]*\]\((?P<target>[^)]+)\)")
ALIGNMENT_CELL_RE = re.compile(r"^:?-{3,}:?$")
IMAGE_SUFFIXES = {".png", ".jpg", ".jpeg", ".gif", ".svg"}


def _safety_defaults() -> dict[str, bool]:
    return dict(SAFETY_DEFAULTS)


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8")


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"Invalid JSON packet: {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValueError(f"Expected JSON object: {path}")
    return payload


def _is_pipe_row(line: str) -> bool:
    stripped = line.strip()
    return stripped.startswith("|") and stripped.count("|") >= 2


def _split_markdown_row(line: str) -> list[str]:
    stripped = line.strip()
    if stripped.startswith("|"):
        stripped = stripped[1:]
    if stripped.endswith("|"):
        stripped = stripped[:-1]
    return [cell.strip() for cell in stripped.split("|")]


def _is_alignment_separator(line: str) -> bool:
    if not _is_pipe_row(line):
        return False
    cells = _split_markdown_row(line)
    return bool(cells) and all(cell.strip() and ALIGNMENT_CELL_RE.match(cell.strip()) for cell in cells)


def _normalized_rows(rows: list[list[str]], cols: int) -> list[list[str]]:
    normalized = []
    for row in rows:
        padded = list(row[:cols])
        if len(padded) < cols:
            padded.extend([""] * (cols - len(padded)))
        normalized.append(padded)
    return normalized


def _parse_markdown_tables(markdown_text: str) -> list[dict[str, Any]]:
    """Parse GitHub-style markdown tables with alignment separators."""

    lines = markdown_text.splitlines()
    tables: list[dict[str, Any]] = []
    index = 1
    while index < len(lines):
        separator_line = lines[index]
        header_line = lines[index - 1]
        if not (_is_alignment_separator(separator_line) and _is_pipe_row(header_line)):
            index += 1
            continue

        header_cells = _split_markdown_row(header_line)
        separator_cells = _split_markdown_row(separator_line)
        body_rows: list[list[str]] = []
        cursor = index + 1
        while cursor < len(lines) and _is_pipe_row(lines[cursor]) and not _is_alignment_separator(lines[cursor]):
            body_rows.append(_split_markdown_row(lines[cursor]))
            cursor += 1

        cols = max([len(header_cells), len(separator_cells), *[len(row) for row in body_rows]] or [0])
        normalized_headers = _normalized_rows([header_cells], cols)[0] if cols else []
        normalized_body = _normalized_rows(body_rows, cols)
        tables.append(
            {
                "table_id": f"T{len(tables) + 1:03d}",
                "line_number": index,
                "rows": 1 + len(normalized_body),
                "cols": cols,
                "headers": normalized_headers,
                "body_rows": normalized_body,
                "has_alignment_separator": True,
            }
        )
        index = max(cursor, index + 1)
    return tables


def _extract_figure_captions(markdown_text: str) -> list[dict[str, Any]]:
    """Extract Figure/Fig./Table captions with source line numbers."""

    captions: list[dict[str, Any]] = []
    for line_number, line in enumerate(markdown_text.splitlines(), start=1):
        for match in CAPTION_RE.finditer(line):
            label = match.group("label")
            number = match.group("number")
            normalized_label = "Figure" if label.lower().startswith("fig") else "Table"
            caption_text = match.group("caption").strip()
            captions.append(
                {
                    "figure_id": f"{normalized_label} {number}",
                    "caption_type": normalized_label.lower(),
                    "caption_text": caption_text,
                    "line_number": line_number,
                }
            )
    return captions


def _svg_dimensions(data: bytes) -> dict[str, int | None]:
    text = data[:4096].decode("utf-8", errors="ignore")
    width_match = re.search(r'\bwidth=["\'](?P<value>\d+(?:\.\d+)?)', text)
    height_match = re.search(r'\bheight=["\'](?P<value>\d+(?:\.\d+)?)', text)
    if width_match and height_match:
        return {"width": int(float(width_match.group("value"))), "height": int(float(height_match.group("value")))}
    viewbox_match = re.search(
        r'\bviewBox=["\']\s*[-\d.]+\s+[-\d.]+\s+(?P<width>\d+(?:\.\d+)?)\s+(?P<height>\d+(?:\.\d+)?)',
        text,
    )
    if viewbox_match:
        return {"width": int(float(viewbox_match.group("width"))), "height": int(float(viewbox_match.group("height")))}
    return {"width": None, "height": None}


def _png_dimensions(data: bytes) -> dict[str, int | None]:
    if len(data) >= 24 and data.startswith(b"\x89PNG\r\n\x1a\n"):
        return {"width": int.from_bytes(data[16:20], "big"), "height": int.from_bytes(data[20:24], "big")}
    return {"width": None, "height": None}


def _gif_dimensions(data: bytes) -> dict[str, int | None]:
    if len(data) >= 10 and (data.startswith(b"GIF87a") or data.startswith(b"GIF89a")):
        return {"width": int.from_bytes(data[6:8], "little"), "height": int.from_bytes(data[8:10], "little")}
    return {"width": None, "height": None}


def _jpeg_dimensions(data: bytes) -> dict[str, int | None]:
    if not data.startswith(b"\xff\xd8"):
        return {"width": None, "height": None}
    cursor = 2
    while cursor + 9 < len(data):
        if data[cursor] != 0xFF:
            cursor += 1
            continue
        marker = data[cursor + 1]
        cursor += 2
        if marker in {0xD8, 0xD9}:
            continue
        if cursor + 2 > len(data):
            break
        segment_length = int.from_bytes(data[cursor : cursor + 2], "big")
        if segment_length < 2 or cursor + segment_length > len(data):
            break
        if marker in {0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF}:
            return {
                "width": int.from_bytes(data[cursor + 5 : cursor + 7], "big"),
                "height": int.from_bytes(data[cursor + 3 : cursor + 5], "big"),
            }
        cursor += segment_length
    return {"width": None, "height": None}


def _image_dimensions(path: Path, data: bytes) -> dict[str, int | None]:
    lower_suffix = path.suffix.lower()
    stripped = data.lstrip()[:128].lower()
    if lower_suffix == ".svg" or stripped.startswith(b"<svg") or stripped.startswith(b"<?xml"):
        return _svg_dimensions(data)
    if lower_suffix == ".png":
        return _png_dimensions(data)
    if lower_suffix == ".gif":
        return _gif_dimensions(data)
    if lower_suffix in {".jpg", ".jpeg"}:
        return _jpeg_dimensions(data)
    return {"width": None, "height": None}


def _detect_charts(image_files_dir: Path) -> list[dict[str, Any]]:
    """Detect chart-like extracted images with deterministic lightweight heuristics."""

    if not image_files_dir.exists() or not image_files_dir.is_dir():
        return []

    charts: list[dict[str, Any]] = []
    for image_path in sorted(path for path in image_files_dir.rglob("*") if path.suffix.lower() in IMAGE_SUFFIXES):
        data = image_path.read_bytes()
        size_bytes = len(data)
        stripped = data.lstrip()[:128].lower()
        dimensions = _image_dimensions(image_path, data)
        width = dimensions.get("width")
        height = dimensions.get("height")
        aspect_ratio = (width / height) if width and height else None

        chart_type = ""
        confidence = 0.0
        if image_path.suffix.lower() == ".svg" or stripped.startswith(b"<svg") or stripped.startswith(b"<?xml"):
            chart_type = "svg"
            confidence = 0.95
        elif aspect_ratio is not None and 0.5 <= aspect_ratio <= 2.0 and size_bytes > 5_000:
            chart_type = "matplotlib_like_raster"
            confidence = 0.70
        elif width and height and width * height >= 250_000 and size_bytes > 5_000:
            chart_type = "high_pixel_density"
            confidence = 0.55

        if chart_type:
            charts.append(
                {
                    "image_path": str(image_path),
                    "chart_type": chart_type,
                    "confidence": confidence,
                    "size_bytes": size_bytes,
                    "dimensions": dimensions,
                }
            )
    return charts


def _distribution(values: list[str]) -> dict[str, int]:
    distribution: dict[str, int] = {}
    for value in values:
        distribution[value] = distribution.get(value, 0) + 1
    return dict(sorted(distribution.items()))


def _average(values: list[float]) -> float:
    return round(sum(values) / len(values), 3) if values else 0.0


def _markdown_path(opendataloader_dir: Path, packet: dict[str, Any]) -> Path:
    raw_path = packet.get("markdown_path")
    if not isinstance(raw_path, str) or not raw_path:
        raise ValueError("OpenDataLoader packet is missing markdown_path")
    path = Path(raw_path)
    return path if path.is_absolute() else opendataloader_dir / path


def _image_dir_candidates(opendataloader_dir: Path, markdown_path: Path, markdown_text: str, arxiv_id: str) -> list[Path]:
    candidates = [markdown_path.parent / f"{markdown_path.stem}_images", opendataloader_dir / f"{arxiv_id}_images"]
    for match in MARKDOWN_IMAGE_RE.finditer(markdown_text):
        target = match.group("target").strip("<>")
        target_path = Path(target)
        if target_path.parent != Path("."):
            candidates.append(markdown_path.parent / target_path.parent)
    unique: list[Path] = []
    for candidate in candidates:
        if candidate not in unique:
            unique.append(candidate)
    return unique


def _extract_correctness_metrics(pdf_dir: Path, opendataloader_packet: dict[str, Any]) -> dict[str, Any]:
    """Extract table, caption, and chart correctness metrics for one packet."""

    opendataloader_dir = Path(pdf_dir)
    markdown_path = _markdown_path(opendataloader_dir, opendataloader_packet)
    markdown_text = markdown_path.read_text(encoding="utf-8")
    arxiv_id = str(opendataloader_packet.get("arxiv_id") or markdown_path.stem)

    tables = _parse_markdown_tables(markdown_text)
    captions = _extract_figure_captions(markdown_text)
    figure_captions = [caption for caption in captions if caption.get("caption_type") == "figure"]
    table_captions = [caption for caption in captions if caption.get("caption_type") == "table"]
    markdown_images = MARKDOWN_IMAGE_RE.findall(markdown_text)

    chart_records: list[dict[str, Any]] = []
    for image_dir in _image_dir_candidates(opendataloader_dir, markdown_path, markdown_text, arxiv_id):
        chart_records.extend(_detect_charts(image_dir))
    deduped_charts = {record["image_path"]: record for record in chart_records}
    charts = [deduped_charts[key] for key in sorted(deduped_charts)]

    row_counts = [int(table["rows"]) for table in tables]
    col_counts = [int(table["cols"]) for table in tables]
    structured_tables = [
        table
        for table in tables
        if table.get("has_alignment_separator") and int(table.get("rows", 0)) >= 2 and int(table.get("cols", 0)) >= 1
    ]
    figures_total = max(int(opendataloader_packet.get("image_count") or 0), len(markdown_images), len(figure_captions))
    figures_with_caption = min(len(figure_captions), figures_total) if figures_total else 0
    tables_with_caption = min(len(table_captions), len(tables)) if tables else 0

    return {
        "tables_total": len(tables),
        "tables_with_caption": tables_with_caption,
        "tables_avg_rows": _average([float(value) for value in row_counts]),
        "tables_avg_cols": _average([float(value) for value in col_counts]),
        "tables_max_rows": max(row_counts) if row_counts else 0,
        "tables_max_cols": max(col_counts) if col_counts else 0,
        "figures_total": figures_total,
        "figures_with_caption": figures_with_caption,
        "charts_detected": len(charts),
        "chart_types_distribution": _distribution([str(chart["chart_type"]) for chart in charts]),
        "image_caption_rate": round(figures_with_caption / figures_total, 3) if figures_total else 0.0,
        "table_structure_quality_score": round(len(structured_tables) / len(tables), 3) if tables else 0.0,
        "tables": tables,
        "captions": captions,
        "charts": charts,
    }


def _error_packet(arxiv_id: str, source_packet_path: Path, exc: Exception) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "arxiv_id": arxiv_id,
        "status": "error",
        "source_packet_path": str(source_packet_path),
        "safety_defaults": _safety_defaults(),
        "diagnostic": {"type": exc.__class__.__name__, "message": str(exc)},
        "tables_total": 0,
        "tables_with_caption": 0,
        "avg_rows_per_table": 0.0,
        "avg_cols_per_table": 0.0,
        "figures_total": 0,
        "figures_with_caption": 0,
        "charts_detected": 0,
        "chart_types": {},
    }


def _success_packet(manifest_entry: dict[str, Any], source_packet_path: Path, packet: dict[str, Any], metrics: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "article_key": manifest_entry.get("article_key"),
        "arxiv_id": manifest_entry.get("arxiv_id"),
        "category": manifest_entry.get("category"),
        "pdf_path": manifest_entry.get("path"),
        "source_packet_path": str(source_packet_path),
        "source_schema_version": packet.get("schema_version"),
        "status": "success",
        "safety_defaults": _safety_defaults(),
        "tables_total": metrics["tables_total"],
        "tables_with_caption": metrics["tables_with_caption"],
        "avg_rows_per_table": metrics["tables_avg_rows"],
        "avg_cols_per_table": metrics["tables_avg_cols"],
        "tables_max_rows": metrics["tables_max_rows"],
        "tables_max_cols": metrics["tables_max_cols"],
        "figures_total": metrics["figures_total"],
        "figures_with_caption": metrics["figures_with_caption"],
        "charts_detected": metrics["charts_detected"],
        "chart_types": metrics["chart_types_distribution"],
        "image_caption_rate": metrics["image_caption_rate"],
        "table_structure_quality_score": metrics["table_structure_quality_score"],
        "correctness_metrics": metrics,
    }


def _aggregate(per_pdf_packets: list[dict[str, Any]]) -> dict[str, Any]:
    successes = [packet for packet in per_pdf_packets if packet.get("status") == "success"]
    chart_types: dict[str, int] = {}
    for packet in successes:
        for chart_type, count in dict(packet.get("chart_types") or {}).items():
            chart_types[chart_type] = chart_types.get(chart_type, 0) + int(count)

    total_tables = sum(int(packet.get("tables_total") or 0) for packet in successes)
    total_figures = sum(int(packet.get("figures_total") or 0) for packet in successes)
    total_captioned_figures = sum(int(packet.get("figures_with_caption") or 0) for packet in successes)
    return {
        "tables_total": total_tables,
        "tables_with_caption": sum(int(packet.get("tables_with_caption") or 0) for packet in successes),
        "avg_rows_per_table": _average([float(packet.get("avg_rows_per_table") or 0.0) for packet in successes]),
        "avg_cols_per_table": _average([float(packet.get("avg_cols_per_table") or 0.0) for packet in successes]),
        "tables_max_rows": max([int(packet.get("tables_max_rows") or 0) for packet in successes] or [0]),
        "tables_max_cols": max([int(packet.get("tables_max_cols") or 0) for packet in successes] or [0]),
        "figures_total": total_figures,
        "figures_with_caption": total_captioned_figures,
        "charts_detected": sum(int(packet.get("charts_detected") or 0) for packet in successes),
        "chart_types_distribution": dict(sorted(chart_types.items())),
        "image_caption_rate": round(total_captioned_figures / total_figures, 3) if total_figures else 0.0,
        "table_structure_quality_score": _average(
            [float(packet.get("table_structure_quality_score") or 0.0) for packet in successes]
        ),
    }


def probe_opendataloader_correctness(corpus_manifest_path: Path, opendataloader_dir: Path, output_dir: Path) -> dict[str, Any]:
    manifest = _load_json(Path(corpus_manifest_path))
    manifest_pdfs = manifest.get("pdfs")
    if not isinstance(manifest_pdfs, list):
        raise ValueError(f"Manifest missing pdfs list: {corpus_manifest_path}")

    output_dir = Path(output_dir)
    per_pdf_dir = output_dir / "per-pdf"
    per_pdf_packets: list[dict[str, Any]] = []
    per_pdf_statuses: dict[str, str] = {}

    for entry in manifest_pdfs:
        if not isinstance(entry, dict):
            continue
        arxiv_id = str(entry.get("arxiv_id") or entry.get("article_key") or "unknown")
        source_packet_path = Path(opendataloader_dir) / "per-pdf" / f"{arxiv_id}.json"
        try:
            source_packet = _load_json(source_packet_path)
            metrics = _extract_correctness_metrics(Path(opendataloader_dir), source_packet)
            packet = _success_packet(entry, source_packet_path, source_packet, metrics)
        except Exception as exc:  # fail-closed typed diagnostic packet
            packet = _error_packet(arxiv_id, source_packet_path, exc)
        per_pdf_statuses[arxiv_id] = str(packet["status"])
        per_pdf_packets.append(packet)
        _write_json(per_pdf_dir / f"{arxiv_id}.json", packet)

    aggregate = _aggregate(per_pdf_packets)
    summary = {
        "schema_version": SCHEMA_VERSION,
        "corpus_manifest_path": str(corpus_manifest_path),
        "opendataloader_dir": str(opendataloader_dir),
        "output_dir": str(output_dir),
        "total_pdfs": len(manifest_pdfs),
        "success_count": sum(1 for packet in per_pdf_packets if packet.get("status") == "success"),
        "error_count": sum(1 for packet in per_pdf_packets if packet.get("status") != "success"),
        "per_pdf_statuses": dict(sorted(per_pdf_statuses.items())),
        "aggregate_correctness_metrics": aggregate,
        "safety_defaults": _safety_defaults(),
    }
    _write_json(output_dir / "summary.json", summary)
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-manifest", type=Path, default=DEFAULT_CORPUS_MANIFEST)
    parser.add_argument("--opendataloader-dir", type=Path, default=DEFAULT_OPENDATALOADER_DIR)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    summary = probe_opendataloader_correctness(args.corpus_manifest, args.opendataloader_dir, args.output_dir)
    print(json.dumps(summary, indent=2, sort_keys=True, ensure_ascii=False))
    return 0 if summary.get("error_count") == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
