#!/usr/bin/env python3
"""M058-cmjp1u S02: run Marker on the stage-1 five-PDF pilot sample.

The script is intentionally idempotent: each run rewrites the same per-PDF
packets and summary under artifacts/m058-marker/pilot-5/.
"""
from __future__ import annotations

import json
import re
import tempfile
import time
from dataclasses import dataclass
from importlib import metadata
from pathlib import Path
from typing import Any

from marker.config.parser import ConfigParser
from marker.converters.pdf import PdfConverter
from marker.models import create_model_dict
from marker.output import text_from_rendered
from pypdf import PdfReader, PdfWriter

CORPUS_ROOT = Path("data/article_catalog/article_catalog/arxiv")
OUTPUT_ROOT = Path("artifacts/m058-marker/pilot-5")
PER_PDF_DIR = OUTPUT_ROOT / "per-pdf"
LOOPBACK_BIND_HOST = "127.0.0.1"
PILOT_PAGE_RANGE = "0"
PILOT_MAX_PAGES = 1

SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "fact_promotion_authorized": False,
    "graph_writes_authorized": False,
    "llm_calls_authorized": False,
    "production_import_authorized": False,
}

REQUESTED_SAMPLE = [
    "2603.21520",
    "2605.28617v1",
    "2508.07434",
    "2412.15118",
    "2305.14314",
]

UNAVAILABLE_REQUESTED_SAMPLE = [
    {
        "arxiv_id": "2305.14314",
        "reason": "No local PDF, TeX, or M058 S01 plotextractor artifact exists for this ID in the repository.",
        "replacement_arxiv_id": "1804.02767",
    }
]


@dataclass(frozen=True)
class PilotPdf:
    arxiv_id: str
    category: str
    pdf_relpath: str
    source: str

    @property
    def pdf_path(self) -> Path:
        return CORPUS_ROOT / self.pdf_relpath


# 5-PDF executable sample: four IDs from the S02 plan plus one available M058 S01
# plotextractor PDF. The requested 2305.14314 is recorded above as unavailable.
SAMPLE_PDFS: tuple[PilotPdf, ...] = (
    PilotPdf("2603.21520", "cs-cl", "cs-cl/2603.21520/source/2603.21520.pdf", "M057 S01-fix sample"),
    PilotPdf("2605.28617v1", "cs-ai", "cs-ai/2605.28617v1/source/original.pdf", "M057 S01-fix sample"),
    PilotPdf("2508.07434", "cs-lg", "cs-cl/2508.07434/source/2508.07434.pdf", "M057 S01-fix sample"),
    PilotPdf("2412.15118", "cs-cv", "cs-cl/2412.15118/source/2412.15118.pdf", "M057 S01-fix sample"),
    PilotPdf("1804.02767", "cs-cv", "cs-cv/1804.02767/source/1804.02767.pdf", "M058 S01 plotextractor pilot"),
)

TABLE_ROW_RE = re.compile(r"^\s*\|.+\|\s*$", re.MULTILINE)
FIGURE_RE = re.compile(r"\b(?:figure|fig\.)\s+\d+", re.IGNORECASE)
DISPLAY_EQUATION_RE = re.compile(r"\$\$.*?\$\$|\\\[.*?\\\]", re.DOTALL)
INLINE_EQUATION_RE = re.compile(r"(?<!\$)\$(?!\$).+?(?<!\$)\$(?!\$)", re.DOTALL)


def package_version(package: str) -> str:
    """Return an installed package version without failing extraction."""
    try:
        return metadata.version(package)
    except metadata.PackageNotFoundError:
        return "unknown"


def count_markdown_tables(markdown: str) -> int:
    """Count markdown tables by separator rows instead of raw pipe lines."""
    rows = TABLE_ROW_RE.findall(markdown)
    separator_rows = [row for row in rows if re.search(r"\|\s*:?-{3,}:?\s*(?:\||$)", row)]
    return len(separator_rows)


def make_pilot_pdf(source_pdf: Path, temp_dir: Path, arxiv_id: str) -> tuple[Path, int, int]:
    """Create a bounded first-pages PDF for the S02 Marker cost pilot."""
    reader = PdfReader(str(source_pdf))
    source_page_count = len(reader.pages)
    pilot_page_count = min(PILOT_MAX_PAGES, source_page_count)
    writer = PdfWriter()
    for page_index in range(pilot_page_count):
        writer.add_page(reader.pages[page_index])
    pilot_pdf = temp_dir / f"{arxiv_id}-pilot-pages.pdf"
    with pilot_pdf.open("wb") as handle:
        writer.write(handle)
    return pilot_pdf, source_page_count, pilot_page_count


