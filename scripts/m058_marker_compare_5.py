#!/usr/bin/env python3
"""M058-cmjp1u S02: compare Marker pilot output with available OpenDataLoader packets."""
from __future__ import annotations

import json
from pathlib import Path
from statistics import mean
from typing import Any

OUTPUT_ROOT = Path("artifacts/m058-marker/pilot-5")
MARKER_SUMMARY_PATH = OUTPUT_ROOT / "summary.json"
COMPARISON_JSON = OUTPUT_ROOT / "comparison.json"
COMPARISON_MD = OUTPUT_ROOT / "comparison.md"
DECISION_MD = OUTPUT_ROOT / "decision.md"
LOOPBACK_BIND_HOST = "127.0.0.1"

SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}

ODL_CANDIDATE_PATTERNS = [
    "artifacts/m055deep-parser-benchmark/opendataloader-correctness/per-pdf/{arxiv_id}.json",
    "artifacts/m055deep-parser-benchmark/opendataloader-20/per-pdf/{arxiv_id}.json",
]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def find_odl_packet(arxiv_id: str) -> tuple[Path | None, dict[str, Any] | None]:
    """Return the preferred OpenDataLoader packet for an arXiv ID, if present."""
    for pattern in ODL_CANDIDATE_PATTERNS:
        path = Path(pattern.format(arxiv_id=arxiv_id))
        if path.exists():
            return path, load_json(path)
    return None, None


def first_number(*values: Any) -> float | None:
    for value in values:
        if isinstance(value, bool):
            continue
        if isinstance(value, (int, float)):
            return float(value)
    return None


def odl_table_count(packet: dict[str, Any]) -> int | None:
    metrics = packet.get("correctness_metrics") or {}
    value = first_number(packet.get("table_count"), packet.get("tables_total"), metrics.get("tables_total"))
    if value is not None:
        return int(value)
    tables = metrics.get("tables")
    if isinstance(tables, list):
        return len(tables)
    return None


def odl_figure_count(packet: dict[str, Any]) -> int | None:
    metrics = packet.get("correctness_metrics") or {}
    value = first_number(packet.get("figure_count"), packet.get("figures_total"), metrics.get("figures_total"))
    if value is not None:
        return int(value)
    captions = metrics.get("captions")
    if isinstance(captions, list):
        return sum(1 for item in captions if item.get("caption_type") == "figure")
    return None


def odl_body_word_count(packet: dict[str, Any]) -> int | None:
    value = first_number(packet.get("body_word_count"))
    if value is not None:
        return int(value)
    markdown = packet.get("markdown") or packet.get("markdown_text") or packet.get("text")
    if isinstance(markdown, str) and markdown.strip():
        return len(markdown.split())
    return None


def odl_elapsed_sec(packet: dict[str, Any]) -> float | None:
    return first_number(packet.get("elapsed_sec"), packet.get("runtime_sec"), packet.get("pdf_parse_seconds"))


def quality_score(table_count: int | None, figure_count: int | None, body_word_count: int | None) -> int:
    """Transparent extraction-coverage score for relative pilot comparison.

    Missing OpenDataLoader fields contribute zero rather than invented values.
    """
    return int(body_word_count or 0) + int(table_count or 0) * 50 + int(figure_count or 0) * 20


