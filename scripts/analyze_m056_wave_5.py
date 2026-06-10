#!/usr/bin/env python3
"""Analyze M056 Wave 5 parser outputs and cumulative BFS connectivity.

The analysis is evidence-only: it reads acquisition, GROBID, OpenDataLoader,
Wave 1-4 analysis artifacts, Wave 1-5 manifests, and the prior corpus manifest,
then writes markdown/JSON summaries. It does not write or import graph data; all
safety defaults remain false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m056-bfs-wave-5-analysis.v1"
DEFAULT_WAVE_1_DIR = Path("artifacts/m056-bfs-graph/wave-1")
DEFAULT_WAVE_2_DIR = Path("artifacts/m056-bfs-graph/wave-2")
DEFAULT_WAVE_3_DIR = Path("artifacts/m056-bfs-graph/wave-3")
DEFAULT_WAVE_4_DIR = Path("artifacts/m056-bfs-graph/wave-4")
DEFAULT_WAVE_5_DIR = Path("artifacts/m056-bfs-graph/wave-5")
DEFAULT_EXISTING_CORPUS = Path("artifacts/m055deep-parser-benchmark/corpus-manifest-20.json")
DEFAULT_ANCHOR_ARXIV_ID = "2605.18747"
ARXIV_ID_RE = re.compile(r"(?<!\d)(\d{4}\.\d{4,5})(?:v\d+)?(?!\d)")
SUPPORTED_CATEGORIES = ("cs-ai", "cs-cl", "cs-cv", "cs-lg")


def _utc_now() -> str:
    return dt.datetime.now(tz=dt.timezone.utc).isoformat()


def _safety_defaults() -> dict[str, bool]:
    return {
        "graph_write_allowed": False,
        "production_import_attempted": False,
        "promotion_allowed": False,
        "facts_promoted": False,
        "external_mutation_allowed": False,
    }


def _load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _atomic_write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + ".tmp")
    tmp_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp_path.replace(path)


def _atomic_write_text(path: Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + ".tmp")
    tmp_path.write_text(payload, encoding="utf-8")
    tmp_path.replace(path)


def _load_manifest_pdfs(path: Path) -> list[dict[str, Any]]:
    manifest = _load_json(path)
    pdfs = manifest.get("pdfs", [])
    if not isinstance(pdfs, list):
        raise ValueError(f"manifest pdfs must be a list: {path}")
    return pdfs


def _load_packets(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    packets: list[dict[str, Any]] = []
    for packet_path in sorted(path.glob("*.json")):
        packets.append(_load_json(packet_path))
    return packets


def _packet_status(packet: dict[str, Any]) -> str:
    status = packet.get("status") or packet.get("quality_status") or "unknown"
    return str(status)


def _success_count(packets: list[dict[str, Any]]) -> int:
    return sum(1 for packet in packets if _packet_status(packet) == "success")


def _quality_counts(packets: list[dict[str, Any]]) -> dict[str, int]:
    return dict(sorted(Counter(_packet_status(packet) for packet in packets).items()))


def _all_false(value: object) -> bool:
    return isinstance(value, dict) and bool(value) and all(item is False for item in value.values())


def _packet_safety_defaults_false(packets: list[dict[str, Any]]) -> bool:
    return all(_all_false(packet.get("safety_defaults")) for packet in packets)


def _tei_path(packet: dict[str, Any]) -> Path | None:
    raw_path = packet.get("tei_path")
    if not isinstance(raw_path, str) or not raw_path:
        return None
    path = Path(raw_path)
    if path.exists():
        return path
    return None


def _extract_arxiv_refs_from_tei(path: Path | None) -> set[str]:
    if path is None:
        return set()
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError:
        return set()
    text = " ".join(item for item in root.itertext() if item)
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


def _edge_records(packets: list[dict[str, Any]], target_arxiv_ids: set[str]) -> list[dict[str, str]]:
    edges: set[tuple[str, str]] = set()
    for packet in packets:
        source = packet.get("arxiv_id")
        if not isinstance(source, str):
            continue
        for target in _extract_arxiv_refs_from_tei(_tei_path(packet)):
            if target in target_arxiv_ids and target != source:
                edges.add((source, target))
    return [{"source_arxiv_id": source, "target_arxiv_id": target} for source, target in sorted(edges)]


def _edge_key(edge: dict[str, str]) -> tuple[str, str]:
    return (edge["source_arxiv_id"], edge["target_arxiv_id"])


def _sorted_edges(edges: list[dict[str, str]]) -> list[dict[str, str]]:
    return sorted(edges, key=_edge_key)


def _dedupe_pdfs(groups: list[list[dict[str, Any]]]) -> list[dict[str, Any]]:
    deduped: dict[str, dict[str, Any]] = {}
    for group in groups:
        for pdf in group:
            arxiv_id = pdf.get("arxiv_id") or pdf.get("requested_arxiv_id")
            if not isinstance(arxiv_id, str):
                continue
            deduped.setdefault(arxiv_id, pdf)
    return [deduped[key] for key in sorted(deduped)]


def _category_counts(pdfs: list[dict[str, Any]]) -> dict[str, int]:
    counts: Counter[str] = Counter()
    for pdf in pdfs:
        category = str(pdf.get("category") or "mixed-source")
        if category not in SUPPORTED_CATEGORIES:
            category = "mixed-source"
        counts[category] += 1
    return dict(sorted(counts.items()))


def _length_bucket(pdf: dict[str, Any]) -> str:
    pages = pdf.get("pages_estimate")
    if not isinstance(pages, int):
        return "unknown"
    if pages <= 8:
        return "short"
    if pages <= 16:
        return "medium"
    return "long"


def _length_counts(pdfs: list[dict[str, Any]]) -> dict[str, int]:
    return dict(sorted(Counter(_length_bucket(pdf) for pdf in pdfs).items()))


def _build_cumulative_corpus(
    *,
    existing_pdfs: list[dict[str, Any]],
    wave_1_pdfs: list[dict[str, Any]],
    wave_2_pdfs: list[dict[str, Any]],
    wave_3_pdfs: list[dict[str, Any]],
    wave_4_pdfs: list[dict[str, Any]],
    wave_5_pdfs: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    return _dedupe_pdfs([existing_pdfs, wave_1_pdfs, wave_2_pdfs, wave_3_pdfs, wave_4_pdfs, wave_5_pdfs])


def analyze_wave_5(
    *,
    wave_1_dir: Path,
    wave_2_dir: Path,
    wave_3_dir: Path,
    wave_4_dir: Path,
    wave_5_dir: Path,
    existing_corpus: Path,
    anchor_arxiv_id: str,
) -> dict[str, Any]:
    existing_pdfs = _load_manifest_pdfs(existing_corpus)
    wave_1_pdfs = _load_manifest_pdfs(wave_1_dir / "corpus-manifest.json")
    wave_2_pdfs = _load_manifest_pdfs(wave_2_dir / "corpus-manifest.json")
    wave_3_pdfs = _load_manifest_pdfs(wave_3_dir / "corpus-manifest.json")
    wave_4_pdfs = _load_manifest_pdfs(wave_4_dir / "corpus-manifest.json")
    wave_5_pdfs = _load_manifest_pdfs(wave_5_dir / "corpus-manifest.json")
    acquisition = _load_json(wave_5_dir / "acquisition-log.json")
    wave_4_analysis = _load_json(wave_4_dir / "analysis.json")

    wave_5_grobid_packets = _load_packets(wave_5_dir / "grobid-fulltext" / "per-pdf")
    wave_5_opendataloader_packets = _load_packets(wave_5_dir / "opendataloader" / "per-pdf")

    target_arxiv_ids = {
        str(pdf.get("arxiv_id") or pdf.get("requested_arxiv_id"))
        for pdf in existing_pdfs
        if pdf.get("arxiv_id") or pdf.get("requested_arxiv_id")
    }
    target_arxiv_ids.add(anchor_arxiv_id)

    previous_connectivity = wave_4_analysis["connectivity"]
    previous_cumulative_edges = _sorted_edges(list(previous_connectivity.get("cumulative_edges", [])))
    wave_5_edges = _edge_records(wave_5_grobid_packets, target_arxiv_ids)
    cumulative_edges = _sorted_edges(
        [
            {"source_arxiv_id": source, "target_arxiv_id": target}
            for source, target in {_edge_key(edge) for edge in previous_cumulative_edges + wave_5_edges}
        ]
    )
    previous_wave_count = int(previous_connectivity.get("wave_4_new_edge_count", 0))
    wave_5_count = len(wave_5_edges)

    cumulative_pdfs = _build_cumulative_corpus(
        existing_pdfs=existing_pdfs,
        wave_1_pdfs=wave_1_pdfs,
        wave_2_pdfs=wave_2_pdfs,
        wave_3_pdfs=wave_3_pdfs,
        wave_4_pdfs=wave_4_pdfs,
        wave_5_pdfs=wave_5_pdfs,
    )
    cumulative_corpus_path = wave_5_dir / "cumulative-corpus.json"
    _atomic_write_json(
        cumulative_corpus_path,
        {
            "schema_version": "m056-bfs-wave-5-cumulative-corpus.v1",
            "generated_at": _utc_now(),
            "source": "M056-lchpnp S05 Wave 5 cumulative evidence corpus",
            "expected_total_pdfs": 170,
            "pdf_count": len(cumulative_pdfs),
            "safety_defaults": _safety_defaults(),
            "pdfs": cumulative_pdfs,
        },
    )

    anchor_first_author = str(
        wave_4_analysis.get("self_citation_cluster", {}).get("anchor_first_author") or "Xuying Ning"
    )
    matching_self_citations = []
    for packet in wave_5_grobid_packets:
        arxiv_id = packet.get("arxiv_id")
        authors = _extract_authors_from_tei(_tei_path(packet))
        if isinstance(arxiv_id, str) and any(anchor_first_author.lower() in author.lower() for author in authors):
            matching_self_citations.append(arxiv_id)

    parser_quality = {
        "grobid_packet_count": len(wave_5_grobid_packets),
        "grobid_success_count": _success_count(wave_5_grobid_packets),
        "grobid_quality_counts": _quality_counts(wave_5_grobid_packets),
        "opendataloader_packet_count": len(wave_5_opendataloader_packets),
        "opendataloader_success_count": _success_count(wave_5_opendataloader_packets),
        "opendataloader_quality_counts": _quality_counts(wave_5_opendataloader_packets),
        "packet_safety_defaults_all_false": _packet_safety_defaults_false(
            wave_5_grobid_packets + wave_5_opendataloader_packets
        ),
    }

    analysis = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "inputs": {
            "existing_corpus": str(existing_corpus),
            "wave_1_dir": str(wave_1_dir),
            "wave_2_dir": str(wave_2_dir),
            "wave_3_dir": str(wave_3_dir),
            "wave_4_dir": str(wave_4_dir),
            "wave_5_dir": str(wave_5_dir),
        },
        "safety_defaults": _safety_defaults(),
        "acquisition": {
            "success_count": acquisition.get("success_count", 0),
            "blocked_count": acquisition.get("blocked_count", 0),
            "network_error_count": acquisition.get("network_error_count", 0),
            "status_counts": acquisition.get("status_counts", {}),
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
            "wave_5_new_edge_count": wave_5_count,
            "wave_5_new_edges": wave_5_edges,
            "connectivity_gain_delta_vs_wave_4": wave_5_count - previous_wave_count,
            "cumulative_edge_count": len(cumulative_edges),
            "cumulative_edges": cumulative_edges,
            "edge_saturation_by_wave": {
                "wave_1": previous_connectivity.get("wave_1_edge_count", 0),
                "wave_2": previous_connectivity.get("wave_2_new_edge_count", 0),
                "wave_3": previous_connectivity.get("wave_3_new_edge_count", 0),
                "wave_4": previous_connectivity.get("wave_4_new_edge_count", 0),
                "wave_5": wave_5_count,
            },
            "saturation_status": "saturated" if wave_5_count <= previous_wave_count else "expanded",
        },
        "self_citation_cluster": {
            "anchor_first_author": anchor_first_author,
            "matching_wave_5_pdfs": matching_self_citations,
            "matching_wave_5_count": len(matching_self_citations),
            "wave_5_pdf_count": len(wave_5_pdfs),
        },
        "category_distribution": _category_counts(wave_5_pdfs),
        "length_distribution": _length_counts(wave_5_pdfs),
        "cumulative_category_distribution": _category_counts(cumulative_pdfs),
        "cumulative_length_distribution": _length_counts(cumulative_pdfs),
        "cumulative_corpus": {
            "expected_total": 170,
            "actual_total": len(cumulative_pdfs),
            "path": str(cumulative_corpus_path),
        },
    }
    _atomic_write_json(wave_5_dir / "analysis.json", analysis)
    _atomic_write_text(wave_5_dir / "analysis.md", _render_markdown(analysis))
    return analysis


def _render_counts(counts: dict[str, int]) -> str:
    return ", ".join(f"{key}: {value}" for key, value in sorted(counts.items())) or "none"


def _render_edges(edges: list[dict[str, str]]) -> str:
    if not edges:
        return "- None"
    return "\n".join(f"- {edge['source_arxiv_id']} -> {edge['target_arxiv_id']}" for edge in edges)


def _render_markdown(analysis: dict[str, Any]) -> str:
    acquisition = analysis["acquisition"]
    parser_quality = analysis["parser_quality"]
    connectivity = analysis["connectivity"]
    self_cluster = analysis["self_citation_cluster"]
    category_distribution = analysis["category_distribution"]
    length_distribution = analysis["length_distribution"]
    cumulative_corpus = analysis["cumulative_corpus"]
    matching_count = self_cluster["matching_wave_5_count"]
    wave_pdf_count = max(1, self_cluster["wave_5_pdf_count"])
    matching_percent = matching_count / wave_pdf_count * 100
    return f"""# M056 Wave 5 Analysis