def marker_packet_for_pdf(converter: PdfConverter, sample: PilotPdf, temp_dir: Path) -> dict[str, Any]:
    """Extract one bounded pilot PDF with Marker and return the normalized M058 packet."""
    pdf_path = sample.pdf_path
    common: dict[str, Any] = {
        "schema_version": "m058.marker-pilot.per-pdf.v1",
        "arxiv_id": sample.arxiv_id,
        "category": sample.category,
        "pdf_path": str(pdf_path),
        "source": sample.source,
        "safety_defaults": SAFETY_DEFAULTS,
        "loopback_bind_host": LOOPBACK_BIND_HOST,
        "page_range": PILOT_PAGE_RANGE,
        "pilot_max_pages": PILOT_MAX_PAGES,
        "marker_version": package_version("marker-pdf"),
        "transformers_version": package_version("transformers"),
    }
    if not pdf_path.exists():
        return {
            **common,
            "status": "pdf_not_found",
            "table_count": 0,
            "figure_count": 0,
            "equation_count": 0,
            "body_word_count": 0,
            "markdown_length": 0,
            "elapsed_sec": 0.0,
            "error": f"PDF is not available at {pdf_path}",
        }

    start = time.time()
    try:
        pilot_pdf, source_page_count, marker_input_page_count = make_pilot_pdf(pdf_path, temp_dir, sample.arxiv_id)
        rendered = converter(str(pilot_pdf))
        markdown, _metadata, _images = text_from_rendered(rendered)
        elapsed_sec = time.time() - start
        return {
            **common,
            "status": "marker_extracted",
            "source_page_count": source_page_count,
            "marker_input_page_count": marker_input_page_count,
            "table_count": count_markdown_tables(markdown),
            "figure_count": len(FIGURE_RE.findall(markdown)),
            "equation_count": len(DISPLAY_EQUATION_RE.findall(markdown)) + len(INLINE_EQUATION_RE.findall(markdown)),
            "body_word_count": len(markdown.split()),
            "markdown_length": len(markdown),
            "elapsed_sec": round(elapsed_sec, 3),
        }
    except Exception as exc:  # noqa: BLE001 - packet must preserve extraction failure state.
        elapsed_sec = time.time() - start
        return {
            **common,
            "status": "marker_failed",
            "table_count": 0,
            "figure_count": 0,
            "equation_count": 0,
            "body_word_count": 0,
            "markdown_length": 0,
            "elapsed_sec": round(elapsed_sec, 3),
            "error": str(exc)[:500],
        }


def run_marker_pilot() -> dict[str, Any]:
    """Run the five-PDF Marker pilot and write per-PDF packets + summary."""
    OUTPUT_ROOT.mkdir(parents=True, exist_ok=True)
    PER_PDF_DIR.mkdir(parents=True, exist_ok=True)

    print("Loading Marker models...", flush=True)
    config = ConfigParser({"page_range": PILOT_PAGE_RANGE}).generate_config_dict()
    converter = PdfConverter(artifact_dict=create_model_dict(), config=config)

    results: list[dict[str, Any]] = []
    total_start = time.time()
    with tempfile.TemporaryDirectory(prefix="m058-marker-pilot-") as tmp:
        temp_dir = Path(tmp)
        for sample in SAMPLE_PDFS:
            size_kb = sample.pdf_path.stat().st_size // 1024 if sample.pdf_path.exists() else 0
            print(f"  extracting {sample.arxiv_id} ({size_kb} KB)...", flush=True)
            packet = marker_packet_for_pdf(converter, sample, temp_dir)
            results.append(packet)
            (PER_PDF_DIR / f"{sample.arxiv_id}.json").write_text(
                json.dumps(packet, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            print(
                f"    {packet['status']} chars={packet['markdown_length']} "
                f"words={packet['body_word_count']} sec={packet['elapsed_sec']}",
                flush=True,
            )

    total_elapsed_sec = round(time.time() - total_start, 3)
    extracted = [packet for packet in results if packet["status"] == "marker_extracted"]
    summary = {
        "schema_version": "m058.marker-pilot.summary.v1",
        "sample_size": len(SAMPLE_PDFS),
        "requested_sample": REQUESTED_SAMPLE,
        "executed_sample": [sample.arxiv_id for sample in SAMPLE_PDFS],
        "unavailable_requested_sample": UNAVAILABLE_REQUESTED_SAMPLE,
        "successful": len(extracted),
        "failed": len(results) - len(extracted),
        "total_elapsed_sec": total_elapsed_sec,
        "avg_elapsed_sec": round(sum(packet["elapsed_sec"] for packet in results) / len(results), 3),
        "avg_body_word_count": round(sum(packet["body_word_count"] for packet in extracted) / len(extracted), 1)
        if extracted
        else 0.0,
        "avg_markdown_length": round(sum(packet["markdown_length"] for packet in extracted) / len(extracted), 1)
        if extracted
        else 0.0,
        "safety_defaults": SAFETY_DEFAULTS,
        "loopback_bind_host": LOOPBACK_BIND_HOST,
        "page_range": PILOT_PAGE_RANGE,
        "pilot_max_pages": PILOT_MAX_PAGES,
        "pilot_scope": "first page per PDF for bounded stage-1 cost",
        "marker_version": package_version("marker-pdf"),
        "transformers_version": package_version("transformers"),
        "per_pdf": results,
    }
    (OUTPUT_ROOT / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {OUTPUT_ROOT / 'summary.json'}", flush=True)
    return summary


def main() -> None:
    run_marker_pilot()


if __name__ == "__main__":
    main()