def compare_packet(marker_packet: dict[str, Any]) -> dict[str, Any]:
    arxiv_id = marker_packet["arxiv_id"]
    odl_path, odl_packet = find_odl_packet(arxiv_id)
    marker_table_count = int(marker_packet.get("table_count") or 0)
    marker_figure_count = int(marker_packet.get("figure_count") or 0)
    marker_body_word_count = int(marker_packet.get("body_word_count") or 0)
    marker_elapsed_sec = first_number(marker_packet.get("elapsed_sec"))

    if odl_packet is None:
        marker_score = quality_score(marker_table_count, marker_figure_count, marker_body_word_count)
        return {
            "arxiv_id": arxiv_id,
            "status": "odl_not_available",
            "marker_status": marker_packet.get("status"),
            "marker_table_count": marker_table_count,
            "marker_figure_count": marker_figure_count,
            "marker_body_word_count": marker_body_word_count,
            "marker_markdown_length": marker_packet.get("markdown_length"),
            "marker_elapsed_sec": marker_elapsed_sec,
            "marker_quality_score": marker_score,
            "odl_path": None,
            "odl_status": None,
            "odl_table_count": None,
            "odl_figure_count": None,
            "odl_body_word_count": None,
            "odl_elapsed_sec": None,
            "table_count_delta": None,
            "body_word_count_delta": None,
            "time_ratio_marker_over_odl": None,
            "quality_delta": None,
            "marker_better_than_odl": None,
        }

    odl_tables = odl_table_count(odl_packet)
    odl_figures = odl_figure_count(odl_packet)
    odl_words = odl_body_word_count(odl_packet)
    odl_time = odl_elapsed_sec(odl_packet)
    marker_score = quality_score(marker_table_count, marker_figure_count, marker_body_word_count)
    odl_score = quality_score(odl_tables, odl_figures, odl_words)
    quality_delta = marker_score - odl_score

    return {
        "arxiv_id": arxiv_id,
        "status": "compared",
        "marker_status": marker_packet.get("status"),
        "marker_table_count": marker_table_count,
        "marker_figure_count": marker_figure_count,
        "marker_body_word_count": marker_body_word_count,
        "marker_markdown_length": marker_packet.get("markdown_length"),
        "marker_elapsed_sec": marker_elapsed_sec,
        "marker_quality_score": marker_score,
        "odl_path": str(odl_path),
        "odl_status": odl_packet.get("status"),
        "odl_table_count": odl_tables,
        "odl_figure_count": odl_figures,
        "odl_body_word_count": odl_words,
        "odl_elapsed_sec": odl_time,
        "odl_quality_score": odl_score,
        "table_count_delta": marker_table_count - odl_tables if odl_tables is not None else None,
        "body_word_count_delta": marker_body_word_count - odl_words if odl_words is not None else None,
        "time_ratio_marker_over_odl": round(marker_elapsed_sec / odl_time, 3)
        if marker_elapsed_sec is not None and odl_time not in (None, 0)
        else None,
        "quality_delta": quality_delta,
        "marker_better_than_odl": quality_delta > 0,
    }


def build_comparison() -> dict[str, Any]:
    marker_summary = load_json(MARKER_SUMMARY_PATH)
    comparisons = [compare_packet(packet) for packet in marker_summary["per_pdf"]]
    compared = [item for item in comparisons if item["status"] == "compared"]
    quality_deltas = [item["quality_delta"] for item in compared if item.get("quality_delta") is not None]
    marker_wins = [item for item in compared if item.get("marker_better_than_odl") is True]
    extracted = [packet for packet in marker_summary["per_pdf"] if packet.get("status") == "marker_extracted"]
    avg_marker_elapsed = mean(float(packet.get("elapsed_sec") or 0) for packet in extracted) if extracted else 0.0
    avg_quality_delta = mean(quality_deltas) if quality_deltas else 0.0
    marker_win_percent = (len(marker_wins) / len(compared) * 100.0) if compared else 0.0
    page_limited = bool(marker_summary.get("page_range"))
    substituted_input = bool(marker_summary.get("unavailable_requested_sample"))
    go_to_s03 = (
        bool(extracted)
        and len(extracted) == marker_summary["sample_size"]
        and avg_quality_delta > 0
        and avg_marker_elapsed <= 900
        and not page_limited
        and not substituted_input
    )

    comparison = {
        "schema_version": "m058.marker-vs-opendataloader-5.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "loopback_bind_host": LOOPBACK_BIND_HOST,
        "sample_size": marker_summary["sample_size"],
        "executed_sample": marker_summary["executed_sample"],
        "requested_sample": marker_summary.get("requested_sample"),
        "unavailable_requested_sample": marker_summary.get("unavailable_requested_sample", []),
        "successful_marker_extractions": len(extracted),
        "available_odl_comparisons": len(compared),
        "avg_quality_delta": round(avg_quality_delta, 3),
        "marker_better_than_odl_percent": round(marker_win_percent, 1),
        "avg_marker_elapsed_sec": round(avg_marker_elapsed, 3),
        "page_range": marker_summary.get("page_range"),
        "page_limited": page_limited,
        "substituted_input": substituted_input,
        "go_to_s03": go_to_s03,
        "go_to_s03_rationale": (
            "Marker extracted all five full PDFs and beats available OpenDataLoader packets within the pilot cost bound."
            if go_to_s03
            else "Do not proceed automatically to S03: this evidence is page-limited and the requested 2305.14314 input is not available locally. Marker quality is promising, but full-document cost/input readiness must be resolved first."
            if page_limited or substituted_input
            else "Do not proceed: Marker did not clear extraction, quality, or cost thresholds."
        ),
        "per_pdf": comparisons,
    }
    return comparison


