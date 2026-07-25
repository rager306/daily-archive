#!/usr/bin/env python3
"""Analyze M056 Wave 1 parser outputs and build a cumulative corpus manifest.

The analysis is evidence-only: it reads acquisition, GROBID, OpenDataLoader, and
prior corpus artifacts, then writes markdown/JSON summaries. It does not write
or import graph data; all safety defaults remain false.
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

SCHEMA_VERSION = "m056-bfs-wave-1-analysis.v1"
DEFAULT_WAVE_DIR = Path("artifacts/m056-bfs-graph/wave-1")
DEFAULT_EXISTING_CORPUS = Path("artifacts/m055deep-parser-benchmark/corpus-manifest-20.json")
DEFAULT_ANCHOR_ARXIV_ID = "2605.18747"
ARXIV_ID_RE = re.compile(r"(?<!\d)(\d{4}\.\d{4,5})(?:v\d+)?(?!\d)")
SUPPORTED_CATEGORIES = ("cs-ai", "cs-cl", "cs-cv", "cs-lg", "mixed-source")


def _utc_now() -> str:
    return dt.datetime.now(tz=dt.UTC).isoformat()


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
    author = root.find(".//tei:teiHeader//tei:fileDesc//tei:titleStmt//tei:author", ns)
    if author is None:
        author = root.find(".//tei:sourceDesc//tei:biblStruct//tei:analytic//tei:author", ns)
    if author is None:
        return {"forename": None, "surname": None, "display": None}
    forename = author.findtext(".//tei:forename", default=None, namespaces=ns)
    surname = author.findtext(".//tei:surname", default=None, namespaces=ns)
    if surname is None:
        pers_name = author.find(".//tei:persName", ns)
        if pers_name is not None:
            text = " ".join(part.strip() for part in pers_name.itertext() if part.strip())
            surname = text.split()[-1] if text else None
            forename = " ".join(text.split()[:-1]) or None if text else None
    display = " ".join(part for part in [forename, surname] if part) or None
    return {"forename": forename, "surname": surname, "display": display}


def _length_bucket(pages: int) -> str:
    if pages <= 10:
        return "short"
    if pages <= 30:
        return "medium"
    return "long"


def _quality_status(packet: dict[str, Any]) -> str:
    if packet.get("status") == "success" and not packet.get("low_quality_source"):
        return "success"
    if packet.get("low_quality_source"):
        return "low_quality_source"
    return str(packet.get("status") or "blocked")


def _summary_counts(packets: dict[str, dict[str, Any]]) -> dict[str, int]:
    counts = Counter(_quality_status(packet) for packet in packets.values())
    return dict(sorted(counts.items()))


def _packet_safety_defaults_false(packet: dict[str, Any]) -> bool:
    safety = packet.get("safety_defaults")
    return (
        isinstance(safety, dict)
        and bool(safety)
        and all(value is False for value in safety.values())
    )


def _edge_records(
    *,
    wave_pdfs: list[dict[str, Any]],
    grobid_dir: Path,
    target_ids: set[str],
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


def _build_cumulative_corpus(
    existing_corpus: dict[str, Any], wave_manifest: dict[str, Any]
) -> dict[str, Any]:
    existing_pdfs = []
    for entry in existing_corpus.get("pdfs", []):
        copied = dict(entry)
        copied.setdefault("source_milestone", entry.get("source_milestone") or "M055deep")
        existing_pdfs.append(copied)
    wave_pdfs = []
    for entry in wave_manifest.get("pdfs", []):
        copied = dict(entry)
        copied["source_milestone"] = "M056-lchpnp/S01"
        wave_pdfs.append(copied)
    return {
        "schema_version": "m056-bfs-wave-1-cumulative-corpus.v1",
        "generated_at": _utc_now(),
        "inputs": {
            "existing_corpus": str(DEFAULT_EXISTING_CORPUS.as_posix()),
            "wave_manifest": str((DEFAULT_WAVE_DIR / "corpus-manifest.json").as_posix()),
        },
        "expected_total": 50,
        "actual_total": len(existing_pdfs) + len(wave_pdfs),
        "source_counts": dict(
            Counter(
                str(entry.get("source_milestone", "unknown")) for entry in existing_pdfs + wave_pdfs
            )
        ),
        "safety_defaults": _safety_defaults(),
        "pdfs": existing_pdfs + wave_pdfs,
    }


def analyze_wave_1(
    *,
    wave_dir: Path,
    existing_corpus_path: Path,
    anchor_arxiv_id: str,
) -> dict[str, Any]:
    acquisition_log = _load_json(wave_dir / "acquisition-log.json")
    wave_manifest = _load_json(wave_dir / "corpus-manifest.json")
    existing_corpus = _load_json(existing_corpus_path)
    grobid_dir = wave_dir / "grobid-fulltext"
    opendataloader_dir = wave_dir / "opendataloader"
    grobid_packets = _load_packets(grobid_dir)
    opendataloader_packets = _load_packets(opendataloader_dir)
    wave_pdfs = list(wave_manifest.get("pdfs", []))
    existing_ids = {
        str(entry.get("arxiv_id"))
        for entry in existing_corpus.get("pdfs", [])
        if entry.get("arxiv_id")
    }
    target_ids = set(existing_ids)
    target_ids.add(anchor_arxiv_id)
    edges = _edge_records(wave_pdfs=wave_pdfs, grobid_dir=grobid_dir, target_ids=target_ids)

    category_distribution = Counter(
        str(entry.get("category", "mixed-source")) for entry in wave_pdfs
    )
    for category in SUPPORTED_CATEGORIES:
        category_distribution.setdefault(category, 0)
    length_distribution = Counter(
        _length_bucket(int(entry.get("pages_estimate") or 0)) for entry in wave_pdfs
    )
    for bucket in ("short", "medium", "long"):
        length_distribution.setdefault(bucket, 0)

    first_authors: dict[str, dict[str, str | None]] = {}
    anchor_author = {"forename": None, "surname": None, "display": None}
    anchor_tei_candidates = [
        wave_dir / "anchor-grobid" / "tei" / f"{anchor_arxiv_id}.tei.xml",
        wave_dir / "grobid-fulltext" / "tei" / f"{anchor_arxiv_id}.tei.xml",
    ]
    for candidate in anchor_tei_candidates:
        if candidate.exists():
            anchor_author = _first_author_from_tei(candidate)
            break

    anchor_surname = (anchor_author.get("surname") or "").casefold()
    self_cluster_matches = 0
    anchor_citing_sources = {
        edge["source_arxiv_id"] for edge in edges if edge["target_arxiv_id"] == anchor_arxiv_id
    }
    for entry in wave_pdfs:
        arxiv_id = str(entry["arxiv_id"])
        author = _first_author_from_tei(_tei_path(grobid_dir, arxiv_id))
        first_authors[arxiv_id] = author
        surname = (author.get("surname") or "").casefold()
        if arxiv_id in anchor_citing_sources or (anchor_surname and surname == anchor_surname):
            self_cluster_matches += 1

    wave_count = len(wave_pdfs)
    self_cluster_percent = (
        round((self_cluster_matches / wave_count * 100.0), 2) if wave_count else 0.0
    )
    cumulative = _build_cumulative_corpus(existing_corpus, wave_manifest)
    _atomic_write_json(wave_dir / "cumulative-corpus.json", cumulative)

    analysis = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": _utc_now(),
        "inputs": {
            "acquisition_log": str((wave_dir / "acquisition-log.json").as_posix()),
            "wave_manifest": str((wave_dir / "corpus-manifest.json").as_posix()),
            "grobid_dir": str(grobid_dir.as_posix()),
            "opendataloader_dir": str(opendataloader_dir.as_posix()),
            "existing_corpus": str(existing_corpus_path.as_posix()),
        },
        "safety_defaults": _safety_defaults(),
        "acquisition": {
            "success_count": acquisition_log.get("success_count", 0),
            "blocked_count": acquisition_log.get("blocked_count", 0),
            "network_error_count": acquisition_log.get("network_error_count", 0),
            "status_counts": acquisition_log.get("status_counts", {}),
        },
        "parser_quality": {
            "grobid_packet_count": len(grobid_packets),
            "grobid_success_count": sum(
                1 for packet in grobid_packets.values() if packet.get("status") == "success"
            ),
            "grobid_quality_counts": _summary_counts(grobid_packets),
            "opendataloader_packet_count": len(opendataloader_packets),
            "opendataloader_success_count": sum(
                1 for packet in opendataloader_packets.values() if packet.get("status") == "success"
            ),
            "opendataloader_quality_counts": _summary_counts(opendataloader_packets),
            "all_packet_safety_defaults_false": all(
                _packet_safety_defaults_false(packet)
                for packet in list(grobid_packets.values()) + list(opendataloader_packets.values())
            ),
        },
        "connectivity": {
            "target_count": len(target_ids),
            "existing_corpus_target_count": len(existing_ids),
            "anchor_arxiv_id": anchor_arxiv_id,
            "new_edge_count": len(edges),
            "edges": edges,
        },
        "self_citation_cluster": {
            "anchor_first_author": anchor_author,
            "wave_pdf_count": wave_count,
            "match_count": self_cluster_matches,
            "percent": self_cluster_percent,
            "first_authors": first_authors,
        },
        "category_distribution": dict(sorted(category_distribution.items())),
        "length_distribution": dict(sorted(length_distribution.items())),
        "cumulative_corpus": {
            "path": str((wave_dir / "cumulative-corpus.json").as_posix()),
            "actual_total": cumulative["actual_total"],
            "expected_total": cumulative["expected_total"],
        },
    }
    _atomic_write_json(wave_dir / "analysis.json", analysis)
    _atomic_write_text(wave_dir / "analysis.md", _render_markdown(analysis))
    return analysis


def _render_counts(counts: dict[str, Any]) -> str:
    return ", ".join(f"{key}: {value}" for key, value in sorted(counts.items())) or "none"


def _render_markdown(analysis: dict[str, Any]) -> str:
    acquisition = analysis["acquisition"]
    parser = analysis["parser_quality"]
    connectivity = analysis["connectivity"]
    self_cluster = analysis["self_citation_cluster"]
    lines = [
        "# M056 Wave 1 Analysis",
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
        f"- Target set: {connectivity['existing_corpus_target_count']} existing corpus PDFs + anchor `{connectivity['anchor_arxiv_id']}`",
        f"- New directed edges from Wave 1 PDFs to target set: {connectivity['new_edge_count']}",
        "",
        "## Self-citation cluster",
        "",
        f"- Anchor first author: {self_cluster['anchor_first_author'].get('display') or 'unknown'}",
        f"- Matching Wave 1 PDFs: {self_cluster['match_count']} / {self_cluster['wave_pdf_count']} ({self_cluster['percent']}%)",
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
        f"- Path: `{analysis['cumulative_corpus']['path']}`",
        f"- Total PDFs: {analysis['cumulative_corpus']['actual_total']} / {analysis['cumulative_corpus']['expected_total']}",
        "",
    ]
    if connectivity["edges"]:
        lines.extend(["### Edges", ""])
        for edge in connectivity["edges"]:
            lines.append(f"- `{edge['source_arxiv_id']}` -> `{edge['target_arxiv_id']}`")
        lines.append("")
    return "\n".join(lines)


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wave-dir", type=Path, default=DEFAULT_WAVE_DIR)
    parser.add_argument("--existing-corpus", type=Path, default=DEFAULT_EXISTING_CORPUS)
    parser.add_argument("--anchor-arxiv-id", default=DEFAULT_ANCHOR_ARXIV_ID)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    analysis = analyze_wave_1(
        wave_dir=args.wave_dir,
        existing_corpus_path=args.existing_corpus,
        anchor_arxiv_id=args.anchor_arxiv_id,
    )
    print(json.dumps(analysis, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
