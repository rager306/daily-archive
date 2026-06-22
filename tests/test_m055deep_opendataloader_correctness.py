"""Tests for M055deep S02 OpenDataLoader correctness validation."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import benchmark_m055deep_opendataloader_correctness as correctness  # noqa: E402

SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}


def png_payload(width: int, height: int, *, payload_bytes: int = 0) -> bytes:
    return (
        b"\x89PNG\r\n\x1a\n"
        + b"\x00\x00\x00\rIHDR"
        + width.to_bytes(4, "big")
        + height.to_bytes(4, "big")
        + b"\x08\x02\x00\x00\x00"
        + (b"x" * payload_bytes)
    )


def write_manifest(tmp_path: Path, arxiv_ids: list[str]) -> Path:
    pdfs = []
    for index, arxiv_id in enumerate(arxiv_ids):
        pdf_path = tmp_path / "pdfs" / f"{arxiv_id}.pdf"
        pdf_path.parent.mkdir(parents=True, exist_ok=True)
        pdf_path.write_bytes(b"%PDF-1.4\nfixture\n")
        pdfs.append(
            {
                "article_key": arxiv_id,
                "arxiv_id": arxiv_id,
                "category": "cs-cl",
                "path": str(pdf_path),
                "target_index": index,
            }
        )
    manifest_path = tmp_path / "corpus-manifest.json"
    manifest_path.write_text(json.dumps({"pdfs": pdfs}), encoding="utf-8")
    return manifest_path


def write_opendataloader_packet(
    tmp_path: Path, arxiv_id: str, markdown: str, *, image_count: int = 0
) -> Path:
    opendl_dir = tmp_path / "opendataloader-only"
    markdown_dir = opendl_dir / "markdown"
    per_pdf_dir = opendl_dir / "per-pdf"
    markdown_dir.mkdir(parents=True, exist_ok=True)
    per_pdf_dir.mkdir(parents=True, exist_ok=True)
    markdown_path = markdown_dir / f"{arxiv_id}.md"
    markdown_path.write_text(markdown, encoding="utf-8")
    packet = {
        "schema_version": "m055-parser-benchmark.opendataloader-only.v1",
        "article_key": arxiv_id,
        "arxiv_id": arxiv_id,
        "category": "cs-cl",
        "image_count": image_count,
        "markdown_path": f"markdown/{arxiv_id}.md",
        "safety_defaults": correctness._safety_defaults(),
    }
    (per_pdf_dir / f"{arxiv_id}.json").write_text(json.dumps(packet), encoding="utf-8")
    return opendl_dir


def test_parse_markdown_tables_valid_2x3_table() -> None:
    markdown = "| A | B | C |\n| --- | --- | --- |\n| 1 | 2 | 3 |\n"

    tables = correctness._parse_markdown_tables(markdown)

    assert len(tables) == 1
    assert tables[0]["rows"] == 2
    assert tables[0]["cols"] == 3
    assert tables[0]["headers"] == ["A", "B", "C"]
    assert tables[0]["body_rows"] == [["1", "2", "3"]]
    assert tables[0]["has_alignment_separator"] is True


def test_parse_markdown_tables_malformed_table_without_separator() -> None:
    markdown = "| A | B |\n| 1 | 2 |\n"

    assert correctness._parse_markdown_tables(markdown) == []


def test_extract_figure_captions_matches_figure_colon() -> None:
    markdown = "Intro\nFigure 1: A chart showing parser quality.\n"

    captions = correctness._extract_figure_captions(markdown)

    assert captions == [
        {
            "figure_id": "Figure 1",
            "caption_type": "figure",
            "caption_text": "A chart showing parser quality.",
            "line_number": 2,
        }
    ]


def test_extract_figure_captions_without_caption_returns_empty() -> None:
    assert correctness._extract_figure_captions("No figure caption here.") == []


def test_detect_charts_detects_chart_like_png(tmp_path: Path) -> None:
    image_dir = tmp_path / "images"
    image_dir.mkdir()
    (image_dir / "chart.png").write_bytes(png_payload(800, 600, payload_bytes=6000))

    charts = correctness._detect_charts(image_dir)

    assert len(charts) == 1
    assert charts[0]["chart_type"] == "matplotlib_like_raster"
    assert charts[0]["dimensions"] == {"width": 800, "height": 600}


def test_detect_charts_ignores_small_non_chart_image(tmp_path: Path) -> None:
    image_dir = tmp_path / "images"
    image_dir.mkdir()
    (image_dir / "icon.png").write_bytes(png_payload(16, 16, payload_bytes=10))

    assert correctness._detect_charts(image_dir) == []


def test_probe_opendataloader_correctness_aggregates_metrics(tmp_path: Path) -> None:
    manifest_path = write_manifest(tmp_path, ["2401.00001", "2401.00002"])
    markdown_one = "\n".join(
        [
            "Figure 1: A chart-like plot.",
            "Table 1: Main results.",
            "![plot](<2401.00001_images/chart.png>)",
            "| A | B |",
            "| --- | --- |",
            "| 1 | 2 |",
        ]
    )
    opendl_dir = write_opendataloader_packet(tmp_path, "2401.00001", markdown_one, image_count=1)
    image_dir = opendl_dir / "markdown" / "2401.00001_images"
    image_dir.mkdir()
    (image_dir / "chart.png").write_bytes(png_payload(640, 480, payload_bytes=7000))
    write_opendataloader_packet(tmp_path, "2401.00002", "| X |\n| --- |\n| y |\n", image_count=0)

    summary = correctness.probe_opendataloader_correctness(
        manifest_path, opendl_dir, tmp_path / "out"
    )

    assert summary["success_count"] == 2
    assert summary["error_count"] == 0
    aggregate = summary["aggregate_correctness_metrics"]
    assert aggregate["tables_total"] == 2
    assert aggregate["tables_with_caption"] == 1
    assert aggregate["figures_total"] == 1
    assert aggregate["figures_with_caption"] == 1
    assert aggregate["charts_detected"] == 1
    assert aggregate["chart_types_distribution"] == {"matplotlib_like_raster": 1}


def test_probe_outputs_five_safety_defaults_false(tmp_path: Path) -> None:
    manifest_path = write_manifest(tmp_path, ["2401.00003"])
    opendl_dir = write_opendataloader_packet(tmp_path, "2401.00003", "| A |\n| --- |\n| 1 |\n")

    summary = correctness.probe_opendataloader_correctness(
        manifest_path, opendl_dir, tmp_path / "out"
    )
    packet = json.loads(
        (tmp_path / "out" / "per-pdf" / "2401.00003.json").read_text(encoding="utf-8")
    )

    assert set(summary["safety_defaults"]) == SAFETY_KEYS
    assert set(packet["safety_defaults"]) == SAFETY_KEYS
    assert all(value is False for value in summary["safety_defaults"].values())
    assert all(value is False for value in packet["safety_defaults"].values())


def test_probe_summary_is_idempotent(tmp_path: Path) -> None:
    manifest_path = write_manifest(tmp_path, ["2401.00004"])
    opendl_dir = write_opendataloader_packet(tmp_path, "2401.00004", "| A |\n| --- |\n| 1 |\n")
    output_dir = tmp_path / "out"

    correctness.probe_opendataloader_correctness(manifest_path, opendl_dir, output_dir)
    first_summary = (output_dir / "summary.json").read_text(encoding="utf-8")
    correctness.probe_opendataloader_correctness(manifest_path, opendl_dir, output_dir)
    second_summary = (output_dir / "summary.json").read_text(encoding="utf-8")

    assert first_summary == second_summary


def test_probe_fail_closed_emits_typed_diagnostic(tmp_path: Path) -> None:
    manifest_path = write_manifest(tmp_path, ["2401.00005"])
    opendl_dir = tmp_path / "opendataloader-only"
    (opendl_dir / "per-pdf").mkdir(parents=True)
    (opendl_dir / "per-pdf" / "2401.00005.json").write_text("{bad json", encoding="utf-8")

    summary = correctness.probe_opendataloader_correctness(
        manifest_path, opendl_dir, tmp_path / "out"
    )
    packet = json.loads(
        (tmp_path / "out" / "per-pdf" / "2401.00005.json").read_text(encoding="utf-8")
    )

    assert summary["success_count"] == 0
    assert summary["error_count"] == 1
    assert packet["status"] == "error"
    assert packet["diagnostic"]["type"] == "ValueError"
    assert all(value is False for value in packet["safety_defaults"].values())
