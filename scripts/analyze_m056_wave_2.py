#!/usr/bin/env python3
"""Analyze M056 Wave 2 parser outputs and cumulative BFS connectivity.

The analysis is evidence-only: it reads acquisition, GROBID, OpenDataLoader,
Wave 1, and prior corpus artifacts, then writes markdown/JSON summaries. It does
not write or import graph data; all safety defaults remain false.
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

SCHEMA_VERSION = "m056-bfs-wave-2-analysis.v1"
DEFAULT_WAVE_1_DIR = Path("artifacts/m056-bfs-graph/wave-1")
DEFAULT_WAVE_2_DIR = Path("artifacts/m056-bfs-graph/wave-2")
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


def _packet_paths(directory: Path) -> list[Path]:
    return sorted((directory / "per-pdf").glob("*.json"))


def _load_packets(directory: Path) -> dict[str, dict[str, Any]]:
    packets: dict[str, dict[str, Any]] = {}
    for path in _packet_paths(directory):
        packet = _load_json(path)
        arxiv_id = str(packet.get("arxiv_id") or path.stem)
        packets[arxiv_id] = packet
    return packets


def _tei_path(grobid_dir: Path, arxiv_id: str) -> Path:
    return grobid_dir / "tei" / f"{arxiv_id}.tei.xml"


def _extract_arxiv_ids_from_text(text: str) -> set[str]:
    return {match.group(1) for match in ARXIV_ID_RE.finditer(text)}


def _extract_arxiv_refs_from_tei(path: Path) -> set[str]:
    if not path.exists():
        return set()
    text = path.read_text(encoding="utf-8", errors="replace")
    refs = _extract_arxiv_ids_from_text(text)
    try:
        root = ET.fromstring(text)
    except ET.ParseError:
        return refs
    for elem in root.iter():
        local_name = elem.tag.rsplit("}", 1)[-1]
        if local_name in {"idno", "ref", "title", "note"} and elem.text:
            refs.update(_extract_arxiv_ids_from_text(elem.text))
        for value in elem.attrib.values():
            refs.update(_extract_arxiv_ids_from_text(value))
    return refs


def _first_author_from_tei(path: Path) -> dict[str, str | None]:
    if not path.exists():
        return {"forename": None, "surname": None, "display": None}
    try:
        root = ET.parse(path).getroot()
    except ET.ParseError:
        return {"forename": None, "surname": None, "display": None}
    ns = {"tei": "http://www.tei-c.org/ns/1.0"}
    author = root.find(".//tei:fileDesc//tei:titleStmt//tei:author", ns)
    if author is None:
        author = root.find(".//tei:sourceDesc//tei:author", ns)
    if author is None:
        return {"forename": None, "surname": None, "display": None}
    forename = (
        " ".join(
            (node.text or "").strip()
            for node in author.findall(".//tei:forename", ns)
            if (node.text or "").strip()
        )
        or None
    )
    surname = (
        " ".join(
            (node.text or "").strip()
            for node in author.findall(".//tei:surname", ns)
            if (node.text or "").strip()
        )
        or None
    )
    display = " ".join(part for part in (forename, surname) if part) or None
    return {"forename": forename, "surname": surname, "display": display}


def _status_for_packet(packet: dict[str, Any]) -> str:
    status = packet.get("status")
    if isinstance(status, str) and status:
        return status
    if packet.get("error"):
        return "blocked"
    if packet.get("low_quality_source") is True:
        return "low_quality_source"
    return "success"


def _packet_safety_defaults_false(packet: dict[str, Any]) -> bool:
    safety = packet.get("safety_defaults")
    return (
        isinstance(safety, dict)
        and bool(safety)
        and all(value is False for value in safety.values())
    )


def _length_bucket(pages: int) -> str:
    if pages <= 0:
        return "unknown"
    if pages <= 10:
        return "short_1_10"
    if pages <= 25:
        return "medium_11_25"
    return "long_26_plus"


def _edge_records(
    *, wave_pdfs: list[dict[str, Any]], grobid_dir: Path, target_ids: set[str]
) -> list[dict[str, str]]:
    edges: list[dict[str, str]] = []
    seen: set[tuple[str, str]] = set()
    for entry in wave_pdfs:
        source_id = str(entry["arxiv_id"])
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


def _build_cumulative_corpus(
    *,
    existing_corpus: dict[str, Any],
    wave_1_manifest: dict[str, Any],
    wave_2_manifest: dict[str, Any],
    wave_1_dir: Path,
    wave_2_dir: Path,
) -> dict[str, Any]:
    existing_pdfs = list(existing_corpus.get("pdfs", []))
    wave_1_pdfs = list(wave_1_manifest.get("pdfs", []))
    wave_2_pdfs = list(wave_2_manifest.get("pdfs", []))
    pdfs = existing_pdfs + wave_1_pdfs + wave_2_pdfs
    unique_arxiv_ids = {str(entry.get("arxiv_id")) for entry in pdfs if entry.get("arxiv_id")}
    return {
        "generated_at": _utc_now(),
        "inputs": {
            "existing_corpus": str(DEFAULT_EXISTING_CORPUS.as_posix()),
            "wave_1_manifest": str((wave_1_dir / "corpus-manifest.json").as_posix()),
            "wave_2_manifest": str((wave_2_dir / "corpus-manifest.json").as_posix()),
        },
        "expected_total": 80,
        "actual_total": len(pdfs),
        "unique_arxiv_id_count": len(unique_arxiv_ids),
        "pdfs": pdfs,
        "safety_defaults": _safety_defaults(),
    }


def _quality_counts(packets: dict[str, dict[str, Any]]) -> Counter[str]:
    return Counter(_status_for_packet(packet) for packet in packets.values())


def _success_count(packets: dict[str, dict[str, Any]]) -> int:
    return sum(1 for packet in packets.values() if _status_for_packet(packet) == "success")


def analyze_wave_2(
    *,
    wave_1_dir: Path,
    wave_2_dir: Path,
    existing_corpus_path: Path,
    anchor_arxiv_id: str,
) -> dict[str, Any]:
    wave_1_manifest = _load_json(wave_1_dir / "corpus-manifest.json")
    wave_2_acquisition = _load_json(wave_2_dir / "acquisition-log.json")
    wave_2_manifest = _load_json(wave_2_dir / "corpus-manifest.json")
    existing_corpus = _load_json(existing_corpus_path)

    wave_1_grobid_dir = wave_1_dir / "grobid-fulltext"
    wave_2_grobid_dir = wave_2_dir / "grobid-fulltext"
    wave_1_opendataloader_dir = wave_1_dir / "opendataloader"
    wave_2_opendataloader_dir = wave_2_dir / "opendataloader"

    wave_1_grobid_packets = _load_packets(wave_1_grobid_dir)
    wave_2_grobid_packets = _load_packets(wave_2_grobid_dir)
    wave_1_opendataloader_packets = _load_packets(wave_1_opendataloader_dir)
    wave_2_opendataloader_packets = _load_packets(wave_2_opendataloader_dir)

    existing_ids = {
        str(entry.get("arxiv_id"))
        for entry in existing_corpus.get("pdfs", [])
        if entry.get("arxiv_id")
    }
    target_ids = set(existing_ids)
    target_ids.add(anchor_arxiv_id)

    wave_1_pdfs = list(wave_1_manifest.get("pdfs", []))
    wave_2_pdfs = list(wave_2_manifest.get("pdfs", []))
    wave_1_edges = _edge_records(
        wave_pdfs=wave_1_pdfs, grobid_dir=wave_1_grobid_dir, target_ids=target_ids
    )
    wave_2_edges = _edge_records(
        wave_pdfs=wave_2_pdfs, grobid_dir=wave_2_grobid_dir, target_ids=target_ids
    )
    cumulative_edges = sorted(
        {tuple(edge.items()) for edge in [*wave_1_edges, *wave_2_edges]},
        key=lambda items: (dict(items)["source_arxiv_id"], dict(items)["target_arxiv_id"]),
    )
    cumulative_edge_records = [dict(items) for items in cumulative_edges]

    wave_2_category_distribution = Counter(
        str(entry.get("category", "mixed-source")) for entry in wave_2_pdfs
    )
    cumulative_category_distribution = Counter(
        str(entry.get("category", "mixed-source")) for entry in [*wave_1_pdfs, *wave_2_pdfs]
    )
    for category in SUPPORTED_CATEGORIES:
        wave_2_category_distribution.setdefault(category, 0)
        cumulative_category_distribution.setdefault(category, 0)
    wave_2_length_distribution = Counter(
        _length_bucket(int(entry.get("pages_estimate") or 0)) for entry in wave_2_pdfs
    )
    cumulative_length_distribution = Counter(
        _length_bucket(int(entry.get("pages_estimate") or 0))
        for entry in [*wave_1_pdfs, *wave_2_pdfs]
    )

    anchor_author = _first_author_from_tei(
        wave_1_dir / "anchor-grobid" / "tei" / f"{anchor_arxiv_id}.tei.xml"
    )
    anchor_surname = (anchor_author.get("surname") or "").casefold()
    first_authors: dict[str, dict[str, str | None]] = {}
    anchor_citing_sources = {
        edge["source_arxiv_id"]
        for edge in cumulative_edge_records
        if edge["target_arxiv_id"] == anchor_arxiv_id
    }
    self_cluster_matches: list[str] = []
    for entry, grobid_dir in [(entry, wave_1_grobid_dir) for entry in wave_1_pdfs] + [
        (entry, wave_2_grobid_dir) for entry in wave_2_pdfs
    ]:
        arxiv_id = str(entry["arxiv_id"])
        author = _first_author_from_tei(_tei_path(grobid_dir, arxiv_id))
        first_authors[arxiv_id] = author
        surname = (author.get("surname") or "").casefold()
        if arxiv_id in anchor_citing_sources or (anchor_surname and surname == anchor_surname):
            self_cluster_matches.append(arxiv_id)

    all_packets = [
        *wave_1_grobid_packets.values(),
        *wave_2_grobid_packets.values(),
        *wave_1_opendataloader_packets.values(),
        *wave_2_opendataloader_packets.values(),
    ]
    cumulative = _build_cumulative_corpus(
        existing_corpus=existing_corpus,
        wave_1_manifest=wave_1_manifest,
        wave_2_manifest=wave_2_manifest,
        wave_1_dir=wave_1_dir,
        wave_2_dir=wave_2_dir,
    )
    _atomic_write_json(wave_2_dir / "cumulative-corpus.json", cumulative)

    wave_1_edge_count = len(wave_1_edges)
    wave_2_edge_count = len(wave_2_edges)
    analysis = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "inputs": {
            "wave_1_dir": str(wave_1_dir.as_posix()),
            "wave_2_acquisition_log": str((wave_2_dir / "acquisition-log.json").as_posix()),
            "wave_2_manifest": str((wave_2_dir / "corpus-manifest.json").as_posix()),
            "existing_corpus": str(existing_corpus_path.as_posix()),
            "anchor_tei": str(
                (wave_1_dir / "anchor-grobid" / "tei" / f"{anchor_arxiv_id}.tei.xml").as_posix()
            ),
        },
        "safety_defaults": _safety_defaults(),
        "acquisition": {
            "success_count": int(wave_2_acquisition.get("success_count") or 0),
            "blocked_count": int(wave_2_acquisition.get("blocked_count") or 0),
            "network_error_count": int(wave_2_acquisition.get("network_error_count") or 0),
            "status_counts": dict(wave_2_acquisition.get("status_counts", {})),
        },
        "parser_quality": {
            "wave_1_grobid_packet_count": len(wave_1_grobid_packets),
            "wave_2_grobid_packet_count": len(wave_2_grobid_packets),
            "grobid_packet_count": len(wave_2_grobid_packets),
            "grobid_success_count": _success_count(wave_2_grobid_packets),
            "grobid_quality_counts": dict(sorted(_quality_counts(wave_2_grobid_packets).items())),
            "wave_1_opendataloader_packet_count": len(wave_1_opendataloader_packets),
            "wave_2_opendataloader_packet_count": len(wave_2_opendataloader_packets),
            "opendataloader_packet_count": len(wave_2_opendataloader_packets),
            "opendataloader_success_count": _success_count(wave_2_opendataloader_packets),
            "opendataloader_quality_counts": dict(
                sorted(_quality_counts(wave_2_opendataloader_packets).items())
            ),
            "all_packet_safety_defaults_false": all(
                _packet_safety_defaults_false(packet) for packet in all_packets
            ),
        },
        "connectivity": {
            "anchor_arxiv_id": anchor_arxiv_id,
            "existing_corpus_target_count": len(existing_ids),
            "target_count": len(target_ids),
            "wave_1_edge_count": wave_1_edge_count,
            "wave_2_new_edge_count": wave_2_edge_count,
            "connectivity_gain_delta_vs_wave_1": wave_2_edge_count - wave_1_edge_count,
            "cumulative_edge_count": len(cumulative_edge_records),
            "saturation_status": "saturated"
            if wave_2_edge_count <= wave_1_edge_count
            else "growing",
            "wave_1_edges": wave_1_edges,
            "wave_2_new_edges": wave_2_edges,
            "cumulative_edges": cumulative_edge_records,
        },
        "self_citation_cluster": {
            "anchor_first_author": anchor_author,
            "cumulative_wave_pdf_count": len(wave_1_pdfs) + len(wave_2_pdfs),
            "wave_2_pdf_count": len(wave_2_pdfs),
            "matching_cumulative_pdfs": len(self_cluster_matches),
            "matching_arxiv_ids": sorted(self_cluster_matches),
            "percent": round(
                (len(self_cluster_matches) / (len(wave_1_pdfs) + len(wave_2_pdfs)) * 100.0), 2
            )
            if (wave_1_pdfs or wave_2_pdfs)
            else 0.0,
            "first_authors": first_authors,
        },
        "category_distribution": dict(sorted(wave_2_category_distribution.items())),
        "cumulative_category_distribution": dict(sorted(cumulative_category_distribution.items())),
        "length_distribution": dict(sorted(wave_2_length_distribution.items())),
        "cumulative_length_distribution": dict(sorted(cumulative_length_distribution.items())),
        "cumulative_corpus": {
            "path": str((wave_2_dir / "cumulative-corpus.json").as_posix()),
            "actual_total": cumulative["actual_total"],
            "expected_total": cumulative["expected_total"],
        },
    }
    _atomic_write_json(wave_2_dir / "analysis.json", analysis)
    _atomic_write_text(wave_2_dir / "analysis.md", _render_markdown(analysis))
    return analysis


def _render_counts(counts: dict[str, Any]) -> str:
    return ", ".join(f"{key}: {value}" for key, value in sorted(counts.items())) or "none"


def _render_edge(edge: dict[str, str]) -> str:
    return f"{edge['source_arxiv_id']} -> {edge['target_arxiv_id']}"


def _render_markdown(analysis: dict[str, Any]) -> str:
    acquisition = analysis["acquisition"]
    parser = analysis["parser_quality"]
    connectivity = analysis["connectivity"]
    self_cluster = analysis["self_citation_cluster"]
    wave_2_edges = connectivity["wave_2_new_edges"]
    cumulative_edges = connectivity["cumulative_edges"]
    lines = [
        "# M056 Wave 2 Analysis",
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
        f"- GROBID packets: {parser['grobid_packet_count']}",
        f"- GROBID success: {parser['grobid_success_count']}",
        f"- GROBID quality counts: {_render_counts(parser['grobid_quality_counts'])}",
        f"- OpenDataLoader packets: {parser['opendataloader_packet_count']}",
        f"- OpenDataLoader success: {parser['opendataloader_success_count']}",
        f"- OpenDataLoader quality counts: {_render_counts(parser['opendataloader_quality_counts'])}",
        f"- Packet safety defaults all false: {parser['all_packet_safety_defaults_false']}",
        "",
        "## Connectivity gain",
        "",
        f"- Target set: 20 existing corpus PDFs + anchor `{connectivity['anchor_arxiv_id']}`",
        f"- Wave 1 directed edges to target set: {connectivity['wave_1_edge_count']}",
        f"- Wave 2 new directed edges to target set: {connectivity['wave_2_new_edge_count']}",
        f"- Delta vs Wave 1: {connectivity['connectivity_gain_delta_vs_wave_1']}",
        f"- Cumulative directed edges: {connectivity['cumulative_edge_count']}",
        f"- Saturation status: {connectivity['saturation_status']}",
        "",
        "### New edges added this wave",
        "",
    ]
    lines.extend([f"- {_render_edge(edge)}" for edge in wave_2_edges] or ["- none"])
    lines.extend(
        [
            "",
            "### Cumulative edges",
            "",
        ]
    )
    lines.extend([f"- {_render_edge(edge)}" for edge in cumulative_edges] or ["- none"])
    lines.extend(
        [
            "",
            "## Self-citation cluster",
            "",
            f"- Anchor first author: {self_cluster['anchor_first_author'].get('display') or 'unknown'}",
            f"- Matching cumulative Wave PDFs: {self_cluster['matching_cumulative_pdfs']} / {self_cluster['cumulative_wave_pdf_count']} ({self_cluster['percent']}%)",
            f"- Matching arXiv IDs: {', '.join(self_cluster['matching_arxiv_ids']) or 'none'}",
            "",
            "## Category distribution",
            "",
            f"- Wave 2: {_render_counts(analysis['category_distribution'])}",
            f"- Cumulative waves: {_render_counts(analysis['cumulative_category_distribution'])}",
            "",
            "## Length distribution",
            "",
            f"- Wave 2: {_render_counts(analysis['length_distribution'])}",
            f"- Cumulative waves: {_render_counts(analysis['cumulative_length_distribution'])}",
            "",
            "## Cumulative corpus",
            "",
            f"- Expected total: {analysis['cumulative_corpus']['expected_total']}",
            f"- Actual total: {analysis['cumulative_corpus']['actual_total']}",
            f"- Path: `{analysis['cumulative_corpus']['path']}`",
            "",
        ]
    )
    return "\n".join(lines)


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wave-1-dir", type=Path, default=DEFAULT_WAVE_1_DIR)
    parser.add_argument("--wave-2-dir", type=Path, default=DEFAULT_WAVE_2_DIR)
    parser.add_argument("--existing-corpus", type=Path, default=DEFAULT_EXISTING_CORPUS)
    parser.add_argument("--anchor-arxiv-id", default=DEFAULT_ANCHOR_ARXIV_ID)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    analysis = analyze_wave_2(
        wave_1_dir=args.wave_1_dir,
        wave_2_dir=args.wave_2_dir,
        existing_corpus_path=args.existing_corpus,
        anchor_arxiv_id=args.anchor_arxiv_id,
    )
    print(json.dumps(analysis, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
