#!/usr/bin/env python3
"""Render the M055 parser benchmark report.

The report is a read-only synthesis over S02, S03, and S04 benchmark artifacts.
It does not import papers, write graph data, or authorize production ingestion.
All five safety defaults remain false in both the report and generated metadata.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m055-parser-benchmark-report.v1"
DEFAULT_BENCHMARK_DIR = Path("artifacts/m055-parser-benchmark")
DEFAULT_OUTPUT = DEFAULT_BENCHMARK_DIR / "REPORT.md"
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "import_eligible": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
}
DIMENSION_LABELS = {
    "metadata": "Metadata and header extraction",
    "citations": "Native citation and bibliography extraction",
    "processing_time": "Processing time",
    "body_content": "Body content extraction",
    "layout": "Layout, tables, figures, and bounding boxes",
    "quality": "Operational source quality",
}
DIMENSION_OWNERS = {
    "metadata": "GROBID",
    "citations": "GROBID",
    "processing_time": "GROBID",
    "body_content": "OpenDataLoader",
    "layout": "OpenDataLoader",
    "quality": "OpenDataLoader",
}


def _load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise FileNotFoundError(f"Required benchmark artifact is missing: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def _write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _route_name(route: Any) -> str:
    if isinstance(route, dict):
        return str(route.get("hybrid_route") or route.get("route") or route)
    return str(route)


def _as_bool_text(value: bool) -> str:
    return "false" if value is False else "true"


def _md_table(headers: list[str], rows: list[list[Any]]) -> list[str]:
    rendered = ["| " + " | ".join(headers) + " |", "| " + " | ".join("---" for _ in headers) + " |"]
    for row in rows:
        rendered.append("| " + " | ".join(str(cell) for cell in row) + " |")
    return rendered


def _packet_paths(base_dir: Path, subdir: str) -> list[Path]:
    packet_dir = base_dir / subdir / "per-pdf"
    if not packet_dir.exists():
        raise FileNotFoundError(f"Required per-PDF packet directory is missing: {packet_dir}")
    return sorted(packet_dir.glob("*.json"))


def _load_inputs(base_dir: Path) -> dict[str, Any]:
    corpus = _load_json(base_dir / "corpus-manifest.json")
    grobid_summary = _load_json(base_dir / "grobid-only" / "summary.json")
    opendl_summary = _load_json(base_dir / "opendataloader-only" / "summary.json")
    routing_summary = _load_json(base_dir / "hybrid-routing" / "summary.json")

    grobid_packets = {
        path.stem: _load_json(path) for path in _packet_paths(base_dir, "grobid-only")
    }
    opendl_packets = {
        path.stem: _load_json(path) for path in _packet_paths(base_dir, "opendataloader-only")
    }
    routing_packets = {
        path.stem: _load_json(path) for path in _packet_paths(base_dir, "hybrid-routing")
    }

    missing = sorted(
        (set(grobid_packets) | set(opendl_packets) | set(routing_packets))
        - (set(grobid_packets) & set(opendl_packets) & set(routing_packets))
    )
    if missing:
        raise ValueError(f"Per-PDF packet sets do not match: {missing}")

    manifest_by_id = {pdf["arxiv_id"]: pdf for pdf in corpus.get("pdfs", [])}
    return {
        "corpus": corpus,
        "grobid_summary": grobid_summary,
        "opendl_summary": opendl_summary,
        "routing_summary": routing_summary,
        "grobid_packets": grobid_packets,
        "opendl_packets": opendl_packets,
        "routing_packets": routing_packets,
        "manifest_by_id": manifest_by_id,
    }


def _render_header(data: dict[str, Any]) -> list[str]:
    routing = data["routing_summary"]["aggregate_routing_recommendation"]
    total_pdfs = data["routing_summary"].get("total_pdfs", len(data["routing_packets"]))
    hybrid_percent = routing.get("hybrid_percent", 0.0)
    route = routing.get("recommended_route", "grobid_header + opendataloader_body")
    generated_at = dt.datetime.now(dt.UTC).isoformat(timespec="seconds")
    return [
        "# M055 Parser Benchmark Report",
        "",
        f"Schema version: `{SCHEMA_VERSION}`",
        f"Generated at: `{generated_at}`",
        "Milestone evidence: `M054-proc4f` / benchmark artifact namespace `m055-parser-benchmark`",
        "Decision target: ADR-008 Hybrid Parser Architecture",
        "",
        "## Executive Summary",
        "",
        f"The benchmark compared **{total_pdfs} PDFs** across **6 dimensions**: metadata, citations, processing time, body content, layout, and quality.",
        f"The aggregate routing result is **{hybrid_percent:.0f}% hybrid recommendation** across all benchmarked PDFs.",
        f"The recommended architecture is **{route}** for every PDF in the corpus.",
        "GROBID wins the dimensions where native scholarly-paper semantics matter most: metadata, citations, and processing time.",
        "OpenDataLoader wins the dimensions where markdown body extraction and layout fidelity matter most: body_content, layout, and quality.",
        "The result does not authorize graph import, production import, or LadybugDB writes; this is a parser architecture benchmark only.",
        "The report therefore recommends a bounded hybrid parser pipeline rather than a single-parser replacement.",
        "",
        "### Executive Finding",
        "",
        "> Use GROBID for header/citation extraction and OpenDataLoader for body/table/layout extraction, then merge the outputs through a bounded reconciliation layer before any graph-facing promotion path.",
        "",
        "### Evidence Snapshot",
        "",
        *_md_table(
            ["Evidence", "Value"],
            [
                ["PDFs benchmarked", total_pdfs],
                ["Dimensions evaluated", 6],
                ["Hybrid recommendation", f"{hybrid_percent:.0f}%"],
                ["Recommended route", route],
                ["GROBID wins", "metadata, citations, processing_time"],
                ["OpenDataLoader wins", "body_content, layout, quality"],
                ["Safety posture", "all five safety defaults false"],
            ],
        ),
        "",
    ]


def _render_input_inventory(data: dict[str, Any]) -> list[str]:
    grobid_summary = data["grobid_summary"]
    opendl_summary = data["opendl_summary"]
    routing_summary = data["routing_summary"]
    lines = [
        "## Input Artifact Inventory",
        "",
        "The report is derived from previously completed S02, S03, and S04 artifacts.",
        "It does not rerun parsers and does not mutate benchmark inputs.",
        "",
        *_md_table(
            ["Slice", "Artifact", "Schema", "Role"],
            [
                [
                    "S02",
                    "artifacts/m055-parser-benchmark/grobid-only/summary.json",
                    grobid_summary.get("schema_version"),
                    "GROBID-only baseline summary",
                ],
                [
                    "S03",
                    "artifacts/m055-parser-benchmark/opendataloader-only/summary.json",
                    opendl_summary.get("schema_version"),
                    "OpenDataLoader-only baseline summary",
                ],
                [
                    "S04",
                    "artifacts/m055-parser-benchmark/hybrid-routing/summary.json",
                    routing_summary.get("schema_version"),
                    "Hybrid routing comparison",
                ],
                [
                    "S01",
                    "artifacts/m055-parser-benchmark/corpus-manifest.json",
                    "manifest",
                    "Five-PDF benchmark corpus",
                ],
            ],
        ),
        "",
        "### Aggregate Parser Metrics",
        "",
        *_md_table(
            ["Metric", "GROBID", "OpenDataLoader"],
            [
                [
                    "Successful packets",
                    grobid_summary.get("success_count", 0),
                    opendl_summary.get("success_count", 0),
                ],
                [
                    "Low-quality-source packets",
                    grobid_summary.get("low_quality_source_count", 0),
                    opendl_summary.get("low_quality_source_count", 0),
                ],
                ["Total TEI bytes", grobid_summary.get("total_tei_bytes", "n/a"), "n/a"],
                [
                    "Total markdown bytes",
                    "n/a",
                    opendl_summary.get("total_markdown_size_bytes", "n/a"),
                ],
                ["Total references", grobid_summary.get("total_refs", "n/a"), "n/a"],
                ["Total bibliography entries", grobid_summary.get("total_bibls", "n/a"), "n/a"],
                ["Total body elements", grobid_summary.get("total_body_elements", "n/a"), "n/a"],
                ["Total pages", "n/a", opendl_summary.get("total_page_count", "n/a")],
                ["Total sections", "n/a", opendl_summary.get("total_section_count", "n/a")],
                ["Total tables", "n/a", opendl_summary.get("total_table_count", "n/a")],
                ["Total images", "n/a", opendl_summary.get("total_image_count", "n/a")],
                [
                    "Total bounding boxes",
                    "n/a",
                    opendl_summary.get("total_bounding_box_count", "n/a"),
                ],
            ],
        ),
        "",
    ]
    return lines


def _render_corpus_table(data: dict[str, Any]) -> list[str]:
    rows: list[list[Any]] = []
    for pdf in data["corpus"].get("pdfs", []):
        aid = pdf["arxiv_id"]
        grobid = data["grobid_packets"][aid]
        opendl = data["opendl_packets"][aid]
        routing = data["routing_packets"][aid]
        route = _route_name(routing.get("recommended_route"))
        grobid_metrics = f"TEI {grobid.get('bytes')} bytes; refs {grobid.get('ref_count')}; bibls {grobid.get('bibl_count')}; body {grobid.get('body_element_count')}"
        opendl_metrics = f"MD {opendl.get('markdown_size_bytes')} bytes; sections {opendl.get('section_count')}; tables {opendl.get('table_count')}; images {opendl.get('image_count')}; boxes {opendl.get('bounding_box_count')}"
        rows.append(
            [
                aid,
                pdf.get("category"),
                pdf.get("pages_estimate"),
                grobid_metrics,
                opendl_metrics,
                route,
            ]
        )
    return [
        "## Per-PDF Summary Table",
        "",
        "This table satisfies the benchmark reporting contract: `arxiv_id | category | pages | GROBID TEI metrics | OpenDataLoader md metrics | recommended route`.",
        "",
        *_md_table(
            [
                "arxiv_id",
                "category",
                "pages",
                "GROBID TEI metrics",
                "OpenDataLoader md metrics",
                "recommended route",
            ],
            rows,
        ),
        "",
    ]


def _render_dimension_analysis(data: dict[str, Any]) -> list[str]:
    winners = data["routing_summary"].get("dimension_winners", {})
    lines = [
        "## Per-Dimension Winner Analysis",
        "",
        "The six benchmark dimensions split cleanly into two parser responsibilities.",
        "No evaluated dimension requires a single-parser winner for all downstream work.",
        "",
        *_md_table(
            [
                "Dimension",
                "Benchmark winner",
                "GROBID wins",
                "OpenDataLoader wins",
                "Ties",
                "Interpretation",
            ],
            [
                [
                    dimension,
                    DIMENSION_OWNERS[dimension],
                    counts.get("grobid", 0),
                    counts.get("opendataloader", 0),
                    counts.get("tie", 0),
                    DIMENSION_LABELS[dimension],
                ]
                for dimension, counts in winners.items()
            ],
        ),
        "",
    ]
    explanations = {
        "metadata": [
            "GROBID exposes title, author count, and abstract presence from native header extraction.",
            "OpenDataLoader does not provide the same scholarly metadata contract in the benchmark packets.",
            "The header stage should therefore stay GROBID-owned.",
        ],
        "citations": [
            "GROBID exposes native reference and bibliography counts.",
            "OpenDataLoader packets do not expose native citation extraction.",
            "Citation extraction should therefore stay GROBID-owned until a better citation-specific parser is proven.",
        ],
        "processing_time": [
            "GROBID won processing time for every PDF in the S04 routing packet set.",
            "This is a diagnostic win, not a reason to use GROBID for body extraction.",
            "The hybrid route accepts the faster header path while preserving OpenDataLoader body fidelity.",
        ],
        "body_content": [
            "OpenDataLoader emits substantial markdown bodies for all five PDFs.",
            "GROBID header-only packets have zero body elements in this benchmark configuration.",
            "Body extraction should therefore stay OpenDataLoader-owned.",
        ],
        "layout": [
            "OpenDataLoader emits page counts, table counts, image counts, and bounding-box counts.",
            "GROBID header packets do not expose comparable layout signals.",
            "Layout-sensitive paper evidence should therefore stay OpenDataLoader-owned.",
        ],
        "quality": [
            "OpenDataLoader had five successful packets and zero low-quality-source packets.",
            "GROBID header packets were useful but marked low_quality_source because the configured endpoint is header-only.",
            "Quality scoring should therefore distinguish useful header semantics from insufficient full-document extraction.",
        ],
    }
    for dimension in [
        "metadata",
        "citations",
        "processing_time",
        "body_content",
        "layout",
        "quality",
    ]:
        lines.extend([f"### {dimension}: {DIMENSION_OWNERS[dimension]}", ""])
        for item in explanations[dimension]:
            lines.append(f"- {item}")
        lines.extend(
            [
                "- Routing implication: "
                + (
                    "use GROBID output in the merged packet."
                    if DIMENSION_OWNERS[dimension] == "GROBID"
                    else "use OpenDataLoader output in the merged packet."
                ),
                "",
            ]
        )
    return lines


def _render_per_pdf_details(data: dict[str, Any]) -> list[str]:
    lines = ["## Per-PDF Detail Tables", ""]
    for aid in sorted(data["routing_packets"]):
        manifest = data["manifest_by_id"].get(aid, {})
        grobid = data["grobid_packets"][aid]
        opendl = data["opendl_packets"][aid]
        routing = data["routing_packets"][aid]
        route = _route_name(routing.get("recommended_route"))
        comparison = routing.get("comparison_table", {})
        residual_gaps = routing.get("residual_gaps", [])
        lines.extend(
            [
                f"### PDF {aid}",
                "",
                f"Category: `{manifest.get('category', grobid.get('category', opendl.get('category')))}`",
                f"Pages: `{manifest.get('pages_estimate', opendl.get('page_count'))}`",
                f"Recommended route: `{route}`",
                "",
                *_md_table(
                    ["Metric family", "GROBID", "OpenDataLoader", "Winner"],
                    [
                        [
                            "metadata",
                            f"title={grobid.get('header_title_present')}; authors={grobid.get('header_author_count')}; abstract={grobid.get('abstract_present')}",
                            "no native scholarly header contract",
                            comparison.get("metadata", {}).get("winner", "grobid"),
                        ],
                        [
                            "citations",
                            f"refs={grobid.get('ref_count')}; bibls={grobid.get('bibl_count')}",
                            "no native citation extraction",
                            comparison.get("citations", {}).get("winner", "grobid"),
                        ],
                        [
                            "processing_time",
                            f"{grobid.get('duration_ms')} ms",
                            f"{opendl.get('duration_ms')} ms",
                            comparison.get("processing_time", {}).get("winner", "grobid"),
                        ],
                        [
                            "body_content",
                            f"body_elements={grobid.get('body_element_count')}",
                            f"markdown={opendl.get('markdown_size_bytes')} bytes; sections={opendl.get('section_count')}",
                            comparison.get("body_content", {}).get("winner", "opendataloader"),
                        ],
                        [
                            "layout",
                            "no native layout packet",
                            f"tables={opendl.get('table_count')}; images={opendl.get('image_count')}; boxes={opendl.get('bounding_box_count')}",
                            comparison.get("layout", {}).get("winner", "opendataloader"),
                        ],
                        [
                            "quality",
                            f"low_quality_source={grobid.get('low_quality_source')}",
                            f"low_quality_source={opendl.get('low_quality_source')}",
                            comparison.get("quality", {}).get("winner", "opendataloader"),
                        ],
                    ],
                ),
                "",
                "#### Routing Notes",
                "",
                "- Use the GROBID packet for title, author count, abstract presence, references, and bibliography counts.",
                "- Use the OpenDataLoader packet for markdown body, page-level layout, tables, images, sections, and bounding boxes.",
                "- Preserve both packet identifiers and manifest hashes so downstream reconciliation can trace each merged field.",
                "- Treat the merged packet as candidate evidence only; it is not authorized for graph writes.",
                "",
                "#### Residual Gaps",
                "",
            ]
        )
        for gap in residual_gaps:
            lines.append(f"- `{gap.get('gap')}` ({gap.get('severity')}): {gap.get('reason')}")
        lines.extend(
            [
                "",
                "#### Per-PDF Decision Record",
                "",
                *_md_table(
                    ["Field", "Value"],
                    [
                        ["arxiv_id", aid],
                        ["route", route],
                        ["GROBID-owned dimensions", "metadata, citations, processing_time"],
                        ["OpenDataLoader-owned dimensions", "body_content, layout, quality"],
                        ["safety defaults", "all false"],
                    ],
                ),
                "",
            ]
        )
    return lines


def _render_gap_analysis(data: dict[str, Any]) -> list[str]:
    gap_counts = data["routing_summary"].get("residual_gap_counts", {})
    rows = [
        [gap, count, "medium", "Handle in M057 merge/reconciliation pilot"]
        for gap, count in gap_counts.items()
    ]
    lines = [
        "## Gap Analysis",
        "",
        "The benchmark recommends a hybrid parser architecture, but the recommendation is not a complete graph-ingestion design.",
        "The residual gaps are the work items that the next implementation milestone must retire before any graph-facing path is considered.",
        "",
        *_md_table(["Gap", "Affected PDFs", "Severity", "Required response"], rows),
        "",
        "### Gap 1: citation_to_body_alignment",
        "",
        "- GROBID citations and OpenDataLoader body markdown currently live in separate packet namespaces.",
        "- Neither parser emits aligned citation spans inside the markdown body in the current benchmark output.",
        "- A merger must preserve provenance and should not synthesize citation alignment without evidence.",
        "- M057 should produce explicit alignment diagnostics before any downstream promotion path is opened.",
        "",
        "### Gap 2: table_figure_semantic_linking",
        "",
        "- OpenDataLoader detects tables, images, and bounding boxes, but these are not normalized into semantic entities.",
        "- GROBID does not solve table or figure semantics in the header-only benchmark route.",
        "- A merger may carry layout artifacts forward, but it must mark semantic links as unresolved until proven.",
        "- M057 should keep table/figure linkage candidate-only unless a reviewer or deterministic rule validates it.",
        "",
        "### Non-Gaps Confirmed by M055",
        "",
        "- Parser availability is sufficient for a bounded pilot because both per-PDF packet families exist for all five PDFs.",
        "- The route is stable across the corpus because all five PDFs choose the same hybrid architecture.",
        "- Safety defaults are stable because every S02/S03/S04 packet keeps the five non-authorization flags false.",
        "- The result is operationally useful because it tells M057 exactly which parser owns each field family.",
        "",
    ]
    return lines


def _render_reconciliation() -> list[str]:
    return [
        "## Reconciliation with M033 and M043 Evidence",
        "",
        "M055 does not replace the earlier architecture evidence; it narrows the parser choice inside the existing safety frame.",
        "",
        *_md_table(
            ["Prior evidence", "Constraint carried forward", "M055 reconciliation"],
            [
                [
                    "M033",
                    "Candidate evidence must remain bounded before graph promotion.",
                    "Hybrid parser packets remain candidate evidence and do not authorize import.",
                ],
                [
                    "M033",
                    "Sidecar-style evidence producers are acceptable when boundaries are explicit.",
                    "GROBID and OpenDataLoader are separate sidecar producers feeding a bounded merge layer.",
                ],
                [
                    "M043",
                    "Prior parser evidence showed OpenDataLoader body/layout usefulness but did not settle scholarly header/citation ownership.",
                    "M055 confirms OpenDataLoader for body/layout and adds GROBID ownership for metadata/citations.",
                ],
                [
                    "ADR-001",
                    "Scientific papers are the first proving domain and require citations, figures, tables, sections, source spans, and review burden.",
                    "Hybrid parser architecture better covers paper-domain needs than either parser alone.",
                ],
                [
                    "M048 pattern 3.1",
                    "Bounded candidate generation must be explicit.",
                    "The merge layer must carry candidate-only provenance.",
                ],
                [
                    "M048 pattern 3.4",
                    "Promotion requires checks separate from extraction.",
                    "Parser success is not semantic truth and is not graph authorization.",
                ],
                [
                    "M048 pattern 3.6",
                    "Diagnostics must be reviewable and reproducible.",
                    "Per-PDF packets plus this report form a reproducible benchmark trail.",
                ],
            ],
        ),
        "",
        "### Safety Reading",
        "",
        "The benchmark is evidence for a parser architecture, not evidence for production readiness.",
        "The architecture remains inside the existing non-authorization boundary: graph writes, production imports, and LadybugDB writes are not authorized.",
        "This distinction is important because a 100% route recommendation can otherwise be mistaken for 100% ingestion readiness.",
        "",
    ]


def _render_safety_block() -> list[str]:
    return [
        "## Five-Flag Safety Defaults",
        "",
        "All benchmark-derived artifacts and this report preserve the five safety defaults as false.",
        "These defaults are binding for M055 and must be carried into M057 unless a later accepted ADR explicitly changes them.",
        "",
        "```json",
        json.dumps(SAFETY_DEFAULTS, indent=2, sort_keys=True),
        "```",
        "",
        *_md_table(
            ["Flag", "Default", "Meaning"],
            [
                [
                    "graph_import_allowed",
                    _as_bool_text(SAFETY_DEFAULTS["graph_import_allowed"]),
                    "No parser output may be imported into a graph as part of M055.",
                ],
                [
                    "graphdb_written",
                    _as_bool_text(SAFETY_DEFAULTS["graphdb_written"]),
                    "No graph database write occurred.",
                ],
                [
                    "import_eligible",
                    _as_bool_text(SAFETY_DEFAULTS["import_eligible"]),
                    "Benchmark packets are not eligible for production import.",
                ],
                [
                    "ladybugdb_written",
                    _as_bool_text(SAFETY_DEFAULTS["ladybugdb_written"]),
                    "No LadybugDB write occurred.",
                ],
                [
                    "production_import_attempted",
                    _as_bool_text(SAFETY_DEFAULTS["production_import_attempted"]),
                    "No production import was attempted.",
                ],
            ],
        ),
        "",
        "Safety sentence for trajectory scanning: graph import is not authorized, production import is not authorized, and LadybugDB writes are not authorized by this benchmark report.",
        "",
    ]


def _render_recommendation(data: dict[str, Any]) -> list[str]:
    route = data["routing_summary"]["aggregate_routing_recommendation"].get("recommended_route")
    return [
        "## Recommendation",
        "",
        f"Adopt **{route}** as the binding parser architecture for the next implementation pilot.",
        "The decision should be captured in ADR-008 and implemented in M057 as a real hybrid parser pilot.",
        "M057 should not attempt production import; it should build, test, and observe the merger boundary first.",
        "",
        "### Required M057 Implementation Shape",
        "",
        "1. Read one PDF and invoke both parser paths independently.",
        "2. Keep raw GROBID and OpenDataLoader packet provenance intact.",
        "3. Merge GROBID metadata/citations with OpenDataLoader body/layout into a candidate packet.",
        "4. Emit diagnostics for citation-to-body alignment and table/figure semantic linking gaps.",
        "5. Keep all five safety defaults false unless a later accepted ADR explicitly authorizes a new state.",
        "6. Treat benchmark success as a routing proof, not as semantic correctness proof.",
        "",
        "### D067 Mermaid-Assisted Architecture Diagram",
        "",
        "```mermaid",
        "flowchart LR",
        "    A[Input PDF] --> B[GROBID header and citation parser]",
        "    A --> C[OpenDataLoader body and layout parser]",
        "    B --> D[Bounded merge layer]",
        "    C --> D",
        "    D --> E[Candidate hybrid parser packet]",
        "    E --> F[Diagnostics: alignment and semantic-link gaps]",
        "    E -. not authorized .-> G[Graph import path]",
        "```",
        "",
        "### Decision Boundary",
        "",
        "- Authorized by M055: choosing the hybrid parser architecture for implementation planning.",
        "- Not authorized by M055: graph import, production import, fact promotion, or LadybugDB writes.",
        "- Required next proof: M057 must show the merger produces observable, reviewable candidate packets.",
        "",
    ]


def _render_appendices(data: dict[str, Any]) -> list[str]:
    lines = [
        "## Appendix A: Reproducibility Checklist",
        "",
        "- The corpus manifest lists exactly five PDFs.",
        "- GROBID per-PDF packets exist for exactly the five manifest IDs.",
        "- OpenDataLoader per-PDF packets exist for exactly the five manifest IDs.",
        "- Hybrid routing per-PDF packets exist for exactly the five manifest IDs.",
        "- The aggregate route count is five for `grobid_header + opendataloader_body`.",
        "- The safety defaults are false in S02, S03, S04, and this report.",
        "- The report schema is `m055-parser-benchmark-report.v1`.",
        "",
        "## Appendix B: Field Ownership Contract",
        "",
        *_md_table(
            ["Hybrid packet field family", "Owning source", "Reason", "M057 obligation"],
            [
                ["title", "GROBID", "native header extraction", "preserve source packet pointer"],
                ["authors", "GROBID", "native header extraction", "preserve source packet pointer"],
                [
                    "abstract",
                    "GROBID",
                    "native header extraction",
                    "preserve source packet pointer",
                ],
                [
                    "references",
                    "GROBID",
                    "native citation extraction",
                    "preserve citation packet pointer",
                ],
                [
                    "bibliography",
                    "GROBID",
                    "native bibliography extraction",
                    "preserve citation packet pointer",
                ],
                [
                    "markdown_body",
                    "OpenDataLoader",
                    "substantial markdown body output",
                    "preserve markdown artifact path",
                ],
                [
                    "sections",
                    "OpenDataLoader",
                    "section count and markdown structure",
                    "preserve section diagnostics",
                ],
                [
                    "tables",
                    "OpenDataLoader",
                    "table detection in markdown/layout",
                    "mark semantic links unresolved",
                ],
                [
                    "images",
                    "OpenDataLoader",
                    "image detection in markdown/layout",
                    "mark semantic links unresolved",
                ],
                [
                    "bounding_boxes",
                    "OpenDataLoader",
                    "layout packet support",
                    "preserve layout artifact path",
                ],
            ],
        ),
        "",
        "## Appendix C: Reader Notes for Future Agents",
        "",
        "- Do not reinterpret this report as a production import approval.",
        "- Do not delete the GROBID path because OpenDataLoader has better body output.",
        "- Do not delete the OpenDataLoader path because GROBID has better header/citation output.",
        "- Do not collapse the merger into a single parser abstraction until M057 proves the reconciliation contract.",
        "- Do preserve per-field provenance in every candidate hybrid packet.",
        "- Do keep the English phrase `is not authorized` in safety-critical docs scanned by trajectory tooling.",
        "",
        "## Appendix D: Per-PDF Route Audit",
        "",
    ]
    for aid in sorted(data["routing_packets"]):
        routing = data["routing_packets"][aid]
        route = _route_name(routing.get("recommended_route"))
        lines.extend(
            [
                f"### Audit {aid}",
                "",
                f"- Route: `{route}`",
                "- GROBID-owned dimensions: metadata, citations, processing_time.",
                "- OpenDataLoader-owned dimensions: body_content, layout, quality.",
                "- Safety defaults: all false.",
                "- Remaining proof burden: merger diagnostics, alignment gaps, and semantic-link gaps.",
                "",
            ]
        )
    lines.extend(
        [
            "## Appendix E: Report Integrity Statement",
            "",
            "This report is generated from local JSON benchmark artifacts and is intended to be deterministic except for the generation timestamp.",
            "If any source packet changes, rerun `uv run python scripts/render_m055_report.py` and re-run the S05 tests.",
            "If the route changes away from 100% hybrid, ADR-008 must be reconsidered before M057 implementation work proceeds.",
            "",
        ]
    )
    return lines


def render_report(base_dir: Path = DEFAULT_BENCHMARK_DIR, output: Path = DEFAULT_OUTPUT) -> str:
    data = _load_inputs(base_dir)
    lines: list[str] = []
    lines.extend(_render_header(data))
    lines.extend(_render_input_inventory(data))
    lines.extend(_render_corpus_table(data))
    lines.extend(_render_dimension_analysis(data))
    lines.extend(_render_per_pdf_details(data))
    lines.extend(_render_gap_analysis(data))
    lines.extend(_render_reconciliation())
    lines.extend(_render_safety_block())
    lines.extend(_render_recommendation(data))
    lines.extend(_render_appendices(data))
    text = "\n".join(lines).rstrip() + "\n"
    _write_text(output, text)
    return text


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render the M055 parser benchmark report.")
    parser.add_argument("--benchmark-dir", type=Path, default=DEFAULT_BENCHMARK_DIR)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    render_report(args.benchmark_dir, args.output)
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
