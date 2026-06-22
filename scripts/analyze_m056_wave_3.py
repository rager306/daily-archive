#!/usr/bin/env python3
"""Analyze M056 Wave 3 parser outputs and cumulative BFS connectivity.

The analysis is evidence-only: it reads acquisition, GROBID, OpenDataLoader,
Wave 1, Wave 2, and prior corpus artifacts, then writes markdown/JSON summaries.
It does not write or import graph data; all safety defaults remain false.
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

SCHEMA_VERSION = "m056-bfs-wave-3-analysis.v1"
DEFAULT_WAVE_1_DIR = Path("artifacts/m056-bfs-graph/wave-1")
DEFAULT_WAVE_2_DIR = Path("artifacts/m056-bfs-graph/wave-2")
DEFAULT_WAVE_3_DIR = Path("artifacts/m056-bfs-graph/wave-3")
DEFAULT_EXISTING_CORPUS = Path("artifacts/m055deep-parser-benchmark/corpus-manifest-20.json")
DEFAULT_ANCHOR_ARXIV_ID = "2605.18747"
ARXIV_ID_RE = re.compile(r"(?<!\d)(\d{4}\.\d{4,5})(?:v\d+)?(?!\d)")
SUPPORTED_CATEGORIES = ("cs-ai", "cs-cl", "cs-cv", "cs-lg", "mixed-source")


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


def _atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + ".tmp")
    tmp_path.write_text(text, encoding="utf-8")
    tmp_path.replace(path)


def _load_manifest_pdfs(path: Path) -> list[dict[str, Any]]:
    payload = _load_json(path)
    pdfs = payload.get("pdfs", [])
    if not isinstance(pdfs, list):
        raise ValueError(f"manifest pdfs must be a list: {path}")
    return [entry for entry in pdfs if isinstance(entry, dict)]


def _load_packets(packet_dir: Path) -> list[dict[str, Any]]:
    if not packet_dir.exists():
        return []
    packets: list[dict[str, Any]] = []
    for packet_path in sorted(packet_dir.glob("*.json")):
        payload = _load_json(packet_path)
        if isinstance(payload, dict):
            packets.append(payload)
    return packets


def _packet_status(packet: dict[str, Any]) -> str:
    status = str(packet.get("status") or "unknown")
    if status == "success" and packet.get("low_quality_source") is True:
        return "low_quality_source"
    return status


def _success_count(packets: list[dict[str, Any]]) -> int:
    return sum(1 for packet in packets if packet.get("status") == "success")


def _quality_counts(packets: list[dict[str, Any]]) -> Counter[str]:
    return Counter(_packet_status(packet) for packet in packets)


def _all_false(value: object) -> bool:
    return isinstance(value, dict) and bool(value) and all(item is False for item in value.values())


def _packet_safety_defaults_false(packet: dict[str, Any]) -> bool:
    return _all_false(packet.get("safety_defaults"))


def _tei_path(grobid_dir: Path, arxiv_id: str) -> Path:
    return grobid_dir / "tei" / f"{arxiv_id}.tei.xml"


def _extract_arxiv_refs_from_tei(path: Path) -> set[str]:
    if not path.exists():
        return set()
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return set()
    return set(ARXIV_ID_RE.findall(text))


def _extract_first_author_from_tei(path: Path) -> str | None:
    if not path.exists():
        return None
    try:
        root = ET.parse(path).getroot()
    except (ET.ParseError, OSError):
        return None
    ns = {"tei": "http://www.tei-c.org/ns/1.0"}
    for author in root.findall(".//tei:teiHeader//tei:author", ns):
        names = []
        for tag in ("forename", "surname"):
            node = author.find(f".//tei:{tag}", ns)
            if node is not None and node.text:
                names.append(node.text.strip())
        if names:
            return " ".join(names)
    return None


def _extract_authors_from_tei(path: Path) -> list[str]:
    if not path.exists():
        return []
    try:
        root = ET.parse(path).getroot()
    except (ET.ParseError, OSError):
        return []
    ns = {"tei": "http://www.tei-c.org/ns/1.0"}
    authors: list[str] = []
    for author in root.findall(".//tei:teiHeader//tei:author", ns):
        names = []
        for tag in ("forename", "surname"):
            node = author.find(f".//tei:{tag}", ns)
            if node is not None and node.text:
                names.append(node.text.strip())
        if names:
            authors.append(" ".join(names))
    return authors


def _edge_records(
    *, wave_pdfs: list[dict[str, Any]], grobid_dir: Path, target_ids: set[str]
) -> list[dict[str, str]]:
    edges: list[dict[str, str]] = []
    seen: set[tuple[str, str]] = set()
    for entry in wave_pdfs:
        source_id = str(entry.get("arxiv_id") or "")
        if not source_id:
            continue
        refs = _extract_arxiv_refs_from_tei(_tei_path(grobid_dir, source_id))
        for target_id in sorted(refs & target_ids):
            pair = (source_id, target_id)
            if source_id != target_id and pair not in seen:
                seen.add(pair)
                edges.append({"source_arxiv_id": source_id, "target_arxiv_id": target_id})
    return edges


def _dedupe_pdfs(groups: list[list[dict[str, Any]]]) -> list[dict[str, Any]]:
    pdfs: list[dict[str, Any]] = []
    seen: set[str] = set()
    for group in groups:
        for entry in group:
            arxiv_id = str(entry.get("arxiv_id") or "")
            if not arxiv_id or arxiv_id in seen:
                continue
            seen.add(arxiv_id)
            pdfs.append(entry)
    return pdfs


def _category_counts(pdfs: list[dict[str, Any]]) -> dict[str, int]:
    counts = Counter(str(entry.get("category") or "mixed-source") for entry in pdfs)
    return {category: counts.get(category, 0) for category in SUPPORTED_CATEGORIES}


def _length_bucket(entry: dict[str, Any]) -> str:
    pages = int(entry.get("pages_estimate") or entry.get("page_count") or 0)
    if pages <= 10:
        return "short_1_10"
    if pages <= 25:
        return "medium_11_25"
    return "long_26_plus"


def _length_counts(pdfs: list[dict[str, Any]]) -> dict[str, int]:
    counts = Counter(_length_bucket(entry) for entry in pdfs)
    return {
        bucket: counts.get(bucket, 0) for bucket in ("short_1_10", "medium_11_25", "long_26_plus")
    }


def _edge_key(edge: dict[str, str]) -> tuple[str, str]:
    return (str(edge["source_arxiv_id"]), str(edge["target_arxiv_id"]))


def _sorted_edges(edges: list[dict[str, str]]) -> list[dict[str, str]]:
    return sorted(edges, key=lambda edge: (edge["source_arxiv_id"], edge["target_arxiv_id"]))


def _build_cumulative_corpus(
    *,
    existing_pdfs: list[dict[str, Any]],
    wave_1_pdfs: list[dict[str, Any]],
    wave_2_pdfs: list[dict[str, Any]],
    wave_3_pdfs: list[dict[str, Any]],
    output_path: Path,
) -> dict[str, Any]:
    cumulative_pdfs = _dedupe_pdfs([existing_pdfs, wave_1_pdfs, wave_2_pdfs, wave_3_pdfs])
    payload = {
        "schema_version": "m056-bfs-wave-3-cumulative-corpus.v1",
        "generated_at": _utc_now(),
        "source": "M056-lchpnp S03 Wave 3 cumulative corpus",
        "expected_total_pdfs": 110,
        "pdf_count": len(cumulative_pdfs),
        "pdfs": cumulative_pdfs,
        "safety_defaults": _safety_defaults(),
    }
    _atomic_write_json(output_path, payload)
    return payload


def analyze_wave_3(
    *,
    wave_1_dir: Path,
    wave_2_dir: Path,
    wave_3_dir: Path,
    existing_corpus: Path,
    anchor_arxiv_id: str,
) -> dict[str, Any]:
    wave_1_manifest = wave_1_dir / "corpus-manifest.json"
    wave_2_manifest = wave_2_dir / "corpus-manifest.json"
    wave_3_manifest = wave_3_dir / "corpus-manifest.json"
    wave_3_acquisition_log = wave_3_dir / "acquisition-log.json"

    existing_pdfs = _load_manifest_pdfs(existing_corpus)
    wave_1_pdfs = _load_manifest_pdfs(wave_1_manifest)
    wave_2_pdfs = _load_manifest_pdfs(wave_2_manifest)
    wave_3_pdfs = _load_manifest_pdfs(wave_3_manifest)
    acquisition = _load_json(wave_3_acquisition_log)

    wave_1_grobid_packets = _load_packets(wave_1_dir / "grobid-fulltext" / "per-pdf")
    wave_2_grobid_packets = _load_packets(wave_2_dir / "grobid-fulltext" / "per-pdf")
    wave_3_grobid_packets = _load_packets(wave_3_dir / "grobid-fulltext" / "per-pdf")
    wave_1_opendataloader_packets = _load_packets(wave_1_dir / "opendataloader" / "per-pdf")
    wave_2_opendataloader_packets = _load_packets(wave_2_dir / "opendataloader" / "per-pdf")
    wave_3_opendataloader_packets = _load_packets(wave_3_dir / "opendataloader" / "per-pdf")
    all_packets = (
        wave_1_grobid_packets
        + wave_2_grobid_packets
        + wave_3_grobid_packets
        + wave_1_opendataloader_packets
        + wave_2_opendataloader_packets
        + wave_3_opendataloader_packets
    )

    target_ids = {str(entry.get("arxiv_id")) for entry in existing_pdfs if entry.get("arxiv_id")}
    target_ids.add(anchor_arxiv_id)
    wave_2_analysis = _load_json(wave_2_dir / "analysis.json")
    wave_1_edges = wave_2_analysis["connectivity"].get("wave_1_edges", [])
    wave_2_edges = wave_2_analysis["connectivity"].get("wave_2_new_edges", [])
    wave_3_edges = _edge_records(
        wave_pdfs=wave_3_pdfs, grobid_dir=wave_3_dir / "grobid-fulltext", target_ids=target_ids
    )
    cumulative_edge_map = {
        _edge_key(edge): edge for edge in wave_1_edges + wave_2_edges + wave_3_edges
    }
    cumulative_edges = _sorted_edges(list(cumulative_edge_map.values()))

    anchor_tei = wave_1_dir / "anchor-grobid" / "tei" / f"{anchor_arxiv_id}.tei.xml"
    anchor_first_author = _extract_first_author_from_tei(anchor_tei) or "unknown"
    anchor_last_name = (
        anchor_first_author.split()[-1].lower() if anchor_first_author != "unknown" else ""
    )
    self_citation_matches = []
    for entry in wave_3_pdfs:
        arxiv_id = str(entry.get("arxiv_id") or "")
        authors = _extract_authors_from_tei(_tei_path(wave_3_dir / "grobid-fulltext", arxiv_id))
        if anchor_last_name and any(anchor_last_name in author.lower() for author in authors):
            self_citation_matches.append({"arxiv_id": arxiv_id, "authors": authors})

    cumulative_corpus_path = wave_3_dir / "cumulative-corpus.json"
    cumulative_corpus = _build_cumulative_corpus(
        existing_pdfs=existing_pdfs,
        wave_1_pdfs=wave_1_pdfs,
        wave_2_pdfs=wave_2_pdfs,
        wave_3_pdfs=wave_3_pdfs,
        output_path=cumulative_corpus_path,
    )

    wave_1_edge_count = int(wave_2_analysis["connectivity"].get("wave_1_edge_count", 0))
    wave_2_edge_count = int(wave_2_analysis["connectivity"].get("wave_2_new_edge_count", 0))
    wave_3_edge_count = len(wave_3_edges)
    saturation_status = "saturated" if wave_3_edge_count <= wave_2_edge_count else "not_saturated"

    analysis = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "inputs": {
            "wave_1_dir": str(wave_1_dir),
            "wave_2_dir": str(wave_2_dir),
            "wave_3_acquisition_log": str(wave_3_acquisition_log),
            "wave_3_manifest": str(wave_3_manifest),
            "existing_corpus": str(existing_corpus),
            "anchor_tei": str(anchor_tei),
        },
        "safety_defaults": _safety_defaults(),
        "acquisition": {
            "success_count": int(acquisition.get("success_count") or 0),
            "blocked_count": int(acquisition.get("blocked_count") or 0),
            "network_error_count": int(acquisition.get("network_error_count") or 0),
            "status_counts": acquisition.get("status_counts", {}),
        },
        "parser_quality": {
            "wave_1_grobid_packet_count": len(wave_1_grobid_packets),
            "wave_2_grobid_packet_count": len(wave_2_grobid_packets),
            "wave_3_grobid_packet_count": len(wave_3_grobid_packets),
            "grobid_packet_count": len(wave_3_grobid_packets),
            "grobid_success_count": _success_count(wave_3_grobid_packets),
            "grobid_quality_counts": dict(sorted(_quality_counts(wave_3_grobid_packets).items())),
            "wave_1_opendataloader_packet_count": len(wave_1_opendataloader_packets),
            "wave_2_opendataloader_packet_count": len(wave_2_opendataloader_packets),
            "wave_3_opendataloader_packet_count": len(wave_3_opendataloader_packets),
            "opendataloader_packet_count": len(wave_3_opendataloader_packets),
            "opendataloader_success_count": _success_count(wave_3_opendataloader_packets),
            "opendataloader_quality_counts": dict(
                sorted(_quality_counts(wave_3_opendataloader_packets).items())
            ),
            "all_packet_safety_defaults_false": all(
                _packet_safety_defaults_false(packet) for packet in all_packets
            ),
        },
        "connectivity": {
            "anchor_arxiv_id": anchor_arxiv_id,
            "existing_corpus_target_count": len(target_ids - {anchor_arxiv_id}),
            "target_count": len(target_ids),
            "wave_1_edge_count": wave_1_edge_count,
            "wave_1_edges": _sorted_edges(wave_1_edges),
            "wave_2_new_edge_count": wave_2_edge_count,
            "wave_2_new_edges": _sorted_edges(wave_2_edges),
            "wave_3_new_edge_count": wave_3_edge_count,
            "wave_3_new_edges": _sorted_edges(wave_3_edges),
            "edge_saturation_by_wave": {
                "wave_1": wave_1_edge_count,
                "wave_2": wave_2_edge_count,
                "wave_3": wave_3_edge_count,
            },
            "connectivity_gain_delta_vs_wave_2": wave_3_edge_count - wave_2_edge_count,
            "cumulative_edge_count": len(cumulative_edges),
            "cumulative_edges": cumulative_edges,
            "saturation_status": saturation_status,
        },
        "self_citation_cluster": {
            "anchor_first_author": anchor_first_author,
            "matching_wave_3_pdf_count": len(self_citation_matches),
            "wave_3_pdf_count": len(wave_3_pdfs),
            "matches": self_citation_matches,
        },
        "category_distribution": _category_counts(wave_3_pdfs),
        "length_distribution": _length_counts(wave_3_pdfs),
        "cumulative_category_distribution": _category_counts(cumulative_corpus["pdfs"]),
        "cumulative_length_distribution": _length_counts(cumulative_corpus["pdfs"]),
        "cumulative_corpus": {
            "expected_total": 110,
            "actual_total": int(cumulative_corpus["pdf_count"]),
            "path": str(cumulative_corpus_path),
        },
    }
    _atomic_write_json(wave_3_dir / "analysis.json", analysis)
    _atomic_write_text(wave_3_dir / "analysis.md", _render_markdown(analysis))
    return analysis


def _render_counts(counts: dict[str, int]) -> str:
    return ", ".join(f"{key}: {value}" for key, value in counts.items())


def _render_edges(edges: list[dict[str, str]]) -> str:
    if not edges:
        return "- None"
    return "\n".join(f"- {edge['source_arxiv_id']} -> {edge['target_arxiv_id']}" for edge in edges)


def _render_markdown(analysis: dict[str, Any]) -> str:
    connectivity = analysis["connectivity"]
    parser_quality = analysis["parser_quality"]
    self_cluster = analysis["self_citation_cluster"]
    acquisition = analysis["acquisition"]
    lines = [
        "# M056 Wave 3 Analysis",
        "",
        f"Generated: `{analysis['generated_at']}`",
        "",
        "## Safety",
        "",
        "- Graph writes: false",
        "- Production import attempted: false",
        "- Promotion allowed: false",
        "- Facts promoted: false",
        "- External mutation allowed: false",
        "- This evidence is not authorized for graph import or fact promotion.",
        "",
        "## Acquisition",
        "",
        f"- Success: {acquisition['success_count']}",
        f"- Blocked: {acquisition['blocked_count']}",
        f"- Network errors: {acquisition['network_error_count']}",
        f"- Status counts: {_render_counts(acquisition['status_counts'])}",
        "",
        "## Parser quality",
        "",
        f"- GROBID packets: {parser_quality['grobid_packet_count']}",
        f"- GROBID success: {parser_quality['grobid_success_count']}",
        f"- GROBID quality counts: {_render_counts(parser_quality['grobid_quality_counts'])}",
        f"- OpenDataLoader packets: {parser_quality['opendataloader_packet_count']}",
        f"- OpenDataLoader success: {parser_quality['opendataloader_success_count']}",
        f"- OpenDataLoader quality counts: {_render_counts(parser_quality['opendataloader_quality_counts'])}",
        f"- Packet safety defaults all false: {parser_quality['all_packet_safety_defaults_false']}",
        "",
        "## Connectivity gain",
        "",
        f"- Target set: {connectivity['existing_corpus_target_count']} existing corpus PDFs + anchor `{connectivity['anchor_arxiv_id']}`",
        f"- Wave 1 directed edges to target set: {connectivity['wave_1_edge_count']}",
        f"- Wave 2 new directed edges to target set: {connectivity['wave_2_new_edge_count']}",
        f"- Wave 3 new directed edges to target set: {connectivity['wave_3_new_edge_count']}",
        f"- Delta vs Wave 2: {connectivity['connectivity_gain_delta_vs_wave_2']}",
        f"- Cumulative directed edges: {connectivity['cumulative_edge_count']}",
        f"- Saturation status: {connectivity['saturation_status']}",
        "",
        "### Edge saturation by wave",
        "",
        f"- Wave 1: {connectivity['edge_saturation_by_wave']['wave_1']}",
        f"- Wave 2: {connectivity['edge_saturation_by_wave']['wave_2']}",
        f"- Wave 3: {connectivity['edge_saturation_by_wave']['wave_3']}",
        "",
        "### New edges added this wave",
        "",
        _render_edges(connectivity["wave_3_new_edges"]),
        "",
        "## Self-citation cluster",
        "",
        f"- Anchor first author: {self_cluster['anchor_first_author']}",
        f"- Matching Wave 3 PDFs: {self_cluster['matching_wave_3_pdf_count']} / {self_cluster['wave_3_pdf_count']}",
        "",
        "## Category distribution",
        "",
        f"- {_render_counts(analysis['category_distribution'])}",
        "",
        "## Length distribution",
        "",
        f"- {_render_counts(analysis['length_distribution'])}",
        "",
        "## Cumulative corpus",
        "",
        f"- Expected total PDFs: {analysis['cumulative_corpus']['expected_total']}",
        f"- Actual unique PDFs: {analysis['cumulative_corpus']['actual_total']}",
        f"- Path: `{analysis['cumulative_corpus']['path']}`",
        "",
    ]
    return "\n".join(lines)


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wave-1-dir", type=Path, default=DEFAULT_WAVE_1_DIR)
    parser.add_argument("--wave-2-dir", type=Path, default=DEFAULT_WAVE_2_DIR)
    parser.add_argument("--wave-3-dir", type=Path, default=DEFAULT_WAVE_3_DIR)
    parser.add_argument("--existing-corpus", type=Path, default=DEFAULT_EXISTING_CORPUS)
    parser.add_argument("--anchor-arxiv-id", default=DEFAULT_ANCHOR_ARXIV_ID)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    analysis = analyze_wave_3(
        wave_1_dir=args.wave_1_dir,
        wave_2_dir=args.wave_2_dir,
        wave_3_dir=args.wave_3_dir,
        existing_corpus=args.existing_corpus,
        anchor_arxiv_id=args.anchor_arxiv_id,
    )
    print(json.dumps(analysis, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