Generated: `{analysis['generated_at']}`

## Safety

- Graph writes: false
- Production import attempted: false
- Promotion allowed: false
- Facts promoted: false
- External mutation allowed: false
- This evidence is not authorized for graph import or fact promotion.

## Acquisition

- Success: {acquisition['success_count']}
- Blocked: {acquisition['blocked_count']}
- Network errors: {acquisition['network_error_count']}
- Status counts: {_render_counts(acquisition['status_counts'])}

## Parser quality

- GROBID packets: {parser_quality['grobid_packet_count']}
- GROBID success: {parser_quality['grobid_success_count']}
- GROBID quality counts: {_render_counts(parser_quality['grobid_quality_counts'])}
- OpenDataLoader packets: {parser_quality['opendataloader_packet_count']}
- OpenDataLoader success: {parser_quality['opendataloader_success_count']}
- OpenDataLoader quality counts: {_render_counts(parser_quality['opendataloader_quality_counts'])}
- Packet safety defaults all false: {parser_quality['packet_safety_defaults_all_false']}

## Connectivity gain

- Target set: 20 existing corpus PDFs + anchor `{connectivity['anchor_arxiv_id']}`
- Wave 1 directed edges to target set: {connectivity['wave_1_edge_count']}
- Wave 2 new directed edges to target set: {connectivity['wave_2_new_edge_count']}
- Wave 3 new directed edges to target set: {connectivity['wave_3_new_edge_count']}
- Wave 4 new directed edges to target set: {connectivity['wave_4_new_edge_count']}
- Wave 5 new directed edges to target set: {connectivity['wave_5_new_edge_count']}
- Delta vs Wave 4: {connectivity['connectivity_gain_delta_vs_wave_4']}
- Cumulative directed edges: {connectivity['cumulative_edge_count']}
- Saturation status: {connectivity['saturation_status']}