def write_markdown_report(comparison: dict[str, Any]) -> str:
    lines = [
        "# M058 S02 Marker vs OpenDataLoader comparison",
        "",
        "## Safety defaults",
        "",
        "External network is not authorized; fact promotion is not authorized; graph writes are disabled; "
        "LLM calls are disabled; production import is disabled.",
        f"Loopback bind host: `{LOOPBACK_BIND_HOST}`.",
        "",
        "## Aggregate",
        "",
        f"- Marker extractions: {comparison['successful_marker_extractions']}/{comparison['sample_size']}",
        f"- Available OpenDataLoader comparisons: {comparison['available_odl_comparisons']}/{comparison['sample_size']}",
        f"- Avg quality delta: {comparison['avg_quality_delta']}",
        f"- Marker > OpenDataLoader: {comparison['marker_better_than_odl_percent']}%",
        f"- Avg Marker elapsed seconds: {comparison['avg_marker_elapsed_sec']}",
        f"- Page range: {comparison.get('page_range')}",
        f"- Go to S03: {'yes' if comparison['go_to_s03'] else 'no'}",
        "",
        "## Per-PDF comparison",
        "",
        "| arxiv_id | status | marker tables | ODL tables | marker words | ODL words | marker sec | time ratio | quality delta | Marker > ODL |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |",
    ]
    for item in comparison["per_pdf"]:
        lines.append(
            "| {arxiv_id} | {status} | {marker_table_count} | {odl_table_count} | "
            "{marker_body_word_count} | {odl_body_word_count} | {marker_elapsed_sec} | "
            "{time_ratio_marker_over_odl} | {quality_delta} | {marker_better_than_odl} |".format(**item)
        )
    lines.extend(
        [
            "",
            "## Notes",
            "",
            "- `quality_delta` uses body words + 50 points per table + 20 points per figure; missing ODL fields contribute zero rather than invented values.",
            "- Requested `2305.14314` is not available in the local repository; `1804.02767` from M058 S01 is used as the fifth executable PDF.",
            "- Marker was run with `page_range=0` after full-document and three-page attempts exceeded the command budget before writing the first packet.",
        ]
    )
    return "\n".join(lines) + "\n"


def write_decision(comparison: dict[str, Any]) -> str:
    decision = "yes" if comparison["go_to_s03"] else "no"
    lines = [
        "# M058 S02 decision",
        "",
        f"Decision: go to S03 (cumulative 15)? **{decision}**.",
        "",
        "## Rationale",
        "",
        comparison["go_to_s03_rationale"],
        "",
        "## Safety boundary",
        "",
        "External network is not authorized. Fact promotion is not authorized. Graph writes are disabled. "
        "LLM calls are disabled. Production import is disabled.",
        "",
        "## Cost and quality signals",
        "",
        f"- Avg quality delta: {comparison['avg_quality_delta']}",
        f"- Marker > OpenDataLoader: {comparison['marker_better_than_odl_percent']}%",
        f"- Avg Marker elapsed seconds: {comparison['avg_marker_elapsed_sec']}",
        f"- Page range: {comparison.get('page_range')}",
        f"- Available ODL comparisons: {comparison['available_odl_comparisons']}/{comparison['sample_size']}",
        "",
        "## Deviation",
        "",
        "The planned fifth ID `2305.14314` is not available locally and does not appear in the M058 S01 plotextractor summary. "
        "The executable fifth PDF is `1804.02767`, which is present in M058 S01 and has OpenDataLoader correctness data.",
    ]
    return "\n".join(lines) + "\n"


def write_outputs() -> dict[str, Any]:
    comparison = build_comparison()
    OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    COMPARISON_JSON.write_text(json.dumps(comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    COMPARISON_MD.write_text(write_markdown_report(comparison), encoding="utf-8")
    DECISION_MD.write_text(write_decision(comparison), encoding="utf-8")
    print(f"Wrote {COMPARISON_JSON}")
    print(f"Wrote {COMPARISON_MD}")
    print(f"Wrote {DECISION_MD}")
    return comparison


def main() -> None:
    write_outputs()


if __name__ == "__main__":
    main()
