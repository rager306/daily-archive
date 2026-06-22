#!/usr/bin/env python3
"""Analyze M056 Wave 6 parser outputs and final 1-hop BFS connectivity.

The analysis is evidence-only: it reads Wave 1-5 analyses, Wave 1-6 manifests,
the prior corpus manifest, the anchor PDF, and Wave 6 parser packets, then
writes final markdown/JSON summaries. It does not write or import graph data;
all safety defaults remain false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m056-bfs-wave-6-analysis.v1"
DEFAULT_WAVE_1_DIR = Path("artifacts/m056-bfs-graph/wave-1")
DEFAULT_WAVE_2_DIR = Path("artifacts/m056-bfs-graph/wave-2")
DEFAULT_WAVE_3_DIR = Path("artifacts/m056-bfs-graph/wave-3")
DEFAULT_WAVE_4_DIR = Path("artifacts/m056-bfs-graph/wave-4")
DEFAULT_WAVE_5_DIR = Path("artifacts/m056-bfs-graph/wave-5")
DEFAULT_WAVE_6_DIR = Path("artifacts/m056-bfs-graph/wave-6")
DEFAULT_EXISTING_CORPUS = Path("artifacts/m055deep-parser-benchmark/corpus-manifest-20.json")
DEFAULT_ANCHOR_ARXIV_ID = "2605.18747"
DEFAULT_ANCHOR_PDF = Path(
    "data/article_catalog/article_catalog/arxiv/cs-cl/2605.18747/source/2605.18747.pdf"
)
DEFAULT_WAVE_ORDER = Path("/tmp/wave-order.json")
ARXIV_ID_RE = re.compile(r"\b\d{4}\.\d{4,5}(?:v\d+)?\b")
PDF_PAGE_RE = re.compile(rb"/Type\s*/Page\b")


def _utc_now() -> str:
    return dt.datetime.now(dt.UTC).isoformat()


def _safety_defaults() -> dict[str, bool]:
    return {
        "external_mutation_allowed": False,
        "facts_promoted": False,
        "graph_write_allowed": False,
        "production_import_attempted": False,
        "promotion_allowed": False,
    }


def _atomic_write_text(path: Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(payload, encoding="utf-8")
    tmp.replace(path)


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    _atomic_write_text(path, json.dumps(payload, indent=2, sort_keys=True) + "\n")


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _load_manifest_pdfs(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    payload = _load_json(path)
    pdfs = payload.get("pdfs", [])
    return [item for item in pdfs if isinstance(item, dict)]


def _load_packets(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    return [_load_json(packet_path) for packet_path in sorted(path.glob("*.json"))]


def _packet_status(packet: dict[str, Any]) -> str:
    return str(packet.get("status") or packet.get("quality_status") or "unknown")


def _quality_counts(packets: list[dict[str, Any]]) -> dict[str, int]:
    return dict(sorted(Counter(_packet_status(packet) for packet in packets).items()))


def _success_count(packets: list[dict[str, Any]]) -> int:
    return sum(1 for packet in packets if _packet_status(packet) == "success")


def _all_false(value: object) -> bool:
    return isinstance(value, dict) and bool(value) and all(item is False for item in value.values())


def _packet_safety_defaults_false(packets: list[dict[str, Any]]) -> bool:
    return all(_all_false(packet.get("safety_defaults")) for packet in packets)


def _tei_path(packet: dict[str, Any]) -> Path | None:
    value = packet.get("tei_path") or packet.get("raw_tei_path")
    if not isinstance(value, str) or not value:
        return None
    path = Path(value)
    return path if path.exists() else None


def _extract_arxiv_refs_from_tei(path: Path | None) -> set[str]:
    if path is None:
        return set()
    try:
        text = " ".join(ET.parse(path).getroot().itertext())
    except ET.ParseError:
        text = path.read_text(encoding="utf-8", errors="ignore")
    return set(ARXIV_ID_RE.findall(text))


def _extract_authors_from_tei(path: Path | None) -> list[str]:
    if path is None:
        return []
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError:
        return []
    authors: list[str] = []
    for element in root.iter():
        tag = element.tag.rsplit("}", 1)[-1]
        if tag not in {"author", "persName"}:
            continue
        text = " ".join(part.strip() for part in element.itertext() if part and part.strip())
        if text:
            authors.append(text)
    return authors


def _edge_records(
    packets: list[dict[str, Any]], target_arxiv_ids: set[str]
) -> list[dict[str, str]]:
    edges: set[tuple[str, str]] = set()
    for packet in packets:
        source = packet.get("arxiv_id")
        if not isinstance(source, str):
            continue
        for target in _extract_arxiv_refs_from_tei(_tei_path(packet)):
            if target in target_arxiv_ids and target != source:
                edges.add((source, target))
    return [
        {"source_arxiv_id": source, "target_arxiv_id": target} for source, target in sorted(edges)
    ]


def _edge_key(edge: dict[str, str]) -> tuple[str, str]:
    return (edge["source_arxiv_id"], edge["target_arxiv_id"])


def _sorted_edges(edges: list[dict[str, str]]) -> list[dict[str, str]]:
    return sorted(edges, key=_edge_key)


def _dedupe_pdfs(groups: list[list[dict[str, Any]]]) -> list[dict[str, Any]]:
    deduped: dict[str, dict[str, Any]] = {}
    for group in groups:
        for pdf in group:
            arxiv_id = pdf.get("arxiv_id") or pdf.get("requested_arxiv_id")
            if not isinstance(arxiv_id, str) or not arxiv_id:
                continue
            deduped[arxiv_id] = dict(pdf)
            deduped[arxiv_id]["arxiv_id"] = arxiv_id
    return [deduped[arxiv_id] for arxiv_id in sorted(deduped)]


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _pages_estimate(path: Path) -> int:
    try:
        return len(PDF_PAGE_RE.findall(path.read_bytes()))
    except OSError:
        return 0


def _anchor_pdf_entry(anchor_arxiv_id: str, anchor_pdf: Path) -> dict[str, Any] | None:
    if not anchor_pdf.exists():
        return None
    return {
        "arxiv_id": anchor_arxiv_id,
        "category": "cs-cl",
        "path": str(anchor_pdf),
        "sha256": _sha256_file(anchor_pdf),
        "size_bytes": anchor_pdf.stat().st_size,
        "pages_estimate": _pages_estimate(anchor_pdf),
        "source_milestone": "M056-lchpnp/anchor",
    }


def _wave_order_ids(path: Path) -> list[str]:
    if not path.exists():
        return []
    payload = json.loads(path.read_text(encoding="utf-8"))
    return [item for item in payload if isinstance(item, str)]


def _render_counts(counts: dict[str, int]) -> str:
    if not counts:
        return "none"
    return ", ".join(f"{key}: {value}" for key, value in sorted(counts.items()))


def _render_edges(edges: list[dict[str, str]]) -> str:
    if not edges:
        return "- none"
    return "\n".join(f"- {edge['source_arxiv_id']} -> {edge['target_arxiv_id']}" for edge in edges)


def analyze_wave_6(
    *,
    wave_1_dir: Path,
    wave_2_dir: Path,
    wave_3_dir: Path,
    wave_4_dir: Path,
    wave_5_dir: Path,
    wave_6_dir: Path,
    existing_corpus: Path,
    anchor_arxiv_id: str,
    anchor_pdf: Path,
    wave_order: Path,
) -> dict[str, Any]:
    existing_pdfs = _load_manifest_pdfs(existing_corpus)
    wave_pdfs_by_wave = {
        "wave_1": _load_manifest_pdfs(wave_1_dir / "corpus-manifest.json"),
        "wave_2": _load_manifest_pdfs(wave_2_dir / "corpus-manifest.json"),
        "wave_3": _load_manifest_pdfs(wave_3_dir / "corpus-manifest.json"),
        "wave_4": _load_manifest_pdfs(wave_4_dir / "corpus-manifest.json"),
        "wave_5": _load_manifest_pdfs(wave_5_dir / "corpus-manifest.json"),
        "wave_6": _load_manifest_pdfs(wave_6_dir / "corpus-manifest.json"),
    }
    wave_6_acquisition = _load_json(wave_6_dir / "acquisition-log.json")
    wave_5_analysis = _load_json(wave_5_dir / "analysis.json")
    wave_6_grobid_packets = _load_packets(wave_6_dir / "grobid-fulltext" / "per-pdf")
    wave_6_opendataloader_packets = _load_packets(wave_6_dir / "opendataloader" / "per-pdf")

    target_arxiv_ids = {
        str(pdf.get("arxiv_id") or pdf.get("requested_arxiv_id"))
        for pdf in existing_pdfs
        if pdf.get("arxiv_id") or pdf.get("requested_arxiv_id")
    }
    target_arxiv_ids.add(anchor_arxiv_id)

    previous_connectivity = wave_5_analysis.get("connectivity", {})
    previous_cumulative_edges = previous_connectivity.get("cumulative_edges", [])
    wave_6_edges = _edge_records(wave_6_grobid_packets, target_arxiv_ids)
    prior_edge_keys = {_edge_key(edge) for edge in previous_cumulative_edges}
    wave_6_new_edges = [edge for edge in wave_6_edges if _edge_key(edge) not in prior_edge_keys]
    cumulative_edges = _sorted_edges([*previous_cumulative_edges, *wave_6_new_edges])

    anchor_entry = _anchor_pdf_entry(anchor_arxiv_id, anchor_pdf)
    one_hop_pdfs = _dedupe_pdfs(list(wave_pdfs_by_wave.values()))
    one_hop_with_anchor = _dedupe_pdfs([one_hop_pdfs, [anchor_entry] if anchor_entry else []])
    evidence_corpus_pdfs = _dedupe_pdfs(
        [existing_pdfs, one_hop_pdfs, [anchor_entry] if anchor_entry else []]
    )

    cumulative_corpus_path = wave_6_dir / "cumulative-corpus.json"
    cumulative_corpus_payload = {
        "schema_version": "m056-bfs-wave-6-cumulative-corpus.v1",
        "generated_at": _utc_now(),
        "source": "M056-lchpnp S06 Wave 6 final 1-hop evidence corpus",
        "expected_wave_entries": 166,
        "wave_entry_count": sum(len(group) for group in wave_pdfs_by_wave.values()),
        "one_hop_unique_pdf_count": len(one_hop_pdfs),
        "one_hop_unique_with_anchor_count": len(one_hop_with_anchor),
        "evidence_corpus_unique_pdf_count": len(evidence_corpus_pdfs),
        "safety_defaults": _safety_defaults(),
        "pdfs": evidence_corpus_pdfs,
    }
    _atomic_write_json(cumulative_corpus_path, cumulative_corpus_payload)

    anchor_first_author = str(
        wave_5_analysis.get("self_citation_cluster", {}).get("anchor_first_author") or "Xuying Ning"
    )
    matching_self_citations = []
    for packet in wave_6_grobid_packets:
        arxiv_id = packet.get("arxiv_id")
        authors = _extract_authors_from_tei(_tei_path(packet))
        if isinstance(arxiv_id, str) and any(
            anchor_first_author.lower() in author.lower() for author in authors
        ):
            matching_self_citations.append(arxiv_id)

    parser_quality = {
        "grobid_packet_count": len(wave_6_grobid_packets),
        "grobid_success_count": _success_count(wave_6_grobid_packets),
        "grobid_quality_counts": _quality_counts(wave_6_grobid_packets),
        "opendataloader_packet_count": len(wave_6_opendataloader_packets),
        "opendataloader_success_count": _success_count(wave_6_opendataloader_packets),
        "opendataloader_quality_counts": _quality_counts(wave_6_opendataloader_packets),
        "packet_safety_defaults_all_false": _packet_safety_defaults_false(wave_6_grobid_packets)
        and _packet_safety_defaults_false(wave_6_opendataloader_packets),
    }
    previous_by_wave = dict(previous_connectivity.get("edge_saturation_by_wave", {}))
    edge_saturation_by_wave = {**previous_by_wave, "wave_6": len(wave_6_new_edges)}
    previous_wave_5_edges = int(previous_by_wave.get("wave_5", 0))
    if previous_wave_5_edges == 0 and len(wave_6_new_edges) == 0:
        saturation_status = "final-saturated"
    elif len(wave_6_new_edges) == 0:
        saturation_status = "saturated"
    else:
        saturation_status = "expanded"

    recommendation = {
        "decision": "2-hop needed for graph-readiness; accept 1-hop as benchmark-only evidence",
        "rationale": (
            "The final two waves added zero new target-set edges after sparse cumulative connectivity, "
            "so the 1-hop corpus is operationally complete but not graph-ready."
        ),
    }
    category_distribution = dict(
        sorted(
            Counter(pdf.get("category", "unknown") for pdf in wave_pdfs_by_wave["wave_6"]).items()
        )
    )
    length_distribution = dict(
        sorted(
            Counter(
                "0-pages"
                if int(pdf.get("pages_estimate") or 0) <= 0
                else "1-10"
                if int(pdf.get("pages_estimate") or 0) <= 10
                else "11-25"
                if int(pdf.get("pages_estimate") or 0) <= 25
                else "26+"
                for pdf in wave_pdfs_by_wave["wave_6"]
            ).items()
        )
    )
    wave_order_ids = _wave_order_ids(wave_order)

    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "inputs": {
            "wave_1_dir": str(wave_1_dir),
            "wave_2_dir": str(wave_2_dir),
            "wave_3_dir": str(wave_3_dir),
            "wave_4_dir": str(wave_4_dir),
            "wave_5_dir": str(wave_5_dir),
            "wave_6_dir": str(wave_6_dir),
            "existing_corpus": str(existing_corpus),
            "anchor_pdf": str(anchor_pdf),
            "wave_order": str(wave_order),
        },
        "safety_defaults": _safety_defaults(),
        "acquisition": {
            "success_count": wave_6_acquisition.get("success_count", 0),
            "blocked_count": wave_6_acquisition.get("blocked_count", 0),
            "network_error_count": wave_6_acquisition.get("network_error_count", 0),
            "status_counts": wave_6_acquisition.get("status_counts", {}),
            "requested_arxiv_ids": wave_6_acquisition.get("requested_arxiv_ids", []),
        },
        "parser_quality": parser_quality,
        "connectivity": {
            "anchor_arxiv_id": anchor_arxiv_id,
            "existing_corpus_target_count": len(existing_pdfs),
            "target_count": len(target_arxiv_ids),
            "wave_1_edge_count": previous_connectivity.get("wave_1_edge_count", 0),
            "wave_1_edges": previous_connectivity.get("wave_1_edges", []),
            "wave_2_new_edge_count": previous_connectivity.get("wave_2_new_edge_count", 0),
            "wave_2_new_edges": previous_connectivity.get("wave_2_new_edges", []),
            "wave_3_new_edge_count": previous_connectivity.get("wave_3_new_edge_count", 0),
            "wave_3_new_edges": previous_connectivity.get("wave_3_new_edges", []),
            "wave_4_new_edge_count": previous_connectivity.get("wave_4_new_edge_count", 0),
            "wave_4_new_edges": previous_connectivity.get("wave_4_new_edges", []),
            "wave_5_new_edge_count": previous_connectivity.get("wave_5_new_edge_count", 0),
            "wave_5_new_edges": previous_connectivity.get("wave_5_new_edges", []),
            "wave_6_new_edge_count": len(wave_6_new_edges),
            "wave_6_new_edges": _sorted_edges(wave_6_new_edges),
            "connectivity_gain_delta_vs_wave_5": len(wave_6_new_edges) - previous_wave_5_edges,
            "cumulative_edge_count": len(cumulative_edges),
            "cumulative_edges": cumulative_edges,
            "edge_saturation_by_wave": edge_saturation_by_wave,
            "saturation_status": saturation_status,
        },
        "final_1hop": {
            "wave_order_entry_count": len(wave_order_ids),
            "wave_order_unique_count": len(set(wave_order_ids)),
            "anchor_present_in_wave_order": anchor_arxiv_id in set(wave_order_ids),
            "acquired_wave_entry_count": sum(len(group) for group in wave_pdfs_by_wave.values()),
            "acquired_wave_unique_pdf_count": len(one_hop_pdfs),
            "total_unique_pdfs_with_anchor": len(one_hop_with_anchor),
            "evidence_corpus_unique_pdf_count": len(evidence_corpus_pdfs),
            "cumulative_corpus_path": str(cumulative_corpus_path),
            "recommendation": recommendation,
        },
        "self_citation_cluster": {
            "anchor_first_author": anchor_first_author,
            "matching_wave_6_pdfs": sorted(matching_self_citations),
            "matching_wave_6_count": len(matching_self_citations),
            "wave_6_pdf_count": len(wave_pdfs_by_wave["wave_6"]),
        },
        "category_distribution": category_distribution,
        "length_distribution": length_distribution,
    }


def render_markdown(analysis: dict[str, Any]) -> str:
    acquisition = analysis["acquisition"]
    parser_quality = analysis["parser_quality"]
    connectivity = analysis["connectivity"]
    final_1hop = analysis["final_1hop"]
    self_cluster = analysis["self_citation_cluster"]
    matching_count = int(self_cluster["matching_wave_6_count"])
    wave_pdf_count = max(1, int(self_cluster["wave_6_pdf_count"]))
    matching_percent = matching_count / wave_pdf_count * 100
    recommendation = final_1hop["recommendation"]
    return f"""# M056 Wave 6 Final 1-hop Analysis