### Edge saturation by wave

- Wave 1: {connectivity['edge_saturation_by_wave']['wave_1']}
- Wave 2: {connectivity['edge_saturation_by_wave']['wave_2']}
- Wave 3: {connectivity['edge_saturation_by_wave']['wave_3']}
- Wave 4: {connectivity['edge_saturation_by_wave']['wave_4']}
- Wave 5: {connectivity['edge_saturation_by_wave']['wave_5']}

### New edges added this wave

{_render_edges(connectivity['wave_5_new_edges'])}

### Cumulative edges

{_render_edges(connectivity['cumulative_edges'])}

## Self-citation cluster

- Anchor first author: {self_cluster['anchor_first_author']}
- Matching Wave 5 PDFs: {matching_count} / {self_cluster['wave_5_pdf_count']} ({matching_percent:.1f}%)

## Category distribution

- {_render_counts(category_distribution)}

## Length distribution

- {_render_counts(length_distribution)}

## Cumulative corpus

- Expected total PDFs: {cumulative_corpus['expected_total']}
- Actual total PDFs: {cumulative_corpus['actual_total']}
- Path: `{cumulative_corpus['path']}`
"""


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wave-1-dir", type=Path, default=DEFAULT_WAVE_1_DIR)
    parser.add_argument("--wave-2-dir", type=Path, default=DEFAULT_WAVE_2_DIR)
    parser.add_argument("--wave-3-dir", type=Path, default=DEFAULT_WAVE_3_DIR)
    parser.add_argument("--wave-4-dir", type=Path, default=DEFAULT_WAVE_4_DIR)
    parser.add_argument("--wave-5-dir", type=Path, default=DEFAULT_WAVE_5_DIR)
    parser.add_argument("--existing-corpus", type=Path, default=DEFAULT_EXISTING_CORPUS)
    parser.add_argument("--anchor-arxiv-id", default=DEFAULT_ANCHOR_ARXIV_ID)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    analysis = analyze_wave_5(
        wave_1_dir=args.wave_1_dir,
        wave_2_dir=args.wave_2_dir,
        wave_3_dir=args.wave_3_dir,
        wave_4_dir=args.wave_4_dir,
        wave_5_dir=args.wave_5_dir,
        existing_corpus=args.existing_corpus,
        anchor_arxiv_id=args.anchor_arxiv_id,
    )
    print(json.dumps(analysis, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
