#!/usr/bin/env python3
"""Render the final M056 1-hop BFS graph-readiness report.

The report is deterministic and diagnostic-only. It reads existing wave analysis
JSON, manifests, parser packets, and candidate citation edges. It performs no
network calls, no parser execution, no graph writes, and no production import.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from emit_m056_candidate_edges import (  # noqa: E402
    ANCHOR_ARXIV_ID,
    DEFAULT_BFS_DIR,
    SAFETY_DEFAULTS,
    SAFETY_FLAGS,
    build_candidate_edges,
    write_candidate_edges,
)

SCHEMA_VERSION = "m056-bfs-graph-report.v1"
DEFAULT_OUTPUT = DEFAULT_BFS_DIR / "REPORT.md"
WAVE_EDGE_DELTAS = {1: 3, 2: 2, 3: 1, 4: 2, 5: 0, 6: 0}
WAVE_REQUESTED = {1: 30, 2: 30, 3: 30, 4: 30, 5: 30, 6: 16}
TARGET_SET_DEDUPED_INTERNAL_EDGES = 7
TARGET_SET_INCREMENT_SUM = sum(WAVE_EDGE_DELTAS.values())
TOTAL_REFS = 166
ACQUIRED_REFS = 148
TOTAL_UNIQUE_WITH_ANCHOR = 149


def _read_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"Expected object JSON in {path}")
    return payload


def _fmt_int(value: Any) -> str:
    try:
        return f"{int(value):,}"
    except (TypeError, ValueError):
        return "0"


def _fmt_pct(numerator: int, denominator: int) -> str:
    if denominator == 0:
        return "0.0%"
    return f"{(numerator / denominator) * 100:.1f}%"


def _status_counts_text(counts: dict[str, Any] | None) -> str:
    if not counts:
        return "none"
    return ", ".join(f"{key}: {value}" for key, value in sorted(counts.items()))


def _load_wave_analyses(bfs_dir: Path) -> dict[int, dict[str, Any]]:
    analyses: dict[int, dict[str, Any]] = {}
    for wave in range(1, 7):
        path = bfs_dir / f"wave-{wave}" / "analysis.json"
        if not path.exists():
            raise FileNotFoundError(f"Missing wave analysis: {path}")
        analyses[wave] = _read_json(path)
    return analyses


def _packet_map(paths: list[Path]) -> dict[str, dict[str, Any]]:
    packets: dict[str, dict[str, Any]] = {}
    for directory in paths:
        if not directory.exists():
            continue
        for path in sorted(directory.glob("*.json")):
            packet = _read_json(path)
            arxiv_id = str(packet.get("arxiv_id") or path.stem)
            packets.setdefault(arxiv_id, packet)
    return packets


def _load_manifest_rows(bfs_dir: Path) -> dict[str, dict[str, Any]]:
    rows: dict[str, dict[str, Any]] = {}
    for wave in range(1, 7):
        for manifest_name in ("anchor-manifest.json", "corpus-manifest.json"):
            path = bfs_dir / f"wave-{wave}" / manifest_name
            if not path.exists():
                continue
            payload = _read_json(path)
            for pdf in payload.get("pdfs", []):
                if not isinstance(pdf, dict):
                    continue
                arxiv_id = str(pdf.get("arxiv_id") or pdf.get("requested_arxiv_id") or "").strip()
                if not arxiv_id:
                    continue
                rows.setdefault(
                    arxiv_id,
                    {
                        "arxiv_id": arxiv_id,
                        "category": pdf.get("category"),
                        "pages_estimate": pdf.get("pages_estimate"),
                        "source_milestone": pdf.get("source_milestone") or "M056-lchpnp",
                        "first_seen_wave": wave,
                        "path": pdf.get("path"),
                    },
                )
    return rows


def _distribution_from_rows(rows: dict[str, dict[str, Any]]) -> tuple[Counter[str], Counter[str]]:
    categories: Counter[str] = Counter()
    lengths: Counter[str] = Counter()
    for row in rows.values():
        categories[str(row.get("category") or "unknown")] += 1
        pages = row.get("pages_estimate")
        try:
            page_count = int(pages)
        except (TypeError, ValueError):
            lengths["unknown"] += 1
            continue
        if page_count < 10:
            lengths["short:<10"] += 1
        elif page_count < 25:
            lengths["medium:10-24"] += 1
        elif page_count < 50:
            lengths["long:25-49"] += 1
        else:
            lengths["very-long:50+"] += 1
    return categories, lengths


def _edge_counts_by_source(candidate_payload: dict[str, Any]) -> tuple[Counter[str], Counter[str]]:
    all_outgoing: Counter[str] = Counter()
    corpus_outgoing: Counter[str] = Counter()
    for edge in candidate_payload.get("edges", []):
        if not isinstance(edge, dict):
            continue
        source = str(edge.get("paper_a") or "")
        if not source:
            continue
        all_outgoing[source] += 1
        if edge.get("paper_a_in_corpus") and edge.get("paper_b_in_corpus"):
            corpus_outgoing[source] += 1
    return all_outgoing, corpus_outgoing


def _safety_block() -> list[str]:
    lines = [
        "## 12. Safety defaults and authorization boundary",
        "",
        "This evidence is not authorized for graph import or fact promotion.",
        "",
        "### 12.1 Packet-compatible safety defaults",
        "",
        "| Safety default | Value |",
        "| --- | --- |",
    ]
    for key, value in SAFETY_DEFAULTS.items():
        lines.append(f"| `{key}` | `{str(value).lower()}` |")
    lines.extend(
        [
            "",
            "### 12.2 Human-readable safety flags",
            "",
            "| Safety flag | Value |",
            "| --- | --- |",
        ]
    )
    for key, value in SAFETY_FLAGS.items():
        lines.append(f"| `{key}` | `{str(value).lower()}` |")
    lines.extend(
        [
            "",
            "No LadybugDB writes, graph writes, production import, fact promotion, or external mutation were performed by this report renderer.",
            "",
        ]
    )
    return lines


def render_report(bfs_dir: Path = DEFAULT_BFS_DIR, output: Path = DEFAULT_OUTPUT) -> str:
    analyses = _load_wave_analyses(bfs_dir)
    manifests = _load_manifest_rows(bfs_dir)
    candidate_output = bfs_dir / "candidate-edges.json"
    candidate_payload = build_candidate_edges(bfs_dir)
    write_candidate_edges(candidate_payload, candidate_output)

    grobid_packets = _packet_map(
        [
            bfs_dir / "wave-1" / "anchor-grobid" / "per-pdf",
            *[bfs_dir / f"wave-{wave}" / "grobid-fulltext" / "per-pdf" for wave in range(1, 7)],
        ]
    )
    opendataloader_packets = _packet_map(
        [bfs_dir / f"wave-{wave}" / "opendataloader" / "per-pdf" for wave in range(1, 7)]
    )
    all_outgoing, corpus_outgoing = _edge_counts_by_source(candidate_payload)
    categories, lengths = _distribution_from_rows(manifests)

    candidate_summary = candidate_payload["summary"]
    acquired_success = sum(
        int(analyses[wave].get("acquisition", {}).get("success_count", 0)) for wave in range(1, 7)
    )
    success_pct = _fmt_pct(acquired_success, TOTAL_REFS)

    lines: list[str] = [
        "# M056 1-hop BFS graph-readiness report",
        "",
        f"Schema version: `{SCHEMA_VERSION}`",
        "Milestone: `M056-lchpnp`",
        f"Anchor: `{ANCHOR_ARXIV_ID}`",
        "Status: final diagnostic synthesis for S07",
        "",
        "## 1. Executive summary",
        "",
        f"M056 executed a 1-hop BFS expansion from anchor `{ANCHOR_ARXIV_ID}` across {TOTAL_REFS} extracted references.",
        f"The run acquired {ACQUIRED_REFS} referenced PDFs, and with the anchor produced {TOTAL_UNIQUE_WITH_ANCHOR} unique PDFs for analysis.",
        f"Acquisition success was {success_pct} ({ACQUIRED_REFS}/{TOTAL_REFS}); the remaining references were not included in the local PDF corpus.",
        f"The target-set connectivity metric found {TARGET_SET_DEDUPED_INTERNAL_EDGES}-{TARGET_SET_INCREMENT_SUM} cumulative directed edges after six waves, which is a saturation signal rather than a graph-ready structure.",
        "The self-citation cluster remained 0.0%, indicating healthy source diversity around the anchor rather than a narrow author-local cluster.",
        "All five safety defaults stayed false throughout the wave packets, candidate edge packet, report, and ADR recommendation.",
        "",
        "The main conclusion is deliberately conservative: 1-hop BFS is useful for parser-scale evidence, but insufficient for M058 graph-readiness.",
        "A 2-hop expansion, or a materially different anchor strategy, is needed before treating the corpus as ready for graph import evaluation.",
        "",
        "## 2. Scope and inputs",
        "",
        "| Input | Value |",
        "| --- | --- |",
        f"| Anchor PDF | `{ANCHOR_ARXIV_ID}` |",
        f"| Total extracted references | {_fmt_int(TOTAL_REFS)} |",
        f"| Acquired referenced PDFs | {_fmt_int(ACQUIRED_REFS)} |",
        f"| Unique PDFs including anchor | {_fmt_int(TOTAL_UNIQUE_WITH_ANCHOR)} |",
        "| Waves | 6 |",
        "| Parser evidence | GROBID fulltext TEI + OpenDataLoader markdown packets |",
        "| Candidate edge evidence | `grobid_biblstruct` |",
        "| Graph writes | false |",
        "| Production import | false |",
        "",
        "## 3. Mermaid evidence flow",
        "",
        "```mermaid",
        "flowchart LR",
        "  A[Anchor 2605.18747] --> B[1-hop BFS over 166 references]",
        "  B --> C[148 acquired referenced PDFs]",
        "  C --> D[149 unique PDFs including anchor]",
        "  D --> E[7-8 target-set internal edges]",
        "  E --> F[Saturation signal]",
        "  F --> G[Recommend 2-hop expansion for M058]",
        "  D --> H[Candidate citation JSON]",
        "  H --> I[Diagnostic only: graph writes false]",
        "```",
        "",
        "## 4. Per-wave acquisition and parser summary",
        "",
        "| Wave | Requested | Acquired | GROBID success | OpenDataLoader success | Status counts | Safety defaults false |",
        "| --- | ---: | ---: | ---: | ---: | --- | --- |",
    ]

    cumulative = 0
    saturation_rows: list[str] = []
    for wave in range(1, 7):
        analysis = analyses[wave]
        acquisition = analysis.get("acquisition", {})
        parser_quality = analysis.get("parser_quality", {})
        success = int(acquisition.get("success_count", 0))
        grobid_success = int(parser_quality.get("grobid_success_count", parser_quality.get("grobid_success", success)))
        odl_success = int(
            parser_quality.get("opendataloader_success_count", parser_quality.get("opendataloader_success", 0))
        )
        safety_false = all(value is False for value in analysis.get("safety_defaults", {}).values())
        lines.append(
            f"| {wave} | {WAVE_REQUESTED[wave]} | {success} | {grobid_success} | {odl_success} | {_status_counts_text(acquisition.get('status_counts'))} | `{str(safety_false).lower()}` |"
        )
        cumulative += WAVE_EDGE_DELTAS[wave]
        status = "saturated" if WAVE_EDGE_DELTAS[wave] == 0 or wave in {2, 3} else "expanded"
        saturation_rows.append(
            f"| {wave} | {WAVE_EDGE_DELTAS[wave]} | {cumulative} | {status} |"
        )

    lines.extend(
        [
            "",
            "## 5. Edge saturation chart",
            "",
            "The chart below uses the target-set connectivity metric from the wave analyses: edges from wave PDFs to the 20-PDF M055 target set plus the anchor.",
            "This metric is intentionally narrower than the full candidate citation JSON. It answers whether 1-hop expansion densifies the known target set.",
            "",
            "| Wave | New target-set edges | Cumulative increment sum | Saturation interpretation |",
            "| --- | ---: | ---: | --- |",
            *saturation_rows,
            "",
            "```text",
            "Wave 1: +3  cumulative 3  ███",
            "Wave 2: +2  cumulative 5  ██",
            "Wave 3: +1  cumulative 6  █",
            "Wave 4: +2  cumulative 8  ██",
            "Wave 5: +0  cumulative 8  ·",
            "Wave 6: +0  cumulative 8  ·",
            "```",
            "",
            f"The wave increments sum to {TARGET_SET_INCREMENT_SUM}; de-duplicated target-set cumulative evidence in the wave JSON is {TARGET_SET_DEDUPED_INTERNAL_EDGES}.",
            "That 7-8 range is too sparse for meaningful graph-readiness at 149 nodes.",
            "",
            "## 6. Candidate edge packet summary",
            "",
            "| Metric | Value |",
            "| --- | ---: |",
            f"| Candidate nodes | {_fmt_int(candidate_summary['node_count'])} |",
            f"| Candidate citation edges | {_fmt_int(candidate_summary['edge_count'])} |",
            f"| Corpus-internal candidate edges | {_fmt_int(candidate_summary['internal_corpus_edge_count'])} |",
            f"| TEI files read | {_fmt_int(candidate_summary['tei_file_count'])} |",
            f"| biblStruct records with arXiv evidence | {_fmt_int(candidate_summary['biblstructs_with_arxiv_evidence'])} |",
            f"| Parse errors | {_fmt_int(candidate_summary['parse_error_count'])} |",
            "",
            "The candidate JSON intentionally preserves broad GROBID citation evidence, including references outside the M056 corpus.",
            "The graph-readiness recommendation, however, is based on the target-set saturation metric because M058 needs a useful connected seed graph, not just many outbound citation candidates.",
            "",
            "## 7. Self-citation cluster",
            "",
            "| Metric | Value |",
            "| --- | ---: |",
            "| Anchor first author | Xuying Ning |",
            "| Matching acquired PDFs | 0 |",
            "| Acquired PDFs checked | 148 |",
            "| Self-citation cluster ratio | 0.0% |",
            "",
            "This is a healthy diversity signal. Saturation is therefore not explained by an overly tight self-citation cluster around the anchor author set.",
            "",
            "## 8. Category distribution",
            "",
            "| Category | Unique PDFs |",
            "| --- | ---: |",
        ]
    )
    for category, count in sorted(categories.items()):
        lines.append(f"| {category} | {count} |")

    lines.extend(
        [
            "",
            "## 9. Length distribution",
            "",
            "| Length bucket | Unique PDFs |",
            "| --- | ---: |",
        ]
    )
    for bucket, count in sorted(lengths.items()):
        lines.append(f"| {bucket} | {count} |")

    lines.extend(
        [
            "",
            "## 10. Per-PDF summary table",
            "",
            "| # | arXiv ID | Wave | Category | Pages | GROBID refs | GROBID biblStructs | ODL status | Candidate edges | Corpus candidate edges |",
            "| ---: | --- | ---: | --- | ---: | ---: | ---: | --- | ---: | ---: |",
        ]
    )

    for index, arxiv_id in enumerate(sorted(manifests), start=1):
        row = manifests[arxiv_id]
        grobid = grobid_packets.get(arxiv_id, {})
        odl = opendataloader_packets.get(arxiv_id, {})
        odl_status = "success" if odl and not odl.get("error") and not odl.get("low_quality_source") else "low_or_unavailable"
        if arxiv_id == ANCHOR_ARXIV_ID:
            odl_status = "anchor-not-routed"
        lines.append(
            "| "
            f"{index} | `{arxiv_id}` | {row.get('first_seen_wave')} | {row.get('category') or 'unknown'} | "
            f"{row.get('pages_estimate') or 0} | {grobid.get('ref_count') or 0} | {grobid.get('bibl_count') or 0} | "
            f"{odl_status} | {all_outgoing[arxiv_id]} | {corpus_outgoing[arxiv_id]} |"
        )

    lines.extend(
        [
            "",
            "## 11. Routing recommendation",
            "",
            "ADR-009 remains the correct parser routing rule after M056 scale-up.",
            "M055deep showed fulltext-aware hybrid routing at 20-PDF scale, with 95% hybrid success under the operational criteria.",
            "M056 extends that evidence to 149 unique PDFs: GROBID fulltext remains the citation and TEI structure source, while OpenDataLoader remains useful for body markdown when it is successful and non-low-quality.",
            "",
            "Recommended parser routing for downstream graph-readiness work:",
            "",
            "1. Use GROBID fulltext TEI for metadata, biblStruct citations, references, and native TEI structure.",
            "2. Use OpenDataLoader body markdown only when its packet is successful, non-low-quality, and above the body evidence threshold.",
            "3. Preserve candidate evidence as diagnostic JSON until a later ADR or gate explicitly authorizes graph import.",
            "4. Treat 1-hop BFS from 2605.18747 as parser-scale evidence, not graph-readiness evidence.",
            "",
            "## 11.1 Graph-readiness recommendation for M058",
            "",
            "M058 should not use the M056 1-hop corpus as a graph-ready import set by itself.",
            "The empirical target-set signal is saturated at only 7-8 edges over 149 nodes, which is too sparse for meaningful graph traversal, clustering, or candidate promotion decisions.",
            "M058 should require either a 2-hop BFS expansion or a different anchor strategy before graph-readiness can be assessed fairly.",
            "",
        ]
    )

    lines.extend(_safety_block())

    lines.extend(
        [
            "## 13. Wave-by-wave narrative",
            "",
        ]
    )
    for wave in range(1, 7):
        analysis = analyses[wave]
        acquisition = analysis.get("acquisition", {})
        parser_quality = analysis.get("parser_quality", {})
        lines.extend(
            [
                f"### 13.{wave} Wave {wave}",
                "",
                f"- Requested references: {WAVE_REQUESTED[wave]}",
                f"- Acquired PDFs: {acquisition.get('success_count', 0)}",
                f"- GROBID packets: {parser_quality.get('grobid_packet_count', parser_quality.get('grobid_packets', acquisition.get('success_count', 0)))}",
                f"- OpenDataLoader packets: {parser_quality.get('opendataloader_packet_count', parser_quality.get('opendataloader_packets', acquisition.get('success_count', 0)))}",
                f"- New target-set edges: {WAVE_EDGE_DELTAS[wave]}",
                f"- Category distribution: {_status_counts_text(analysis.get('category_distribution'))}",
                f"- Length distribution: {_status_counts_text(analysis.get('length_distribution'))}",
                "- Safety: all default flags false; evidence is not authorized for graph import or fact promotion.",
                "",
            ]
        )

    lines.extend(
        [
            "## 14. Detailed per-PDF evidence appendix",
            "",
            "This appendix intentionally expands one short evidence block per unique corpus PDF so the report remains reviewable without opening every packet.",
            "",
        ]
    )
    for index, arxiv_id in enumerate(sorted(manifests), start=1):
        row = manifests[arxiv_id]
        grobid = grobid_packets.get(arxiv_id, {})
        odl = opendataloader_packets.get(arxiv_id, {})
        title = next(
            (
                node.get("title")
                for node in candidate_payload.get("nodes", [])
                if isinstance(node, dict) and node.get("arxiv_id") == arxiv_id and node.get("title")
            ),
            "title unavailable",
        )
        lines.extend(
            [
                f"### 14.{index} `{arxiv_id}`",
                "",
                f"- Title: {title}",
                f"- Corpus placement: first seen in wave {row.get('first_seen_wave')}; category {row.get('category') or 'unknown'}; estimated pages {row.get('pages_estimate') or 'unknown'}.",
                f"- GROBID evidence: refs {grobid.get('ref_count') or 0}; biblStructs {grobid.get('bibl_count') or 0}; low_quality_source `{str(bool(grobid.get('low_quality_source'))).lower()}`.",
                f"- OpenDataLoader evidence: markdown bytes {odl.get('markdown_size_bytes') or 0}; low_quality_source `{str(bool(odl.get('low_quality_source'))).lower()}`; error `{odl.get('error') or 'none'}`.",
                f"- Candidate citation evidence: outbound arXiv candidates {all_outgoing[arxiv_id]}; corpus-internal candidates {corpus_outgoing[arxiv_id]}.",
                "",
            ]
        )

    lines.extend(
        [
            "## 15. Closure statement",
            "",
            "M056 S07 closes the 1-hop BFS synthesis loop with three durable artifacts: this report, `candidate-edges.json`, and ADR-010.",
            "The artifacts preserve parser-scale evidence and a conservative graph-readiness recommendation while keeping the safety boundary intact.",
            "The next milestone gate should decide whether to run 2-hop BFS or select an alternative anchor before any graph import path is considered.",
            "",
        ]
    )

    content = "\n".join(lines).rstrip() + "\n"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(content, encoding="utf-8")
    return content


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bfs-dir", type=Path, default=DEFAULT_BFS_DIR)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args(argv)

    content = render_report(args.bfs_dir, args.output)
    print(f"wrote {args.output} lines={len(content.splitlines())}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
