#!/usr/bin/env python3
"""Emit M056 candidate citation edges from existing GROBID TEI packets.

This is a diagnostic-only synthesis artifact for M056 S07. It reads the six
1-hop BFS wave directories, extracts arXiv identifiers from GROBID
``listBibl/biblStruct`` references, and writes a deterministic JSON packet. It
never writes graph data, never attempts production import, and leaves all five
safety defaults false.
"""

from __future__ import annotations

import argparse
import json
import re
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m056-bfs-candidate-edges.v1"
DEFAULT_BFS_DIR = Path("artifacts/m056-bfs-graph")
DEFAULT_OUTPUT = DEFAULT_BFS_DIR / "candidate-edges.json"
ANCHOR_ARXIV_ID = "2605.18747"
SOURCE_MILESTONE = "M056-lchpnp"
TEI_NS = {"tei": "http://www.tei-c.org/ns/1.0"}
ARXIV_RE = re.compile(r"(?i)(?:arxiv\s*:?\s*)?(\d{4}\.\d{4,5})(?:v\d+)?")

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "import_eligible": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
}

SAFETY_FLAGS: dict[str, bool] = {
    "graph_writes": False,
    "production_import_attempted": False,
    "promotion_allowed": False,
    "facts_promoted": False,
    "external_mutation_allowed": False,
}


def _read_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"Expected JSON object in {path}")
    return payload


def _safe_text(element: ET.Element | None) -> str | None:
    if element is None:
        return None
    text = " ".join(part.strip() for part in element.itertext() if part.strip())
    return " ".join(text.split()) or None


def _extract_arxiv_ids(text: str | None) -> list[str]:
    if not text:
        return []
    return sorted({match.group(1) for match in ARXIV_RE.finditer(text)})


def _title_from_tei(root: ET.Element) -> str | None:
    for query in (
        ".//tei:teiHeader//tei:titleStmt/tei:title[@type='main']",
        ".//tei:teiHeader//tei:titleStmt/tei:title",
    ):
        title = _safe_text(root.find(query, TEI_NS))
        if title:
            return title
    return None


def _title_from_biblstruct(bibl: ET.Element) -> str | None:
    for query in (
        "tei:analytic/tei:title[@level='a']",
        "tei:analytic/tei:title",
        "tei:monogr/tei:title",
    ):
        title = _safe_text(bibl.find(query, TEI_NS))
        if title:
            return title
    return None


def _load_corpus_nodes(bfs_dir: Path) -> dict[str, dict[str, Any]]:
    nodes: dict[str, dict[str, Any]] = {}

    def add_manifest(manifest_path: Path) -> None:
        if not manifest_path.exists():
            return
        payload = _read_json(manifest_path)
        for pdf in payload.get("pdfs", []):
            if not isinstance(pdf, dict):
                continue
            arxiv_id = str(pdf.get("arxiv_id") or pdf.get("requested_arxiv_id") or "").strip()
            if not arxiv_id:
                continue
            nodes.setdefault(
                arxiv_id,
                {
                    "arxiv_id": arxiv_id,
                    "title": None,
                    "source_milestone": pdf.get("source_milestone") or SOURCE_MILESTONE,
                    "in_corpus": True,
                    "category": pdf.get("category"),
                    "first_seen_wave": None,
                },
            )
            nodes[arxiv_id]["in_corpus"] = True
            nodes[arxiv_id]["source_milestone"] = (
                pdf.get("source_milestone") or nodes[arxiv_id].get("source_milestone") or SOURCE_MILESTONE
            )
            nodes[arxiv_id]["category"] = pdf.get("category") or nodes[arxiv_id].get("category")

    for wave in range(1, 7):
        add_manifest(bfs_dir / f"wave-{wave}" / "anchor-manifest.json")
        add_manifest(bfs_dir / f"wave-{wave}" / "corpus-manifest.json")
        for arxiv_id in nodes:
            if nodes[arxiv_id].get("first_seen_wave") is None:
                # The loop has just loaded all manifests up to this wave; preserve earliest wave lazily.
                seen_in_wave = False
                for name in ("anchor-manifest.json", "corpus-manifest.json"):
                    path = bfs_dir / f"wave-{wave}" / name
                    if path.exists():
                        payload = _read_json(path)
                        seen_in_wave = any(
                            str(pdf.get("arxiv_id") or pdf.get("requested_arxiv_id")) == arxiv_id
                            for pdf in payload.get("pdfs", [])
                            if isinstance(pdf, dict)
                        )
                    if seen_in_wave:
                        nodes[arxiv_id]["first_seen_wave"] = wave
                        break

    nodes.setdefault(
        ANCHOR_ARXIV_ID,
        {
            "arxiv_id": ANCHOR_ARXIV_ID,
            "title": None,
            "source_milestone": SOURCE_MILESTONE,
            "in_corpus": True,
            "category": "cs-cl",
            "first_seen_wave": 1,
        },
    )
    return nodes