Generated: `{analysis["generated_at"]}`

## Safety

- Graph writes: false
- Production import attempted: false
- Promotion allowed: false
- Facts promoted: false
- External mutation allowed: false
- This evidence is not authorized for graph import or fact promotion.

## Acquisition

- Requested refs: {len(acquisition["requested_arxiv_ids"])}
- Success: {acquisition["success_count"]}
- Blocked: {acquisition["blocked_count"]}
- Network errors: {acquisition["network_error_count"]}
- Status counts: {_render_counts(acquisition["status_counts"])}

## Parser quality

- GROBID packets: {parser_quality["grobid_packet_count"]}
- GROBID success: {parser_quality["grobid_success_count"]}
- GROBID quality counts: {_render_counts(parser_quality["grobid_quality_counts"])}
- OpenDataLoader packets: {parser_quality["opendataloader_packet_count"]}
- OpenDataLoader success: {parser_quality["opendataloader_success_count"]}
- OpenDataLoader quality counts: {_render_counts(parser_quality["opendataloader_quality_counts"])}
- Packet safety defaults all false: {parser_quality["packet_safety_defaults_all_false"]}

## Connectivity gain

- Target set: {connectivity["existing_corpus_target_count"]} existing corpus PDFs + anchor `{connectivity["anchor_arxiv_id"]}`
- Wave 1 directed edges to target set: {connectivity["wave_1_edge_count"]}
- Wave 2 new directed edges to target set: {connectivity["wave_2_new_edge_count"]}
- Wave 3 new directed edges to target set: {connectivity["wave_3_new_edge_count"]}
- Wave 4 new directed edges to target set: {connectivity["wave_4_new_edge_count"]}
- Wave 5 new directed edges to target set: {connectivity["wave_5_new_edge_count"]}
- Wave 6 new directed edges to target set: {connectivity["wave_6_new_edge_count"]}
- Delta vs Wave 5: {connectivity["connectivity_gain_delta_vs_wave_5"]}
- Cumulative directed edges: {connectivity["cumulative_edge_count"]}
- Final saturation status: {connectivity["saturation_status"]}

