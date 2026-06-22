#!/usr/bin/env python3
"""Render the M055deep parser benchmark report.

The report is a deterministic markdown artifact built from existing benchmark
JSON packets. It performs no parser execution, no graph writes, and no
production import.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055deep-parser-benchmark-report.v1"
DEFAULT_BENCHMARK_DIR = Path("artifacts/m055deep-parser-benchmark")
DEFAULT_M055_DIR = Path("artifacts/m055-parser-benchmark")
DEFAULT_OUTPUT = DEFAULT_BENCHMARK_DIR / "REPORT.md"
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}


def _read_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"Expected object JSON in {path}")
    return payload


def _packet_map(per_pdf_dir: Path) -> dict[str, dict[str, Any]]:
    packets: dict[str, dict[str, Any]] = {}
    for path in sorted(per_pdf_dir.glob("*.json")):
        packet = _read_json(path)
        packets[str(packet.get("arxiv_id") or path.stem)] = packet
    if not packets:
        raise ValueError(f"No per-PDF packets found in {per_pdf_dir}")
    return packets


def _fmt_int(value: Any) -> str:
    if value is None:
        return "0"
    if isinstance(value, bool):
        return str(int(value))
    try:
        return f"{int(value):,}"
    except (TypeError, ValueError):
        return str(value)


def _fmt_float(value: Any) -> str:
    try:
        return f"{float(value):.2f}"
    except (TypeError, ValueError):
        return "0.00"


def _safety_block() -> list[str]:
    lines = [
        "## Safety Defaults",
        "",
        "The benchmark and report are diagnostic only. Production import is not authorized.",
        "",
        "| Flag | Value |",
        "| --- | --- |",
    ]
    for key in sorted(SAFETY_DEFAULTS):
        lines.append(f"| `{key}` | `{str(SAFETY_DEFAULTS[key]).lower()}` |")
    lines.extend(
        [
            "",
            "These defaults are repeated in the S05 routing summary and per-PDF packets.",
            "Graph writes, LadybugDB writes, fact promotion, and production import are not authorized by this report.",
            "",
        ]
    )
    return lines


def _per_pdf_table(
    manifest: dict[str, Any],
    grobid_packets: dict[str, dict[str, Any]],
    opendl_packets: dict[str, dict[str, Any]],
    routing_packets: dict[str, dict[str, Any]],
) -> list[str]:
    lines = [
        "## Per-PDF Routing Table",
        "",
        "| # | arXiv ID | Bucket | Pages | GROBID refs | GROBID body | GROBID eq | GROBID fig | ODL status | ODL markdown bytes | ODL tables | ODL images | Route |",
        "| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | --- |",
    ]
    for idx, item in enumerate(manifest.get("pdfs", []), start=1):
        arxiv_id = str(item["arxiv_id"])
        grobid = grobid_packets[arxiv_id]
        opendl = opendl_packets[arxiv_id]
        route = routing_packets[arxiv_id]
        lines.append(
            "| "
            + " | ".join(
                [
                    str(idx),
                    arxiv_id,
                    route["length_bucket"],
                    str(route["pages_estimate"]),
                    _fmt_int(grobid.get("ref_count")),
                    _fmt_int(grobid.get("body_element_count")),
                    _fmt_int(grobid.get("equation_count")),
                    _fmt_int(grobid.get("figure_count")),
                    str(opendl.get("status")),
                    _fmt_int(opendl.get("markdown_size_bytes")),
                    _fmt_int(opendl.get("table_count")),
                    _fmt_int(opendl.get("image_count")),
                    route["recommended_route"]["recommended_route"],
                ]
            )
            + " |"
        )
    lines.append("")
    return lines


def _dimension_table(routing_summary: dict[str, Any]) -> list[str]:
    lines = [
        "## Per-Dimension Winner Analysis",
        "",
        "| Dimension | Aggregate winner | GROBID wins | OpenDataLoader wins | Ties | None | Interpretation |",
        "| --- | --- | ---: | ---: | ---: | ---: | --- |",
    ]
    interpretations = {
        "metadata": "GROBID fulltext preserves native header extraction.",
        "citations": "GROBID exposes native reference and bibliography counts.",
        "body_content": "OpenDataLoader usually provides richer markdown body/table/image evidence, except one low-quality fallback.",
        "layout": "GROBID fulltext now contributes TEI sections, figures, and equations; ODL packets do not expose native bounding boxes.",
        "processing_time": "GROBID is the plurality winner, though latency varies by PDF.",
        "quality": "GROBID has 20/20 successful fulltext packets; OpenDataLoader has one low-quality source.",
    }
    for dimension in [
        "metadata",
        "citations",
        "body_content",
        "layout",
        "processing_time",
        "quality",
    ]:
        counts = routing_summary["dimension_winners"][dimension]
        lines.append(
            f"| {dimension} | {routing_summary['per_dimension_winner'][dimension]} | "
            f"{counts.get('grobid', 0)} | {counts.get('opendataloader', 0)} | "
            f"{counts.get('tie', 0)} | {counts.get('none', 0)} | {interpretations[dimension]} |"
        )
    lines.append("")
    return lines


def _length_bucket_section(routing_summary: dict[str, Any]) -> list[str]:
    lines = [
        "## Length-Bucket Patterns",
        "",
        "| Bucket | PDF count | Hybrid count | Hybrid percent | Routes | arXiv IDs |",
        "| --- | ---: | ---: | ---: | --- | --- |",
    ]
    for bucket in ["short", "medium", "long", "unknown"]:
        if bucket not in routing_summary["length_bucket_patterns"]:
            continue
        data = routing_summary["length_bucket_patterns"][bucket]
        routes = ", ".join(f"{route}: {count}" for route, count in data["route_counts"].items())
        lines.append(
            f"| {bucket} | {data['pdf_count']} | {data['hybrid_pdf_count']} | "
            f"{_fmt_float(data['hybrid_percent'])}% | {routes} | {', '.join(data['arxiv_ids'])} |"
        )
    lines.extend(
        [
            "",
            "Observed pattern: short and long PDFs stayed 100% hybrid, while the medium bucket had one OpenDataLoader low-quality source and therefore one GROBID-fulltext-only fallback.",
            "The result argues for fulltext-aware fallback rather than a blanket 100% hybrid assumption.",
            "",
        ]
    )
    return lines


def _axis_sections(
    m055_dir: Path,
    benchmark_dir: Path,
    grobid20: dict[str, Any],
    opendl20: dict[str, Any],
    routing20: dict[str, Any],
) -> list[str]:
    fulltext5 = _read_json(benchmark_dir / "grobid-fulltext" / "summary.json")
    odl5 = _read_json(benchmark_dir / "opendataloader-correctness" / "summary.json")
    header5 = _read_json(m055_dir / "hybrid-routing" / "summary.json")
    lines = ["## Five Evidence Axes", ""]
    axes = [
        (
            "Axis 1: GROBID header-only vs fulltext delta on 5 PDFs",
            [
                f"S01 fulltext succeeded on {fulltext5['aggregate_counts'].get('success', 0)}/{fulltext5['total_pdfs']} PDFs.",
                f"Fulltext body elements increased from header-only absence to {_fmt_int(fulltext5.get('total_body_element_count'))} total TEI body elements.",
                f"The 5-PDF route stayed at {header5['aggregate_routing_recommendation']['hybrid_percent']}% hybrid, but fulltext shifted layout and quality evidence toward GROBID.",
            ],
        ),
        (
            "Axis 2: OpenDataLoader correctness on 5 PDFs",
            [
                f"S02 correctness packets confirmed OpenDataLoader markdown/body usefulness across {odl5['total_pdfs']} PDFs.",
                "OpenDataLoader remains the body-content winner where markdown is successful and not low-quality.",
                "The evidence still does not authorize graph writes or fact promotion.",
            ],
        ),
        (
            "Axis 3: 20-PDF GROBID fulltext metrics",
            [
                f"S04 GROBID fulltext succeeded on {grobid20['aggregate_counts'].get('success', 0)}/{grobid20['total_pdfs']} PDFs.",
                f"Totals: refs={_fmt_int(grobid20.get('total_ref_count'))}, body={_fmt_int(grobid20.get('total_body_element_count'))}, equations={_fmt_int(grobid20.get('total_equation_count'))}, figures={_fmt_int(grobid20.get('total_figure_count'))}, bibliography={_fmt_int(grobid20.get('total_bibl_count'))}.",
                "This proves GROBID fulltext is no longer merely a header/citation source for this corpus.",
            ],
        ),
        (
            "Axis 4: 20-PDF OpenDataLoader metrics",
            [
                f"S04 OpenDataLoader succeeded on {opendl20['aggregate_counts'].get('success', 0)}/20 PDFs with {opendl20['aggregate_counts'].get('low_quality_source', 0)} low-quality source.",
                f"Totals: markdown bytes={_fmt_int(opendl20.get('total_markdown_size_bytes'))}, tables={_fmt_int(opendl20.get('total_table_count'))}, images={_fmt_int(opendl20.get('total_image_count'))}.",
                "OpenDataLoader remains the best body renderer when the packet is successful, but it now needs a per-PDF quality gate.",
            ],
        ),
        (
            "Axis 5: 20-PDF hybrid routing",
            [
                f"S05 recommends hybrid routing for {routing20['aggregate_routing_recommendation']['hybrid_pdf_count']}/20 PDFs ({_fmt_float(routing20['aggregate_routing_recommendation']['hybrid_percent'])}%).",
                f"Route counts: {routing20['aggregate_routing_recommendation']['route_counts']}.",
                "Per-dimension winners are GROBID for metadata, citations, layout, processing-time plurality, and quality; OpenDataLoader wins body_content in aggregate.",
            ],
        ),
    ]
    for title, bullets in axes:
        lines.extend([f"### {title}", ""])
        for bullet in bullets:
            lines.append(f"- {bullet}")
        lines.append("")
    return lines


def _per_pdf_detail_sections(
    manifest: dict[str, Any],
    grobid_packets: dict[str, dict[str, Any]],
    opendl_packets: dict[str, dict[str, Any]],
    routing_packets: dict[str, dict[str, Any]],
) -> list[str]:
    lines = ["## Per-PDF Detail Notes", ""]
    for idx, item in enumerate(manifest.get("pdfs", []), start=1):
        arxiv_id = str(item["arxiv_id"])
        grobid = grobid_packets[arxiv_id]
        opendl = opendl_packets[arxiv_id]
        route = routing_packets[arxiv_id]
        winners = {
            dimension: route["comparison_table"][dimension]["winner"]
            for dimension in [
                "metadata",
                "citations",
                "body_content",
                "layout",
                "processing_time",
                "quality",
            ]
        }
        lines.extend(
            [
                f"### {idx}. {arxiv_id}",
                "",
                f"- Length bucket: `{route['length_bucket']}` ({route['pages_estimate']} pages estimated).",
                f"- Recommended route: `{route['recommended_route']['recommended_route']}` with `{route['recommended_route']['confidence']}` confidence.",
                f"- Route rationale: {route['recommended_route']['rationale']}",
                f"- Use GROBID for: {', '.join(route['recommended_route']['use_grobid_for']) or 'none'}.",
                f"- Use OpenDataLoader for: {', '.join(route['recommended_route']['use_opendataloader_for']) or 'none'}.",
                f"- GROBID fulltext metrics: refs={_fmt_int(grobid.get('ref_count'))}, bibliography={_fmt_int(grobid.get('bibl_count'))}, body={_fmt_int(grobid.get('body_element_count'))}, equations={_fmt_int(grobid.get('equation_count'))}, figures={_fmt_int(grobid.get('figure_count'))}, sections={_fmt_int(grobid.get('section_count'))}.",
                f"- OpenDataLoader metrics: status=`{opendl.get('status')}`, markdown_bytes={_fmt_int(opendl.get('markdown_size_bytes'))}, tables={_fmt_int(opendl.get('table_count'))}, images={_fmt_int(opendl.get('image_count'))}, sections={_fmt_int(opendl.get('section_count'))}.",
                f"- Dimension winners: metadata={winners['metadata']}, citations={winners['citations']}, body_content={winners['body_content']}, layout={winners['layout']}, processing_time={winners['processing_time']}, quality={winners['quality']}.",
                f"- Metadata rationale: {route['comparison_table']['metadata']['reason']}",
                f"- Body rationale: {route['comparison_table']['body_content']['reason']}",
                f"- Layout rationale: {route['comparison_table']['layout']['reason']}",
                f"- Residual gaps: {', '.join(gap['gap'] for gap in route['residual_gaps']) or 'none'}.",
                "- Safety note: graph writes and production import are not authorized for this PDF.",
                "",
            ]
        )
    return lines


def render_report(
    benchmark_dir: Path = DEFAULT_BENCHMARK_DIR,
    m055_dir: Path = DEFAULT_M055_DIR,
    output_path: Path = DEFAULT_OUTPUT,
) -> str:
    manifest = _read_json(benchmark_dir / "corpus-manifest-20.json")
    grobid20 = _read_json(benchmark_dir / "grobid-fulltext-20" / "summary.json")
    opendl20 = _read_json(benchmark_dir / "opendataloader-20" / "summary.json")
    routing20 = _read_json(benchmark_dir / "hybrid-routing-20" / "summary.json")
    grobid_packets = _packet_map(benchmark_dir / "grobid-fulltext-20" / "per-pdf")
    opendl_packets = _packet_map(benchmark_dir / "opendataloader-20" / "per-pdf")
    routing_packets = _packet_map(benchmark_dir / "hybrid-routing-20" / "per-pdf")

    lines: list[str] = [
        "---",
        f"schema_version: {SCHEMA_VERSION}",
        "milestone: M055-kyxuqm",
        "slice: S06",
        "status: diagnostic-report",
        "---",
        "",
        "# M055deep Parser Benchmark Report",
        "",
        "## Executive Summary",
        "",
        "M055deep extends the M055 parser benchmark from a 5-PDF, header-oriented routing decision to a 20-PDF fulltext comparison.",
        "GROBID fulltext dominates five dimensions in aggregate: metadata, citations, native TEI layout, processing-time plurality, and quality.",
        "OpenDataLoader remains the aggregate body-content winner when its markdown extraction succeeds and is not low-quality.",
        f"The resulting route is hybrid for {routing20['aggregate_routing_recommendation']['hybrid_pdf_count']}/20 PDFs ({_fmt_float(routing20['aggregate_routing_recommendation']['hybrid_percent'])}%) and GROBID-fulltext-only for one OpenDataLoader low-quality medium-length PDF.",
        "This amends the operational interpretation of ADR-008: hybrid remains the default, but fulltext-aware per-PDF fallback is required.",
        "Production import is not authorized by this evidence package.",
        "",
        "## Mermaid Architecture Diagram",
        "",
        "```mermaid",
        "flowchart TD",
        "    A[PDF corpus] --> B[GROBID fulltext]",
        "    A --> C[OpenDataLoader]",
        "    B --> D{Routing comparison}",
        "    C --> D",
        "    D -->|metadata citations layout quality| E[GROBID-selected dimensions]",
        "    D -->|successful markdown body| F[OpenDataLoader body]",
        "    D -->|low-quality body packet| G[GROBID fulltext fallback]",
        "    E --> H[Candidate evidence layer]",
        "    F --> H",
        "    G --> H",
        "    H --> I[Diagnostic artifacts only]",
        "```",
        "",
    ]
    lines.extend(_axis_sections(m055_dir, benchmark_dir, grobid20, opendl20, routing20))
    lines.extend(_per_pdf_table(manifest, grobid_packets, opendl_packets, routing_packets))
    lines.extend(_dimension_table(routing20))
    lines.extend(_length_bucket_section(routing20))
    lines.extend(_safety_block())
    lines.extend(
        [
            "## Reconciliation With Prior Evidence",
            "",
            "### M055 five-PDF benchmark",
            "",
            "M055 S04 recommended 100% hybrid routing on five PDFs using GROBID header/citation evidence and OpenDataLoader body/layout evidence.",
            "M055deep preserves the 5-PDF hybrid decision on the overlapping PDFs but changes the reason: GROBID fulltext now wins layout and quality while OpenDataLoader remains the body renderer.",
            "This means ADR-008 remains directionally correct, but its implementation rule must become fulltext-aware.",
            "",
            "### M033 prior evidence",
            "",
            "M033 established that scientific-paper ingestion needs conservative evidence gates before promotion.",
            "M055deep is consistent with that posture: the benchmark emits diagnostic packets only and production import is not authorized.",
            "The new evidence improves parser selection but does not relax validation, provenance, or fact-promotion requirements.",
            "",
            "### M043 prior evidence",
            "",
            "M043 emphasized operational guardrails and bounded side effects for parser-adjacent workflows.",
            "M055deep follows the same boundary: parser metrics are artifacts, not writes to graph stores or downstream serving layers.",
            "The fulltext-aware fallback should be implemented as a bounded routing rule with observable reasons and failure states.",
            "",
            "## Recommendation",
            "",
            "Adopt ADR-009 as an amendment to ADR-008.",
            "Keep hybrid routing as the default path for successful OpenDataLoader markdown packets.",
            "Use GROBID fulltext as the fallback body parser when OpenDataLoader is low-quality, unavailable, or below the markdown evidence threshold.",
            "Continue using GROBID fulltext for metadata, citations, TEI structural layout, and parser-quality diagnostics.",
            "Do not authorize graph writes, LadybugDB writes, production import, or fact promotion from this benchmark report.",
            "",
            "## Machine-Readable Summary",
            "",
            "```json",
            json.dumps(
                {
                    "schema_version": SCHEMA_VERSION,
                    "total_pdfs": routing20["total_pdfs"],
                    "hybrid_percent": routing20["aggregate_routing_recommendation"][
                        "hybrid_percent"
                    ],
                    "route_counts": routing20["aggregate_routing_recommendation"]["route_counts"],
                    "per_dimension_winner": routing20["per_dimension_winner"],
                    "length_bucket_patterns": routing20["length_bucket_patterns"],
                    "safety_defaults": SAFETY_DEFAULTS,
                },
                indent=2,
                sort_keys=True,
            ),
            "```",
            "",
        ]
    )
    lines.extend(
        _per_pdf_detail_sections(manifest, grobid_packets, opendl_packets, routing_packets)
    )

    markdown = "\n".join(lines).rstrip() + "\n"
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(markdown, encoding="utf-8")
    return markdown


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--benchmark-dir", type=Path, default=DEFAULT_BENCHMARK_DIR)
    parser.add_argument("--m055-dir", type=Path, default=DEFAULT_M055_DIR)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    markdown = render_report(args.benchmark_dir, args.m055_dir, args.output)
    print(f"wrote {args.output} ({len(markdown.splitlines())} lines)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