def _tei_paths(bfs_dir: Path) -> list[Path]:
    paths: list[Path] = []
    for wave in range(1, 7):
        wave_dir = bfs_dir / f"wave-{wave}"
        paths.extend(sorted((wave_dir / "anchor-grobid" / "tei").glob("*.tei.xml")))
        paths.extend(sorted((wave_dir / "grobid-fulltext" / "tei").glob("*.tei.xml")))
    return paths


def _source_arxiv_id(path: Path) -> str:
    return path.name.removesuffix(".tei.xml")


def build_candidate_edges(bfs_dir: Path = DEFAULT_BFS_DIR) -> dict[str, Any]:
    """Build the deterministic candidate-edge payload from existing TEI files."""
    if not bfs_dir.exists():
        raise FileNotFoundError(f"M056 BFS directory not found: {bfs_dir}")

    nodes = _load_corpus_nodes(bfs_dir)
    corpus_ids = {arxiv_id for arxiv_id, node in nodes.items() if node.get("in_corpus")}
    edge_counts: Counter[tuple[str, str]] = Counter()
    biblstruct_evidence_count = 0
    tei_file_count = 0
    parse_errors: list[dict[str, str]] = []

    for tei_path in _tei_paths(bfs_dir):
        source_id = _source_arxiv_id(tei_path)
        tei_file_count += 1
        try:
            root = ET.parse(tei_path).getroot()
        except ET.ParseError as exc:
            parse_errors.append({"path": str(tei_path), "error": str(exc)})
            continue

        source_node = nodes.setdefault(
            source_id,
            {
                "arxiv_id": source_id,
                "title": None,
                "source_milestone": SOURCE_MILESTONE,
                "in_corpus": source_id in corpus_ids,
                "category": None,
                "first_seen_wave": None,
            },
        )
        source_node["title"] = source_node.get("title") or _title_from_tei(root)
        source_node["in_corpus"] = source_id in corpus_ids

        for bibl in root.findall(".//tei:listBibl/tei:biblStruct", TEI_NS):
            bibl_text = " ".join(part.strip() for part in bibl.itertext() if part.strip())
            cited_ids = _extract_arxiv_ids(bibl_text)
            if not cited_ids:
                continue
            biblstruct_evidence_count += 1
            cited_title = _title_from_biblstruct(bibl)
            for cited_id in cited_ids:
                if cited_id == source_id:
                    continue
                edge_counts[(source_id, cited_id)] += 1
                cited_node = nodes.setdefault(
                    cited_id,
                    {
                        "arxiv_id": cited_id,
                        "title": None,
                        "source_milestone": "external-reference",
                        "in_corpus": cited_id in corpus_ids,
                        "category": None,
                        "first_seen_wave": None,
                    },
                )
                cited_node["in_corpus"] = cited_id in corpus_ids
                cited_node["title"] = cited_node.get("title") or cited_title
                if cited_id in corpus_ids and cited_node.get("source_milestone") == "external-reference":
                    cited_node["source_milestone"] = SOURCE_MILESTONE

    edges = [
        {
            "paper_a": source_id,
            "paper_b": cited_id,
            "edge_type": "cites",
            "citation_count": count,
            "evidence": "grobid_biblstruct",
            "paper_a_in_corpus": source_id in corpus_ids,
            "paper_b_in_corpus": cited_id in corpus_ids,
        }
        for (source_id, cited_id), count in sorted(edge_counts.items())
    ]

    node_list = [
        {
            "arxiv_id": node["arxiv_id"],
            "title": node.get("title"),
            "source_milestone": node.get("source_milestone"),
            "in_corpus": bool(node.get("in_corpus")),
            "category": node.get("category"),
            "first_seen_wave": node.get("first_seen_wave"),
        }
        for node in sorted(nodes.values(), key=lambda item: str(item.get("arxiv_id")))
    ]

    internal_edges = [edge for edge in edges if edge["paper_a_in_corpus"] and edge["paper_b_in_corpus"]]
    return {
        "schema_version": SCHEMA_VERSION,
        "source_milestone": SOURCE_MILESTONE,
        "source_artifact": str(bfs_dir),
        "diagnostic_only": True,
        "graph_writes_authorized": False,
        "production_import_authorized": False,
        "safety_defaults": SAFETY_DEFAULTS,
        "safety_flags": SAFETY_FLAGS,
        "summary": {
            "anchor_arxiv_id": ANCHOR_ARXIV_ID,
            "corpus_unique_pdf_count": len(corpus_ids),
            "tei_file_count": tei_file_count,
            "node_count": len(node_list),
            "edge_count": len(edges),
            "internal_corpus_edge_count": len(internal_edges),
            "biblstructs_with_arxiv_evidence": biblstruct_evidence_count,
            "parse_error_count": len(parse_errors),
        },
        "nodes": node_list,
        "edges": edges,
        "parse_errors": parse_errors,
    }


def write_candidate_edges(payload: dict[str, Any], output: Path) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bfs-dir", type=Path, default=DEFAULT_BFS_DIR)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args(argv)

    payload = build_candidate_edges(args.bfs_dir)
    write_candidate_edges(payload, args.output)
    print(
        "wrote "
        f"{args.output} nodes={payload['summary']['node_count']} "
        f"edges={payload['summary']['edge_count']} "
        f"internal={payload['summary']['internal_corpus_edge_count']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