### Wave 6 new edges

{_render_edges(connectivity["wave_6_new_edges"])}

### Edge saturation by wave

- {_render_counts(connectivity["edge_saturation_by_wave"])}

## Final 1-hop corpus accounting

- Wave-order entries: {final_1hop["wave_order_entry_count"]}
- Wave-order unique IDs: {final_1hop["wave_order_unique_count"]}
- Anchor present in wave-order: {final_1hop["anchor_present_in_wave_order"]}
- Acquired wave entries: {final_1hop["acquired_wave_entry_count"]}
- Acquired unique wave PDFs: {final_1hop["acquired_wave_unique_pdf_count"]}
- Total unique PDFs with anchor: {final_1hop["total_unique_pdfs_with_anchor"]}
- Evidence corpus unique PDFs including prior target corpus: {final_1hop["evidence_corpus_unique_pdf_count"]}
- Cumulative corpus path: `{final_1hop["cumulative_corpus_path"]}`

## Final recommendation

- Decision: {recommendation["decision"]}
- Rationale: {recommendation["rationale"]}

## Self-citation cluster

- Anchor first author: {self_cluster["anchor_first_author"]}
- Matching Wave 6 PDFs: {matching_count} / {self_cluster["wave_6_pdf_count"]} ({matching_percent:.1f}%)

