#!/usr/bin/env python3
"""Build the M057 S03 figure-caption corpus from OpenDataLoader outputs.

The builder is intentionally read-only with respect to M055/M056 artifacts. It
extracts figure captions from markdown where available, falls back to packet
caption metadata for correctness packets that do not include markdown, and writes
only the M057 figure-links artifact.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Iterable
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "figure-caption-corpus.json"

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}

OPEN_DATALOADER_SOURCES: tuple[dict[str, str], ...] = (
    {
        "name": "m055deep-opendataloader-correctness",
        "path": "artifacts/m055deep-parser-benchmark/opendataloader-correctness",
    },
    {"name": "m056-wave-1", "path": "artifacts/m056-bfs-graph/wave-1/opendataloader"},
    {"name": "m056-wave-2", "path": "artifacts/m056-bfs-graph/wave-2/opendataloader"},
    {"name": "m056-wave-3", "path": "artifacts/m056-bfs-graph/wave-3/opendataloader"},
    {"name": "m056-wave-4", "path": "artifacts/m056-bfs-graph/wave-4/opendataloader"},
    {"name": "m056-wave-5", "path": "artifacts/m056-bfs-graph/wave-5/opendataloader"},
    {"name": "m056-wave-6", "path": "artifacts/m056-bfs-graph/wave-6/opendataloader"},
    {
        "name": "m056-missing-17",
        "path": "artifacts/m056-bfs-graph/missing-17-opendataloader",
    },
)

_WHITESPACE_RE = re.compile(r"\s+")
_FIGURE_START_RE = re.compile(
    r"^\s*(?:#+\s*)?(?:\*\*)?(?P<label>fig(?:ure)?\.?\s*(?P<number>\d+[A-Za-z]?(?:\.\d+)?))\s*[:.\-–—]?\s*(?P<caption>.*)$",
    re.IGNORECASE,
)
_STOP_RE = re.compile(
    r"^\s*(?:#{1,6}\s+|(?:table|algorithm|appendix|references|acknowledg(?:e)?ments)\b)",
    re.IGNORECASE,
)


def clean_text(value: Any) -> str:
    return _WHITESPACE_RE.sub(" ", str(value or "").replace("\u00ad", "")).strip()


def source_roots() -> list[tuple[str, Path]]:
    return [(source["name"], ROOT / source["path"]) for source in OPEN_DATALOADER_SOURCES]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _resolve_markdown_path(source_root: Path, packet: dict[str, Any], packet_path: Path) -> Path | None:
    markdown_path = packet.get("markdown_path")
    candidates: list[Path] = []
    if markdown_path:
        candidates.append(source_root / str(markdown_path))
        candidates.append(ROOT / str(markdown_path))
    arxiv_id = clean_text(packet.get("arxiv_id") or packet.get("article_key") or packet_path.stem)
    candidates.append(source_root / "markdown" / f"{arxiv_id}.md")
    for candidate in candidates:
        if candidate.exists():
            return candidate
    return None


def _caption_is_complete(text: str) -> bool:
    if not text:
        return False
    return text.endswith((".", "?", "!", ")", "]")) and len(text) >= 24


def extract_markdown_figure_captions(markdown_text: str) -> list[dict[str, Any]]:
    """Extract figure captions from markdown lines that start with Figure/Fig."""

    lines = markdown_text.splitlines()
    captions: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    index = 0
    while index < len(lines):
        line = clean_text(lines[index])
        match = _FIGURE_START_RE.match(line)
        if match is None:
            index += 1
            continue
        figure_label = clean_text(match.group("label")).rstrip(".")
        caption_parts = [clean_text(match.group("caption"))]
        start_line = index + 1
        lookahead = index + 1
        while lookahead < len(lines) and len(" ".join(caption_parts)) < 1200:
            candidate = clean_text(lines[lookahead])
            if not candidate:
                break
            if _FIGURE_START_RE.match(candidate) or _STOP_RE.match(candidate):
                break
            caption_parts.append(candidate)
            if _caption_is_complete(" ".join(caption_parts)):
                break
            lookahead += 1
        caption = clean_text(" ".join(part for part in caption_parts if part))
        key = (figure_label.lower(), caption.lower())
        if caption and key not in seen:
            seen.add(key)
            captions.append({"figure_label": figure_label, "caption": caption, "line_number": start_line})
        index += 1
    return captions


def _json_figure_captions(packet: dict[str, Any]) -> list[dict[str, Any]]:
    metrics = packet.get("correctness_metrics")
    raw_captions = metrics.get("captions") if isinstance(metrics, dict) else None
    if not isinstance(raw_captions, list):
        return []
    captions: list[dict[str, Any]] = []
    for item in raw_captions:
        if not isinstance(item, dict):
            continue
        figure_id = clean_text(item.get("figure_id"))
        caption_type = clean_text(item.get("caption_type")).lower()
        if caption_type != "figure" and not figure_id.lower().startswith(("figure", "fig.")):
            continue
        caption = clean_text(item.get("caption_text"))
        if not caption:
            continue
        captions.append(
            {
                "figure_label": figure_id or f"Figure {len(captions) + 1}",
                "caption": caption,
                "line_number": item.get("line_number"),
            }
        )
    return captions


def _source_milestone(source_name: str) -> str:
    if source_name.startswith("m055"):
        return "M055-kyxuqm"
    if source_name.startswith("m056"):
        return "M056"
    return source_name


def iter_figure_records(sources: Iterable[tuple[str, Path]]) -> Iterable[dict[str, Any]]:
    seen_figure_ids: set[str] = set()
    for source_name, source_root in sources:
        per_pdf = source_root / "per-pdf"
        if not per_pdf.exists():
            continue
        for packet_path in sorted(per_pdf.glob("*.json")):
            packet = load_json(packet_path)
            arxiv_id = clean_text(packet.get("arxiv_id") or packet.get("article_key") or packet_path.stem)
            if not arxiv_id:
                continue
            markdown_path = _resolve_markdown_path(source_root, packet, packet_path)
            if markdown_path is not None:
                captions = extract_markdown_figure_captions(
                    markdown_path.read_text(encoding="utf-8", errors="replace")
                )
            else:
                captions = _json_figure_captions(packet)
            source_pdf = clean_text(packet.get("pdf_path") or packet_path.with_suffix(".pdf"))
            for figure_idx, caption_info in enumerate(captions, start=1):
                figure_id = f"{arxiv_id}::{figure_idx}"
                if figure_id in seen_figure_ids:
                    continue
                caption = clean_text(caption_info["caption"])
                if not caption:
                    continue
                seen_figure_ids.add(figure_id)
                yield {
                    "figure_id": figure_id,
                    "arxiv_id": arxiv_id,
                    "figure_idx": figure_idx,
                    "figure_label": clean_text(caption_info.get("figure_label")) or f"Figure {figure_idx}",
                    "caption": caption,
                    "text_repr": f"Figure from {arxiv_id}: {caption}",
                    "source_milestone": _source_milestone(source_name),
                    "source_name": source_name,
                    "source_pdf": source_pdf,
                    "caption_source": "markdown" if markdown_path is not None else "packet-json",
                    "caption_line": caption_info.get("line_number"),
                }


def build_corpus(*, output_path: Path = DEFAULT_OUTPUT) -> dict[str, Any]:
    figures = list(iter_figure_records(source_roots()))
    payload: dict[str, Any] = {
        "schema_version": "m057.figure-caption-corpus.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "figure_count": len(figures),
        "figures": figures,
        "sources": [
            {"name": name, "path": str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path)}
            for name, path in source_roots()
        ],
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return payload


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    payload = build_corpus(output_path=args.output)
    print(json.dumps({"output": str(args.output), "figure_count": payload["figure_count"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
