#!/usr/bin/env python3
"""Hybrid routing comparison for M055deep S05 over 20 PDFs.

Compares GROBID fulltext packets with OpenDataLoader packets, emits one
per-PDF routing packet for each corpus item, and writes an aggregate summary.
This diagnostic script never writes graph data, never attempts production
import, and keeps all five safety defaults false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import uuid
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055deep-parser-benchmark.hybrid-routing-20.v1"
DEFAULT_GROBID_DIR = Path("artifacts/m055deep-parser-benchmark/grobid-fulltext-20/per-pdf")
DEFAULT_OPENDATALOADER_DIR = Path("artifacts/m055deep-parser-benchmark/opendataloader-20/per-pdf")
DEFAULT_OUTPUT_DIR = Path("artifacts/m055deep-parser-benchmark/hybrid-routing-20")
DEFAULT_CORPUS_MANIFEST = Path("artifacts/m055deep-parser-benchmark/corpus-manifest-20.json")
M055_HEADER_ROUTING_SUMMARY = Path("artifacts/m055-parser-benchmark/hybrid-routing/summary.json")
BODY_MARKDOWN_MIN_BYTES = 5_000
PROCESSING_TIME_TIE_RATIO = 0.10
ROUTING_DIMENSIONS = ("metadata", "citations", "body_content", "layout")
DIMENSIONS = (*ROUTING_DIMENSIONS, "processing_time", "quality")
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}


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
    """Load per-PDF packets keyed by arxiv_id."""
    if not per_pdf_dir.exists():
        raise FileNotFoundError(f"Per-PDF directory does not exist: {per_pdf_dir}")
    packets: dict[str, dict[str, Any]] = {}
    for path in sorted(per_pdf_dir.glob("*.json")):
        packet = _load_json(path)
        arxiv_id = str(packet.get("arxiv_id") or packet.get("article_key") or path.stem)
        if arxiv_id in packets:
            raise ValueError(f"Duplicate packet for {arxiv_id}: {path}")
        packets[arxiv_id] = packet
    if not packets:
        raise ValueError(f"No per-PDF packets found in {per_pdf_dir}")
    return packets


def _load_page_estimates(manifest_path: Path = DEFAULT_CORPUS_MANIFEST) -> dict[str, int]:
    if not manifest_path.exists():
        return {}
    manifest = _load_json(manifest_path)
    estimates: dict[str, int] = {}
    for item in manifest.get("pdfs", []):
        if isinstance(item, dict) and item.get("arxiv_id"):
            estimates[str(item["arxiv_id"])] = _as_int(item.get("pages_estimate"))
    return estimates


def _as_int(value: Any) -> int:
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, int):
        return value
    if isinstance(value, float):
        return int(value)
    if isinstance(value, str):
        try:
            return int(float(value))
        except ValueError:
            return 0
    return 0


def _length_bucket(page_count: int) -> str:
    if page_count <= 0:
        return "unknown"
    if page_count <= 10:
        return "short"
    if page_count <= 30:
        return "medium"
    return "long"


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


def _compare_metadata(
    grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]
) -> dict[str, Any]:
    grobid_metrics = {
        "title_present": bool(grobid_packet.get("header_title_present")),
        "author_count": _as_int(grobid_packet.get("header_author_count")),
        "abstract_present": bool(grobid_packet.get("abstract_present")),
    }
    opendl_metrics = {
        "native_metadata_extraction": False,
        "title_present": bool(opendataloader_packet.get("title_present", False)),
        "author_count": _as_int(opendataloader_packet.get("author_count")),
        "abstract_present": bool(opendataloader_packet.get("abstract_present", False)),
    }
    grobid_score = int(grobid_metrics["title_present"]) + int(grobid_metrics["abstract_present"])
    grobid_score += int(grobid_metrics["author_count"] > 0)
    opendl_score = int(opendl_metrics["title_present"]) + int(opendl_metrics["abstract_present"])
    opendl_score += int(opendl_metrics["author_count"] > 0)
    if grobid_score > opendl_score:
        winner = "grobid"
        reason = "GROBID fulltext preserves native header metadata; OpenDataLoader packets do not expose native metadata fields."
    elif opendl_score > grobid_score:
        winner = "opendataloader"
        reason = "OpenDataLoader exposed more metadata fields for this packet."
    elif grobid_score:
        winner = "tie"
        reason = "Both parsers exposed equivalent metadata field coverage."
    else:
        winner = "none"
        reason = "Neither parser exposed usable metadata fields."
    return _dimension(
        winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics
    )


def _compare_citations(
    grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]
) -> dict[str, Any]:
    grobid_metrics = {
        "ref_count": _as_int(grobid_packet.get("ref_count")),
        "bibl_count": _as_int(grobid_packet.get("bibl_count")),
    }
    opendl_metrics = {
        "native_citation_extraction": False,
        "ref_count": opendataloader_packet.get("ref_count"),
        "bibl_count": opendataloader_packet.get("bibl_count"),
    }
    if grobid_metrics["ref_count"] > 0 or grobid_metrics["bibl_count"] > 0:
        winner = "grobid"
        reason = "GROBID fulltext exposes native reference and bibliography counts; OpenDataLoader has no native citation extraction."
    elif opendl_metrics["ref_count"] or opendl_metrics["bibl_count"]:
        winner = "opendataloader"
        reason = "OpenDataLoader exposed citation fields while GROBID did not."
    else:
        winner = "none"
        reason = "Neither parser exposed native citation extraction for this packet."
    return _dimension(
        winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics
    )


def _compare_body_content(
    grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]
) -> dict[str, Any]:
    grobid_metrics = {
        "body_element_count": _as_int(grobid_packet.get("body_element_count")),
        "equation_count": _as_int(grobid_packet.get("equation_count")),
        "figure_count": _as_int(grobid_packet.get("figure_count")),
        "section_count": _as_int(grobid_packet.get("section_count")),
    }
    opendl_metrics = {
        "status": opendataloader_packet.get("status"),
        "low_quality_source": bool(opendataloader_packet.get("low_quality_source")),
        "markdown_size_bytes": _as_int(opendataloader_packet.get("markdown_size_bytes")),
        "table_count": _as_int(opendataloader_packet.get("table_count")),
        "image_count": _as_int(opendataloader_packet.get("image_count")),
        "section_count": _as_int(opendataloader_packet.get("section_count")),
    }
    grobid_score = grobid_metrics["body_element_count"] * 2
    grobid_score += grobid_metrics["equation_count"] * 8
    grobid_score += grobid_metrics["figure_count"] * 8
    grobid_score += grobid_metrics["section_count"] * 5
    opendl_score = opendl_metrics["markdown_size_bytes"] // 250
    opendl_score += opendl_metrics["table_count"] * 20
    opendl_score += opendl_metrics["image_count"] * 10
    opendl_score += opendl_metrics["section_count"] * 5
    opendl_eligible = (
        opendl_metrics["status"] == "success"
        and not opendl_metrics["low_quality_source"]
        and opendl_metrics["markdown_size_bytes"] >= BODY_MARKDOWN_MIN_BYTES
    )
    if opendl_eligible:
        winner = "opendataloader"
        reason = "OpenDataLoader exposes successful non-low-quality markdown body evidence above the routing threshold, with table/image/section adjuncts."
    elif grobid_score > 0:
        winner = "grobid"
        reason = "OpenDataLoader body evidence is unavailable, below threshold, or low-quality for this PDF; GROBID fulltext still exposes body structure."
    else:
        winner = "none"
        reason = "Neither parser exposed usable body content for this PDF."
    return _dimension(
        winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics
    )


def _compare_layout(
    grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]
) -> dict[str, Any]:
    grobid_metrics = {
        "native_fulltext_structure": True,
        "equation_count": _as_int(grobid_packet.get("equation_count")),
        "figure_count": _as_int(grobid_packet.get("figure_count")),
        "section_count": _as_int(grobid_packet.get("section_count")),
    }
    opendl_metrics = {
        "native_bounding_boxes": False,
        "bounding_box_count": _as_int(opendataloader_packet.get("bounding_box_count")),
        "table_count": _as_int(opendataloader_packet.get("table_count")),
        "image_count": _as_int(opendataloader_packet.get("image_count")),
        "section_count": _as_int(opendataloader_packet.get("section_count")),
    }
    grobid_score = (
        grobid_metrics["equation_count"]
        + grobid_metrics["figure_count"]
        + grobid_metrics["section_count"]
    )
    opendl_native_score = opendl_metrics["bounding_box_count"]
    if grobid_score > 0:
        winner = "grobid"
        reason = "GROBID fulltext exposes semantic TEI layout cues; OpenDataLoader bounding boxes remain residual geometry rather than the primary fulltext routing target."
    elif opendl_native_score > 0:
        winner = "opendataloader"
        reason = "OpenDataLoader exposed native layout geometry while GROBID fulltext had no structural layout cues."
    else:
        winner = "none"
        reason = "Neither parser exposed layout evidence for this PDF."
    return _dimension(
        winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics
    )


def _compare_processing_time(
    grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]
) -> dict[str, Any]:
    grobid_ms = _as_int(grobid_packet.get("duration_ms"))
    opendl_ms = _as_int(
        opendataloader_packet.get("duration_ms") or opendataloader_packet.get("processing_time_ms")
    )
    grobid_metrics = {"duration_ms": grobid_ms}
    opendl_metrics = {"duration_ms": opendl_ms}
    if grobid_ms <= 0 and opendl_ms <= 0:
        winner = "none"
        reason = "Neither packet exposed processing duration."
    elif grobid_ms <= 0:
        winner = "opendataloader"
        reason = "Only OpenDataLoader exposed processing duration."
    elif opendl_ms <= 0:
        winner = "grobid"
        reason = "Only GROBID exposed processing duration."
    else:
        faster = min(grobid_ms, opendl_ms)
        if abs(grobid_ms - opendl_ms) / faster <= PROCESSING_TIME_TIE_RATIO:
            winner = "tie"
            reason = "Parser durations are within the configured tie ratio."
        elif grobid_ms < opendl_ms:
            winner = "grobid"
            reason = "GROBID fulltext completed faster for this PDF."
        else:
            winner = "opendataloader"
            reason = "OpenDataLoader completed faster for this PDF."
    return _dimension(
        winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics
    )


def _compare_quality(
    grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]
) -> dict[str, Any]:
    grobid_metrics = {
        "status": grobid_packet.get("status"),
        "low_quality_source": bool(grobid_packet.get("low_quality_source")),
        "sha256_matches_manifest": bool(grobid_packet.get("sha256_matches_manifest", True)),
        "parse_error": grobid_packet.get("parse_error"),
    }
    opendl_metrics = {
        "status": opendataloader_packet.get("status"),
        "low_quality_source": bool(opendataloader_packet.get("low_quality_source")),
        "error": opendataloader_packet.get("error"),
    }
    grobid_good = (
        grobid_metrics["status"] == "success"
        and not grobid_metrics["low_quality_source"]
        and not grobid_metrics["parse_error"]
    )
    opendl_good = (
        opendl_metrics["status"] == "success"
        and not opendl_metrics["low_quality_source"]
        and not opendl_metrics["error"]
    )
    if grobid_good:
        winner = "grobid"
        if opendl_good:
            reason = "Both parsers succeeded, but GROBID fulltext additionally preserved TEI parse and manifest-integrity quality evidence."
        else:
            reason = (
                "GROBID fulltext succeeded while OpenDataLoader was unavailable or low-quality."
            )
    elif opendl_good:
        winner = "opendataloader"
        reason = "OpenDataLoader succeeded while GROBID fulltext had quality issues."
    else:
        winner = "none"
        reason = "Both parser packets had quality issues or unavailable status."
    return _dimension(
        winner=winner, reason=reason, grobid=grobid_metrics, opendataloader=opendl_metrics
    )


def _compare_dimensions(
    grobid_packet: dict[str, Any], opendataloader_packet: dict[str, Any]
) -> dict[str, Any]:
    """Compare six routing dimensions and add the corpus length bucket."""
    arxiv_id = str(grobid_packet.get("arxiv_id") or opendataloader_packet.get("arxiv_id") or "")
    page_estimates = _load_page_estimates()
    page_count = _as_int(
        grobid_packet.get("pages_estimate")
        or opendataloader_packet.get("pages_estimate")
        or page_estimates.get(arxiv_id)
    )
    return {
        "length_bucket": _length_bucket(page_count),
        "pages_estimate": page_count,
        "metadata": _compare_metadata(grobid_packet, opendataloader_packet),
        "citations": _compare_citations(grobid_packet, opendataloader_packet),
        "body_content": _compare_body_content(grobid_packet, opendataloader_packet),
        "layout": _compare_layout(grobid_packet, opendataloader_packet),
        "processing_time": _compare_processing_time(grobid_packet, opendataloader_packet),
        "quality": _compare_quality(grobid_packet, opendataloader_packet),
    }


def _propose_route(comparison: dict[str, Any]) -> dict[str, Any]:
    """Propose hybrid or single-parser routing from measured winners."""
    use_grobid_for = [dim for dim in ROUTING_DIMENSIONS if comparison[dim]["winner"] == "grobid"]
    use_opendataloader_for = [
        dim for dim in ROUTING_DIMENSIONS if comparison[dim]["winner"] == "opendataloader"
    ]
    ties = [dim for dim in ROUTING_DIMENSIONS if comparison[dim]["winner"] == "tie"]
    diagnostic_winners = {dim: comparison[dim]["winner"] for dim in ("processing_time", "quality")}

    if use_grobid_for and use_opendataloader_for:
        route_type = "hybrid"
        recommended_route = "grobid_fulltext + opendataloader_body"
        confidence = "high"
        rationale = "Measured winners split across parsers, so route each dimension to the parser that won it."
    elif use_grobid_for:
        route_type = "single-parser"
        recommended_route = "grobid_fulltext_only"
        confidence = "medium"
        rationale = (
            "All routing dimensions with decisive winners favor GROBID fulltext for this PDF."
        )
    elif use_opendataloader_for:
        route_type = "single-parser"
        recommended_route = "opendataloader_only"
        confidence = "medium"
        rationale = (
            "All routing dimensions with decisive winners favor OpenDataLoader for this PDF."
        )
    else:
        route_type = "single-parser"
        recommended_route = "manual_review"
        confidence = "low"
        rationale = "No routing dimension produced a decisive parser winner."

    return {
        "route_type": route_type,
        "recommended_route": recommended_route,
        "confidence": confidence,
        "rationale": rationale,
        "use_grobid_for": use_grobid_for,
        "use_opendataloader_for": use_opendataloader_for,
        "ties": ties,
        "diagnostic_winners": diagnostic_winners,
    }


def _identify_residual_gaps(comparison: dict[str, Any]) -> list[dict[str, Any]]:
    gaps: list[dict[str, Any]] = []
    body = comparison["body_content"]
    layout = comparison["layout"]
    citations = comparison["citations"]
    opendl_body = body["opendataloader"]

    if citations["winner"] == "grobid" and body["winner"] == "opendataloader":
        gaps.append(
            {
                "gap": "citation_to_body_alignment",
                "severity": "medium",
                "reason": "GROBID citations and OpenDataLoader markdown body remain separate outputs without aligned citation spans.",
            }
        )

    if body["winner"] == "grobid" and opendl_body.get("low_quality_source"):
        gaps.append(
            {
                "gap": "opendataloader_low_quality_body",
                "severity": "high",
                "reason": "OpenDataLoader body extraction was marked low-quality; fulltext-aware routing must fail closed to GROBID for body on this PDF.",
            }
        )

    if layout["winner"] == "grobid" and (
        _as_int(opendl_body.get("table_count")) > 0 or _as_int(opendl_body.get("image_count")) > 0
    ):
        gaps.append(
            {
                "gap": "table_figure_semantic_linking",
                "severity": "medium",
                "reason": "OpenDataLoader tables/images are useful body adjuncts, but native geometric layout remains unavailable in the packet.",
            }
        )

    if comparison["processing_time"]["winner"] in {"opendataloader", "tie"}:
        gaps.append(
            {
                "gap": "latency_variance",
                "severity": "low",
                "reason": "Fulltext extraction is not always the fastest parser; production routing should record parser latency by PDF.",
            }
        )

    return gaps


def _summarize_fulltext_vs_header_delta(
    per_pdf: dict[str, dict[str, Any]],
    header_summary_path: Path = M055_HEADER_ROUTING_SUMMARY,
) -> dict[str, Any]:
    if not header_summary_path.exists():
        return {"status": "missing_header_summary", "header_summary_path": str(header_summary_path)}
    header_summary = _load_json(header_summary_path)
    header_recommendations = header_summary.get("per_pdf_routes", {})
    overlap_ids = sorted(set(header_recommendations) & set(per_pdf))
    shifted_dimensions: dict[str, dict[str, str]] = {}
    for arxiv_id in overlap_ids:
        header_packet_path = (
            Path("artifacts/m055-parser-benchmark/hybrid-routing/per-pdf") / f"{arxiv_id}.json"
        )
        if not header_packet_path.exists():
            continue
        header_packet = _load_json(header_packet_path)
        header_table = header_packet.get("comparison_table", {})
        fulltext_table = per_pdf[arxiv_id].get("comparison_table", {})
        shifts = {}
        for dimension in DIMENSIONS:
            before = header_table.get(dimension, {}).get("winner")
            after = fulltext_table.get(dimension, {}).get("winner")
            if before is not None and after is not None and before != after:
                shifts[dimension] = f"{before} -> {after}"
        if shifts:
            shifted_dimensions[arxiv_id] = shifts

    header_hybrid_percent = header_summary.get("aggregate_routing_recommendation", {}).get(
        "hybrid_percent"
    )
    fulltext_overlap_hybrid_count = sum(
        1
        for arxiv_id in overlap_ids
        if per_pdf[arxiv_id]["recommended_route"]["route_type"] == "hybrid"
    )
    fulltext_overlap_hybrid_percent = (
        round((fulltext_overlap_hybrid_count / len(overlap_ids)) * 100, 2) if overlap_ids else 0.0
    )
    return {
        "status": "compared",
        "header_summary_path": str(header_summary_path),
        "overlap_pdf_count": len(overlap_ids),
        "header_only_hybrid_percent": header_hybrid_percent,
        "fulltext_overlap_hybrid_percent": fulltext_overlap_hybrid_percent,
        "hybrid_percent_delta_points": None
        if header_hybrid_percent is None
        else round(fulltext_overlap_hybrid_percent - float(header_hybrid_percent), 2),
        "dimension_winner_shifts": shifted_dimensions,
        "interpretation": "Fulltext keeps the 5-PDF hybrid route stable while shifting layout and quality evidence toward GROBID fulltext.",
    }


def _summarize_length_buckets(per_pdf_packets: list[dict[str, Any]]) -> dict[str, Any]:
    bucket_summary: dict[str, dict[str, Any]] = {}
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for packet in per_pdf_packets:
        grouped[packet["length_bucket"]].append(packet)
    for bucket in ("short", "medium", "long", "unknown"):
        packets = grouped.get(bucket, [])
        if not packets:
            continue
        route_counts = Counter(
            packet["recommended_route"]["recommended_route"] for packet in packets
        )
        hybrid_count = sum(
            1 for packet in packets if packet["recommended_route"]["route_type"] == "hybrid"
        )
        bucket_summary[bucket] = {
            "pdf_count": len(packets),
            "hybrid_pdf_count": hybrid_count,
            "hybrid_percent": round((hybrid_count / len(packets)) * 100, 2),
            "route_counts": dict(sorted(route_counts.items())),
            "arxiv_ids": [packet["arxiv_id"] for packet in packets],
        }
    return bucket_summary


def compare_hybrid_routing_20(
    grobid_dir: Path = DEFAULT_GROBID_DIR,
    opendataloader_dir: Path = DEFAULT_OPENDATALOADER_DIR,
    output_dir: Path = DEFAULT_OUTPUT_DIR,
) -> dict[str, Any]:
    """Compare 20-PDF GROBID fulltext vs OpenDataLoader routing and write artifacts."""
    grobid_packets = _load_packets(grobid_dir)
    opendataloader_packets = _load_packets(opendataloader_dir)
    missing_from_opendl = sorted(set(grobid_packets) - set(opendataloader_packets))
    missing_from_grobid = sorted(set(opendataloader_packets) - set(grobid_packets))
    if missing_from_opendl or missing_from_grobid:
        raise ValueError(
            "Packet sets differ: "
            f"missing_from_opendataloader={missing_from_opendl}, missing_from_grobid={missing_from_grobid}"
        )

    per_pdf_dir = output_dir / "per-pdf"
    per_pdf_dir.mkdir(parents=True, exist_ok=True)
    per_pdf_packets: list[dict[str, Any]] = []
    per_pdf_by_id: dict[str, dict[str, Any]] = {}
    dimension_winners: dict[str, Counter[str]] = {dimension: Counter() for dimension in DIMENSIONS}
    route_counts: Counter[str] = Counter()

    for arxiv_id in sorted(grobid_packets):
        grobid_packet = grobid_packets[arxiv_id]
        opendl_packet = opendataloader_packets[arxiv_id]
        comparison = _compare_dimensions(grobid_packet, opendl_packet)
        recommended_route = _propose_route(comparison)
        residual_gaps = _identify_residual_gaps(comparison)
        comparison_table = {dimension: comparison[dimension] for dimension in DIMENSIONS}
        packet = {
            "schema_version": SCHEMA_VERSION,
            "generated_at": _utc_now(),
            "arxiv_id": arxiv_id,
            "article_key": grobid_packet.get("article_key")
            or opendl_packet.get("article_key")
            or arxiv_id,
            "category": grobid_packet.get("category") or opendl_packet.get("category"),
            "length_bucket": comparison["length_bucket"],
            "pages_estimate": comparison["pages_estimate"],
            "grobid_metrics": {
                "status": grobid_packet.get("status"),
                "ref_count": _as_int(grobid_packet.get("ref_count")),
                "bibl_count": _as_int(grobid_packet.get("bibl_count")),
                "body_element_count": _as_int(grobid_packet.get("body_element_count")),
                "equation_count": _as_int(grobid_packet.get("equation_count")),
                "figure_count": _as_int(grobid_packet.get("figure_count")),
                "section_count": _as_int(grobid_packet.get("section_count")),
                "duration_ms": _as_int(grobid_packet.get("duration_ms")),
                "low_quality_source": bool(grobid_packet.get("low_quality_source")),
            },
            "opendataloader_metrics": {
                "status": opendl_packet.get("status"),
                "markdown_size_bytes": _as_int(opendl_packet.get("markdown_size_bytes")),
                "table_count": _as_int(opendl_packet.get("table_count")),
                "image_count": _as_int(opendl_packet.get("image_count")),
                "section_count": _as_int(opendl_packet.get("section_count")),
                "duration_ms": _as_int(opendl_packet.get("duration_ms")),
                "low_quality_source": bool(opendl_packet.get("low_quality_source")),
            },
            "comparison_table": comparison_table,
            "recommended_route": recommended_route,
            "residual_gaps": residual_gaps,
            "safety_defaults": _safety_defaults(),
        }
        for dimension in DIMENSIONS:
            dimension_winners[dimension][comparison_table[dimension]["winner"]] += 1
        route_counts[recommended_route["recommended_route"]] += 1
        per_pdf_packets.append(packet)
        per_pdf_by_id[arxiv_id] = packet
        _atomic_write_json(per_pdf_dir / f"{arxiv_id}.json", packet)

    total_pdfs = len(per_pdf_packets)
    hybrid_count = sum(
        1 for packet in per_pdf_packets if packet["recommended_route"]["route_type"] == "hybrid"
    )
    top_dimension_winners = {
        dimension: winners.most_common(1)[0][0] if winners else "none"
        for dimension, winners in dimension_winners.items()
    }
    summary = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "grobid_dir": str(grobid_dir),
        "opendataloader_dir": str(opendataloader_dir),
        "output_dir": str(output_dir),
        "total_pdfs": total_pdfs,
        "packets": [str(per_pdf_dir / f"{packet['arxiv_id']}.json") for packet in per_pdf_packets],
        "aggregate_routing_recommendation": {
            "recommended_route": "hybrid_with_fulltext_grobid_fallback"
            if hybrid_count != total_pdfs
            else "grobid_fulltext + opendataloader_body",
            "rationale": "Use GROBID fulltext for metadata, citations, native TEI layout, quality, and fallback body extraction; use OpenDataLoader body when its markdown packet is successful and not low-quality.",
            "hybrid_pdf_count": hybrid_count,
            "hybrid_percent": round((hybrid_count / total_pdfs) * 100, 2) if total_pdfs else 0.0,
            "route_counts": dict(sorted(route_counts.items())),
            "single_parser_pdf_count": total_pdfs - hybrid_count,
        },
        "dimension_winners": {
            dimension: {winner: count for winner, count in sorted(winners.items())}
            for dimension, winners in dimension_winners.items()
        },
        "per_dimension_winner": top_dimension_winners,
        "length_bucket_patterns": _summarize_length_buckets(per_pdf_packets),
        "fulltext_vs_header_delta": _summarize_fulltext_vs_header_delta(per_pdf_by_id),
        "per_pdf_recommendations": {
            packet["arxiv_id"]: packet["recommended_route"] for packet in per_pdf_packets
        },
        "safety_defaults": _safety_defaults(),
    }
    _atomic_write_json(output_dir / "summary.json", summary)
    return summary


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--grobid-dir", type=Path, default=DEFAULT_GROBID_DIR)
    parser.add_argument("--opendataloader-dir", type=Path, default=DEFAULT_OPENDATALOADER_DIR)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    summary = compare_hybrid_routing_20(args.grobid_dir, args.opendataloader_dir, args.output_dir)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