## Category distribution

- {_render_counts(analysis["category_distribution"])}

## Length distribution

- {_render_counts(analysis["length_distribution"])}
"""


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wave-1-dir", type=Path, default=DEFAULT_WAVE_1_DIR)
    parser.add_argument("--wave-2-dir", type=Path, default=DEFAULT_WAVE_2_DIR)
    parser.add_argument("--wave-3-dir", type=Path, default=DEFAULT_WAVE_3_DIR)
    parser.add_argument("--wave-4-dir", type=Path, default=DEFAULT_WAVE_4_DIR)
    parser.add_argument("--wave-5-dir", type=Path, default=DEFAULT_WAVE_5_DIR)
    parser.add_argument("--wave-6-dir", type=Path, default=DEFAULT_WAVE_6_DIR)
    parser.add_argument("--existing-corpus", type=Path, default=DEFAULT_EXISTING_CORPUS)
    parser.add_argument("--anchor-arxiv-id", default=DEFAULT_ANCHOR_ARXIV_ID)
    parser.add_argument("--anchor-pdf", type=Path, default=DEFAULT_ANCHOR_PDF)
    parser.add_argument("--wave-order", type=Path, default=DEFAULT_WAVE_ORDER)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    analysis = analyze_wave_6(
        wave_1_dir=args.wave_1_dir,
        wave_2_dir=args.wave_2_dir,
        wave_3_dir=args.wave_3_dir,
        wave_4_dir=args.wave_4_dir,
        wave_5_dir=args.wave_5_dir,
        wave_6_dir=args.wave_6_dir,
        existing_corpus=args.existing_corpus,
        anchor_arxiv_id=args.anchor_arxiv_id,
        anchor_pdf=args.anchor_pdf,
        wave_order=args.wave_order,
    )
    _atomic_write_json(args.wave_6_dir / "analysis.json", analysis)
    _atomic_write_text(args.wave_6_dir / "analysis.md", render_markdown(analysis))
    print(
        json.dumps(
            {
                "wave_6_new_edges": analysis["connectivity"]["wave_6_new_edge_count"],
                "cumulative_edges": analysis["connectivity"]["cumulative_edge_count"],
                "total_unique_pdfs_with_anchor": analysis["final_1hop"][
                    "total_unique_pdfs_with_anchor"
                ],
                "recommendation": analysis["final_1hop"]["recommendation"]["decision"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
