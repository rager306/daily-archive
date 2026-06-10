#!/usr/bin/env python3
"""Hybrid routing comparison for M055 parser benchmark S04.

Compares the S02 GROBID-only and S03 OpenDataLoader-only per-PDF packets,
emits a per-PDF routing recommendation, and summarizes residual gaps. The
script only reads benchmark artifacts and writes diagnostic comparison packets;
it never writes graph data, never attempts production import, and keeps all five
safety defaults false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import uuid
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055-parser-benchmark.hybrid-routing.v1"
DEFAULT_GROBID_DIR = Path("artifacts/m055-parser-benchmark/grobid-only/per-pdf")
DEFAULT_OPENDATALOADER_DIR = Path("artifacts/m055-parser-benchmark/opendataloader-only/per-pdf")
DEFAULT_OUTPUT_DIR = Path("artifacts/m055-parser-benchmark/hybrid-routing")
BODY_MARKDOWN_MIN_BYTES = 5_000
LAYOUT_MIN_BOUNDING_BOXES = 1
PROCESSING_TIME_TIE_RATIO = 0.10
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}
ROUTING_DIMENSIONS = ("metadata", "citations", "body_content", "layout")
DIAGNOSTIC_DIMENSIONS = ("processing_time", "quality")
ALL_DIMENSIONS = ROUTING_DIMENSIONS + DIAGNOSTIC_DIMENSIONS


def _utc_now() -> str:
    return dt.datetime.now(tz=dt.UTC).isoformat()


def _safety_defaults() -> dict[str, bool]:
    return dict(SAFETY_DEFAULTS)


def _atomic_write_bytes(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + f".{uuid.uuid4().hex}.tmp")
    tmp_path.write_bytes(payload)
    tmp_path.replace(path)


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    _atomic_write_bytes(
        path,
        (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode("utf-8"),
    )


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"Invalid JSON packet: {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValueError(f"Expected object JSON packet: {path}")
    return payload


def _load_packets(per_pdf_dir: Path) -> dict[str, dict[str, Any]]:
    """Load per-PDF JSON packets keyed by arxiv_id."""
    if not per_pdf_dir.exists():
        raise FileNotFoundError(f"Per-PDF directory not found: {per_pdf_dir}")
    packets: dict[str, dict[str, Any]] = {}
    for packet_path in sorted(per_pdf_dir.glob("*.json")):
        packet = _load_json(packet_path)
        arxiv_id = str(packet.get("arxiv_id") or packet.get("article_key") or packet_path.stem)
        if not arxiv_id:
            raise ValueError(f"Packet has no arxiv_id/article_key: {packet_path}")
        if arxiv_id in packets:
            raise ValueError(f"Duplicate arxiv_id {arxiv_id!r} in {per_pdf_dir}")
        packets[arxiv_id] = packet
    if not packets:
        raise ValueError(f"No per-PDF JSON packets found in {per_pdf_dir}")
    return packets


def _as_int(value: Any) -> int:
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, int):
        return value
    if isinstance(value, float):
        return int(value)
    if isinstance(value, str) and value.strip().isdigit():
        return int(value.strip())
    return 0


def _as_bool(value: Any) -> bool:
    return bool(value)


def _dimension(
    *,
    winner: str,
    reason: str,
    grobid: dict[str, Any],
    opendataloader: dict[str, Any],
) -> dict[str, Any]:
    return {
        "winner": winner,
        "reason": reason,
        "grobid": grobid,
        "opendataloader": opendataloader,
    }


def _compare_metadata(grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]) -> dict[str, Any]:
    grobid_metrics = {
        "header_title_present": _as_bool(grobid_packet.get("header_title_present")),
        "header_author_count": _as_int(grobid_packet.get("header_author_count")),
        "abstract_present": _as_bool(grobid_packet.get("abstract_present")),
    }
    opendl_metrics = {
        "section_count": _as_int(opendataloader_packet.get("section_count")),
        "page_count": _as_int(opendataloader_packet.get("page_count")),
        "markdown_size_bytes": _as_int(opendataloader_packet.get("markdown_size_bytes")),
    }
    grobid_score = int(grobid_metrics["header_title_present"])
    grobid_score += min(grobid_metrics["header_author_count"], 1)
    grobid_score += int(grobid_metrics["abstract_present"])
    opendl_score = int(opendl_metrics["section_count"] > 0) + int(opendl_metrics["page_count"] > 0)
    if grobid_score >= 2:
        winner = "grobid"
        reason = "GROBID exposes native header metadata with title/author/abstract signals."
    elif opendl_score > 0:
        winner = "opendataloader"
        reason = "GROBID header metadata is weak; OpenDataLoader at least exposes document/page structure."
    else:
        winner = "none"
        reason = "Neither parser exposed usable metadata signals."
    return _dimension(winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics)


def _compare_citations(grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]) -> dict[str, Any]:
    grobid_metrics = {
        "ref_count": _as_int(grobid_packet.get("ref_count")),
        "bibl_count": _as_int(grobid_packet.get("bibl_count")),
    }
    opendl_metrics = {
        "native_citation_extraction": False,
        "ref_count": None,
        "bibl_count": None,
    }
    if grobid_metrics["ref_count"] + grobid_metrics["bibl_count"] > 0:
        winner = "grobid"
        reason = "GROBID exposes native reference/bibliography counts; OpenDataLoader has no native citation extraction."
    else:
        winner = "none"
        reason = "Neither parser exposed native citation extraction for this packet."
    return _dimension(winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics)


def _compare_body_content(grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]) -> dict[str, Any]:
    grobid_metrics = {"body_element_count": _as_int(grobid_packet.get("body_element_count"))}
    opendl_metrics = {
        "markdown_size_bytes": _as_int(opendataloader_packet.get("markdown_size_bytes")),
        "table_count": _as_int(opendataloader_packet.get("table_count")),
        "image_count": _as_int(opendataloader_packet.get("image_count")),
    }
    opendl_score = int(opendl_metrics["markdown_size_bytes"] >= BODY_MARKDOWN_MIN_BYTES)
    opendl_score += int(opendl_metrics["table_count"] > 0)
    opendl_score += int(opendl_metrics["image_count"] > 0)
    if opendl_score > 0 and opendl_metrics["markdown_size_bytes"] > grobid_metrics["body_element_count"]:
        winner = "opendataloader"
        reason = "OpenDataLoader exposes substantial markdown body content plus table/image signals."
    elif grobid_metrics["body_element_count"] > 0:
        winner = "grobid"
        reason = "GROBID exposes TEI body elements while OpenDataLoader body signals are weak."
    else:
        winner = "none"
        reason = "Neither parser exposed usable body content."
    return _dimension(winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics)


def _compare_layout(grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]) -> dict[str, Any]:
    grobid_metrics = {"native_layout_extraction": False, "bounding_box_count": None}
    opendl_metrics = {"bounding_box_count": _as_int(opendataloader_packet.get("bounding_box_count"))}
    if opendl_metrics["bounding_box_count"] >= LAYOUT_MIN_BOUNDING_BOXES:
        winner = "opendataloader"
        reason = "OpenDataLoader exposes layout bounding boxes; GROBID header probe has no layout output."
    else:
        winner = "none"
        reason = "Neither parser exposed layout bounding boxes."
    return _dimension(winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics)


def _compare_processing_time(grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]) -> dict[str, Any]:
    grobid_ms = _as_int(grobid_packet.get("duration_ms"))
    opendl_ms = _as_int(opendataloader_packet.get("duration_ms"))
    grobid_metrics = {"duration_ms": grobid_ms}
    opendl_metrics = {"duration_ms": opendl_ms}
    if grobid_ms <= 0 and opendl_ms <= 0:
        winner = "none"
        reason = "Neither packet includes usable duration metrics."
    elif grobid_ms <= 0:
        winner = "opendataloader"
        reason = "Only OpenDataLoader includes a usable duration metric."
    elif opendl_ms <= 0:
        winner = "grobid"
        reason = "Only GROBID includes a usable duration metric."
    else:
        faster = min(grobid_ms, opendl_ms)
        slower = max(grobid_ms, opendl_ms)
        if (slower - faster) / slower <= PROCESSING_TIME_TIE_RATIO:
            winner = "tie"
            reason = "Processing times are within the tie threshold."
        elif grobid_ms < opendl_ms:
            winner = "grobid"
            reason = "GROBID completed faster for this packet."
        else:
            winner = "opendataloader"
            reason = "OpenDataLoader completed faster for this packet."
    return _dimension(winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics)


def _compare_quality(grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]) -> dict[str, Any]:
    grobid_metrics = {
        "status": grobid_packet.get("status"),
        "low_quality_source": _as_bool(grobid_packet.get("low_quality_source")),
    }
    opendl_metrics = {
        "status": opendataloader_packet.get("status"),
        "low_quality_source": _as_bool(opendataloader_packet.get("low_quality_source")),
    }
    if grobid_metrics["low_quality_source"] and not opendl_metrics["low_quality_source"]:
        winner = "opendataloader"
        reason = "GROBID marked the source low quality while OpenDataLoader succeeded without that flag."
    elif opendl_metrics["low_quality_source"] and not grobid_metrics["low_quality_source"]:
        winner = "grobid"
        reason = "OpenDataLoader marked the source low quality while GROBID did not."
    elif grobid_metrics["low_quality_source"] == opendl_metrics["low_quality_source"]:
        winner = "tie"
        reason = "Both packets have the same low-quality flag value."
    else:
        winner = "none"
        reason = "Quality flags are not comparable."
    return _dimension(winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics)


def _compare_dimensions(grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]) -> dict[str, Any]:
    """Compare parser outputs dimension-by-dimension."""
    return {
        "metadata": _compare_metadata(grobid_packet, opendataloader_packet),
        "citations": _compare_citations(grobid_packet, opendataloader_packet),
        "body_content": _compare_body_content(grobid_packet, opendataloader_packet),
        "layout": _compare_layout(grobid_packet, opendataloader_packet),
        "processing_time": _compare_processing_time(grobid_packet, opendataloader_packet),
        "quality": _compare_quality(grobid_packet, opendataloader_packet),
    }


def _propose_route(comparison: dict[str, Any]) -> dict[str, Any]:
    """Propose routing from measured dimension winners, without hardcoding hybrid."""
    use_grobid_for = [dim for dim in ROUTING_DIMENSIONS if comparison[dim]["winner"] == "grobid"]
    use_opendataloader_for = [dim for dim in ROUTING_DIMENSIONS if comparison[dim]["winner"] == "opendataloader"]
    diagnostic_winners = {dim: comparison[dim]["winner"] for dim in DIAGNOSTIC_DIMENSIONS}

    if use_grobid_for and use_opendataloader_for:
        hybrid_route = "grobid_header + opendataloader_body"
    elif use_grobid_for:
        hybrid_route = "grobid_only"
    elif use_opendataloader_for:
        hybrid_route = "opendataloader_only"
    else:
        hybrid_route = "manual_review"

    core_winners = [comparison[dim]["winner"] for dim in ROUTING_DIMENSIONS]
    if hybrid_route == "grobid_header + opendataloader_body" and set(core_winners) >= {"grobid", "opendataloader"}:
        confidence = "high"
    elif hybrid_route in {"grobid_only", "opendataloader_only"} and core_winners.count(use_grobid_for and "grobid" or "opendataloader") >= 3:
        confidence = "medium"
    elif any(winner in {"grobid", "opendataloader"} for winner in core_winners):
        confidence = "medium"
    else:
        confidence = "low"

    return {
        "use_grobid_for": use_grobid_for,
        "use_opendataloader_for": use_opendataloader_for,
        "diagnostic_winners": diagnostic_winners,
        "hybrid_route": hybrid_route,
        "confidence": confidence,
    }


def _identify_residual_gaps(comparison: dict[str, Any]) -> list[dict[str, Any]]:
    """Identify gaps not closed by either parser alone."""
    gaps: list[dict[str, Any]] = []
    body = comparison["body_content"]
    layout = comparison["layout"]
    citations = comparison["citations"]
    body_bytes = _as_int(body["opendataloader"].get("markdown_size_bytes"))
    grobid_body_elements = _as_int(body["grobid"].get("body_element_count"))

    if body["winner"] == "none" and layout["winner"] == "none":
        gaps.append(
            {
                "gap": "ocr_required_for_scanned_or_empty_pdf",
                "severity": "high",
                "reason": "Neither parser exposed body text or layout boxes; scanned or malformed PDFs would need OCR/escalation.",
            }
        )

    if citations["winner"] == "grobid" and body["winner"] == "opendataloader":
        gaps.append(
            {
                "gap": "citation_to_body_alignment",
                "severity": "medium",
                "reason": "GROBID citations and OpenDataLoader body/layout are separate outputs; neither parser produces aligned citation spans in the markdown body.",
            }
        )

    if body_bytes < BODY_MARKDOWN_MIN_BYTES and grobid_body_elements == 0:
        gaps.append(
            {
                "gap": "math_equation_or_dense_body_extraction",
                "severity": "medium",
                "reason": "Both parsers have weak body signals; equation-heavy or dense body content would need targeted validation.",
            }
        )

    if body["winner"] == "opendataloader" and (
        _as_int(body["opendataloader"].get("table_count")) > 0 or _as_int(body["opendataloader"].get("image_count")) > 0
    ):
        gaps.append(
            {
                "gap": "table_figure_semantic_linking",
                "severity": "medium",
                "reason": "OpenDataLoader detects tables/images, but neither parser links them to normalized citations or graph-ready semantic entities.",
            }
        )

    return gaps


def _metrics_from_packets(
    grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]
) -> tuple[dict[str, Any], dict[str, Any]]:
    grobid_metrics = {
        "status": grobid_packet.get("status"),
        "low_quality_source": _as_bool(grobid_packet.get("low_quality_source")),
        "duration_ms": _as_int(grobid_packet.get("duration_ms")),
        "bytes": _as_int(grobid_packet.get("bytes")),
        "tei_size_bytes": _as_int(grobid_packet.get("tei_size_bytes")),
        "header_title_present": _as_bool(grobid_packet.get("header_title_present")),
        "header_author_count": _as_int(grobid_packet.get("header_author_count")),
        "abstract_present": _as_bool(grobid_packet.get("abstract_present")),
        "ref_count": _as_int(grobid_packet.get("ref_count")),
        "bibl_count": _as_int(grobid_packet.get("bibl_count")),
        "body_element_count": _as_int(grobid_packet.get("body_element_count")),
    }
    opendl_metrics = {
        "status": opendataloader_packet.get("status"),
        "low_quality_source": _as_bool(opendataloader_packet.get("low_quality_source")),
        "duration_ms": _as_int(opendataloader_packet.get("duration_ms")),
        "bytes": _as_int(opendataloader_packet.get("bytes")),
        "markdown_size_bytes": _as_int(opendataloader_packet.get("markdown_size_bytes")),
        "section_count": _as_int(opendataloader_packet.get("section_count")),
        "page_count": _as_int(opendataloader_packet.get("page_count")),
        "table_count": _as_int(opendataloader_packet.get("table_count")),
        "image_count": _as_int(opendataloader_packet.get("image_count")),
        "bounding_box_count": _as_int(opendataloader_packet.get("bounding_box_count")),
    }
    return grobid_metrics, opendl_metrics


def _comparison_packet(
    arxiv_id: str,
    grobid_packet: dict[str, Any],
    opendataloader_packet: dict[str, Any],
) -> dict[str, Any]:
    comparison = _compare_dimensions(grobid_packet, opendataloader_packet)
    route = _propose_route(comparison)
    residual_gaps = _identify_residual_gaps(comparison)
    grobid_metrics, opendl_metrics = _metrics_from_packets(grobid_packet, opendataloader_packet)
    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "arxiv_id": arxiv_id,
        "article_key": grobid_packet.get("article_key") or opendataloader_packet.get("article_key") or arxiv_id,
        "category": grobid_packet.get("category") or opendataloader_packet.get("category"),
        "pdf_path": grobid_packet.get("pdf_path") or opendataloader_packet.get("pdf_path"),
        "manifest_sha256": grobid_packet.get("manifest_sha256") or opendataloader_packet.get("manifest_sha256"),
        "grobid_metrics": grobid_metrics,
        "opendataloader_metrics": opendl_metrics,
        "comparison_table": comparison,
        "recommended_route": route,
        "residual_gaps": residual_gaps,
        "safety_defaults": _safety_defaults(),
    }


def _route_label(route: dict[str, Any]) -> str:
    return str(route.get("hybrid_route", "manual_review"))


def _summarize(packets: list[dict[str, Any]], output_dir: Path) -> dict[str, Any]:
    route_counts: dict[str, int] = {}
    dimension_winners: dict[str, dict[str, int]] = {
        dim: {"grobid": 0, "opendataloader": 0, "tie": 0, "none": 0} for dim in ALL_DIMENSIONS
    }
    gap_counts: dict[str, int] = {}
    per_pdf_routes: dict[str, dict[str, Any]] = {}

    for packet in packets:
        arxiv_id = str(packet["arxiv_id"])
        route = packet["recommended_route"]
        label = _route_label(route)
        route_counts[label] = route_counts.get(label, 0) + 1
        per_pdf_routes[arxiv_id] = route
        for dim, entry in packet["comparison_table"].items():
            winner = str(entry.get("winner", "none"))
            dimension_winners[dim][winner] = dimension_winners[dim].get(winner, 0) + 1
        for gap in packet["residual_gaps"]:
            gap_name = str(gap["gap"])
            gap_counts[gap_name] = gap_counts.get(gap_name, 0) + 1

    total_pdfs = len(packets)
    hybrid_count = route_counts.get("grobid_header + opendataloader_body", 0)
    aggregate_recommendation = {
        "recommended_route": "grobid_header + opendataloader_body" if hybrid_count == total_pdfs and total_pdfs else "mixed_or_review",
        "hybrid_pdf_count": hybrid_count,
        "hybrid_percent": round((hybrid_count / total_pdfs) * 100, 2) if total_pdfs else 0.0,
        "route_counts": route_counts,
        "rationale": "Use GROBID where native header/citation metrics win and OpenDataLoader where body/layout metrics win.",
    }

    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "output_dir": str(output_dir),
        "total_pdfs": total_pdfs,
        "per_pdf_routes": per_pdf_routes,
        "aggregate_routing_recommendation": aggregate_recommendation,
        "dimension_winners": dimension_winners,
        "residual_gap_counts": gap_counts,
        "safety_defaults": _safety_defaults(),
    }


def compare_hybrid_routing(
    grobid_dir: Path,
    opendataloader_dir: Path,
    output_dir: Path,
) -> dict[str, Any]:
    """Compare GROBID and OpenDataLoader packet directories and write outputs."""
    grobid_packets = _load_packets(grobid_dir)
    opendl_packets = _load_packets(opendataloader_dir)
    missing_from_opendl = sorted(set(grobid_packets) - set(opendl_packets))
    missing_from_grobid = sorted(set(opendl_packets) - set(grobid_packets))
    if missing_from_opendl or missing_from_grobid:
        raise ValueError(
            "Packet sets differ: "
            f"missing_from_opendataloader={missing_from_opendl}, missing_from_grobid={missing_from_grobid}"
        )

    per_pdf_dir = output_dir / "per-pdf"
    packets: list[dict[str, Any]] = []
    for arxiv_id in sorted(grobid_packets):
        packet = _comparison_packet(arxiv_id, grobid_packets[arxiv_id], opendl_packets[arxiv_id])
        packets.append(packet)
        _atomic_write_json(per_pdf_dir / f"{arxiv_id}.json", packet)

    summary = _summarize(packets, output_dir)
    _atomic_write_json(output_dir / "summary.json", summary)
    return summary


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--grobid-dir", type=Path, default=DEFAULT_GROBID_DIR)
    parser.add_argument("--opendataloader-dir", type=Path, default=DEFAULT_OPENDATALOADER_DIR)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return parser


def main() -> int:
    args = _build_parser().parse_args()
    summary = compare_hybrid_routing(args.grobid_dir, args.opendataloader_dir, args.output_dir)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
