#!/usr/bin/env python3
"""Build the M057 S02 table-text corpus from existing OpenDataLoader outputs.

The builder is intentionally read-only with respect to M055/M056 artifacts. It
normalizes table blocks into compact semantic strings for fd embedding and writes
only the M057 table-similarity artifact.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = (
    ROOT / "artifacts" / "m057-fd-marker" / "table-similarity" / "table-text-corpus.json"
)

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

_SEPARATOR_RE = re.compile(r"^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$")
_WHITESPACE_RE = re.compile(r"\s+")
MAX_CAPTION_CHARS = 320
MAX_HEADER_CHARS = 320
MAX_SAMPLE_CHARS = 640


@dataclass(frozen=True)
class TableBlock:
    caption: str
    header_row: str
    sample_rows: tuple[str, ...]
    line_number: int | None = None


def clean_text(value: Any) -> str:
    """Return a single-line text representation safe for JSON artifacts."""

    if value is None:
        return ""
    if isinstance(value, (list, tuple)):
        value = " | ".join(str(item) for item in value)
    text = str(value).replace("\x00", " ").replace("\ufeff", " ")
    text = text.strip().strip("|").strip()
    return _WHITESPACE_RE.sub(" ", text)


def split_markdown_row(line: str) -> list[str]:
    stripped = line.strip().strip("|")
    if not stripped:
        return []
    return [clean_text(cell) for cell in stripped.split("|")]


def is_separator_row(line: str) -> bool:
    return bool(_SEPARATOR_RE.match(line.strip()))


def _caption_from_context(lines: list[str], start_index: int, current_section: str) -> str:
    candidates: list[str] = []
    for previous in reversed(lines[max(0, start_index - 10) : start_index]):
        text = clean_text(previous.lstrip("# "))
        if text:
            candidates.append(text)
    for candidate in candidates:
        if "table" in candidate.lower():
            return candidate
    return candidates[0] if candidates else current_section


def _table_from_lines(block: list[str], caption: str, line_number: int) -> TableBlock | None:
    data_rows = [line for line in block if not is_separator_row(line)]
    if not data_rows:
        return None
    header_cells = split_markdown_row(data_rows[0])
    if not any(header_cells):
        return None
    body_rows = [" | ".join(split_markdown_row(line)) for line in data_rows[1:3]]
    return TableBlock(
        caption=clean_text(caption),
        header_row=" | ".join(header_cells),
        sample_rows=tuple(row for row in body_rows if row),
        line_number=line_number,
    )


def extract_markdown_tables(markdown_text: str) -> list[TableBlock]:
    """Extract contiguous markdown pipe-table blocks with nearby captions."""

    lines = markdown_text.splitlines()
    tables: list[TableBlock] = []
    current_section = ""
    index = 0
    while index < len(lines):
        stripped = lines[index].strip()
        if stripped.startswith("## "):
            current_section = clean_text(stripped.lstrip("# "))
        if stripped.startswith("|") and stripped.endswith("|"):
            start = index
            block: list[str] = []
            while index < len(lines):
                candidate = lines[index].strip()
                if candidate.startswith("|") and candidate.endswith("|"):
                    block.append(candidate)
                    index += 1
                    continue
                break
            caption = _caption_from_context(lines, start, current_section)
            table = _table_from_lines(block, caption, start + 1)
            if table is not None:
                tables.append(table)
            continue
        index += 1
    return tables


def _nearest_table_caption(packet: dict[str, Any], line_number: int | None) -> str:
    captions = packet.get("correctness_metrics", {}).get("captions", [])
    table_captions = [c for c in captions if c.get("caption_type") == "table"]
    if not table_captions:
        return ""
    if line_number is None:
        return clean_text(table_captions[0].get("caption_text"))
    nearest = min(
        table_captions,
        key=lambda caption: abs(int(caption.get("line_number") or line_number) - line_number),
    )
    return clean_text(nearest.get("caption_text"))


def _json_table_blocks(packet: dict[str, Any]) -> list[TableBlock]:
    tables = packet.get("tables") or packet.get("correctness_metrics", {}).get("tables") or []
    blocks: list[TableBlock] = []
    for table in tables:
        line_number = table.get("line_number")
        headers = table.get("headers") or table.get("columns") or table.get("header") or []
        body_rows = table.get("body_rows") or table.get("rows") or []
        if isinstance(body_rows, int):
            body_rows = []
        normalized_rows: list[str] = []
        for row in body_rows[:2]:
            normalized_rows.append(clean_text(row))
        header_row = clean_text(headers) or clean_text(table.get("table_id"))
        if not header_row and not normalized_rows:
            continue
        blocks.append(
            TableBlock(
                caption=_nearest_table_caption(packet, int(line_number) if line_number else None),
                header_row=header_row,
                sample_rows=tuple(row for row in normalized_rows if row),
                line_number=int(line_number) if line_number else None,
            )
        )
    return blocks


def _resolve_markdown_path(
    source_root: Path, packet: dict[str, Any], packet_path: Path
) -> Path | None:
    markdown_path = packet.get("markdown_path")
    candidates: list[Path] = []
    if markdown_path:
        candidates.append(source_root / str(markdown_path))
        candidates.append(ROOT / str(markdown_path))
    arxiv_id = clean_text(packet.get("arxiv_id") or packet_path.stem)
    candidates.append(source_root / "markdown" / f"{arxiv_id}.md")
    for candidate in candidates:
        if candidate.exists():
            return candidate
    return None


def source_roots() -> list[tuple[str, Path]]:
    return [(source["name"], ROOT / source["path"]) for source in OPEN_DATALOADER_SOURCES]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def truncate_text(text: str, max_chars: int) -> str:
    cleaned = clean_text(text)
    if len(cleaned) <= max_chars:
        return cleaned
    return cleaned[: max_chars - 1].rstrip() + "…"


def semantic_text(arxiv_id: str, block: TableBlock) -> str:
    caption = truncate_text(block.caption or "uncaptioned table", MAX_CAPTION_CHARS)
    header = truncate_text(block.header_row or "unknown columns", MAX_HEADER_CHARS)
    sample_raw = " ; ".join(block.sample_rows) if block.sample_rows else "no sample rows"
    sample = truncate_text(sample_raw, MAX_SAMPLE_CHARS)
    return f"Table from {arxiv_id} {caption}. Columns: {header}. Sample: {sample}"


def iter_table_records(sources: Iterable[tuple[str, Path]]) -> Iterable[dict[str, Any]]:
    seen_table_ids: set[str] = set()
    for source_name, source_root in sources:
        per_pdf = source_root / "per-pdf"
        if not per_pdf.exists():
            continue
        for packet_path in sorted(per_pdf.glob("*.json")):
            packet = load_json(packet_path)
            arxiv_id = clean_text(
                packet.get("arxiv_id") or packet.get("article_key") or packet_path.stem
            )
            if not arxiv_id:
                continue
            markdown_path = _resolve_markdown_path(source_root, packet, packet_path)
            if markdown_path is not None:
                blocks = extract_markdown_tables(
                    markdown_path.read_text(encoding="utf-8", errors="replace")
                )
            else:
                blocks = _json_table_blocks(packet)
            for index, block in enumerate(blocks, start=1):
                table_id = f"{arxiv_id}::{index}"
                if table_id in seen_table_ids:
                    continue
                seen_table_ids.add(table_id)
                yield {
                    "table_id": table_id,
                    "arxiv_id": arxiv_id,
                    "table_idx": index,
                    "caption": block.caption,
                    "header_row": block.header_row,
                    "sample_rows": list(block.sample_rows),
                    "line_number": block.line_number,
                    "text_repr": semantic_text(arxiv_id, block),
                    "source_milestone": source_name,
                    "source_pdf": clean_text(
                        packet.get("pdf_path") or packet.get("source_pdf") or ""
                    ),
                }


def build_corpus(
    *,
    output_path: Path = DEFAULT_OUTPUT,
    sources: Iterable[tuple[str, Path]] | None = None,
) -> dict[str, Any]:
    selected_sources = list(sources) if sources is not None else source_roots()
    tables = sorted(iter_table_records(selected_sources), key=lambda item: item["table_id"])
    payload = {
        "schema_version": "m057.table-text-corpus.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "source_roots": [
            str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path)
            for _, path in selected_sources
        ],
        "table_count": len(tables),
        "tables": tables,
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return payload


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    payload = build_corpus(output_path=args.output)
    sys.stdout.write(
        json.dumps(
            {"output": str(args.output), "table_count": payload["table_count"]}, sort_keys=True
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
