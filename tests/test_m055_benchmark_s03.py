"""Tests for M055 parser benchmark S03 OpenDataLoader-only baseline."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import benchmark_m055_opendataloader_only as opendl_only  # noqa: E402

SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}


def markdown_payload(*, size: int = 1400, image: bool = True, table: bool = False) -> str:
    body = "# Introduction\n\n" + ("x" * size)
    if image:
        body += "\n\n![figure](figure.png)\n"
    if table:
        body += "\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n"
    return body


def layout_payload(*, pages: int = 2, boxes: int = 2) -> dict[str, object]:
    return {
        "number of pages": pages,
        "kids": [
            {"type": "paragraph", "page number": 1, "bounding box": [0, 0, 10, 10]}
            for _ in range(boxes)
        ],
    }


def write_probe_outputs(output_folder: str, *, md: str | None = None) -> None:
    out = Path(output_folder)
    out.mkdir(parents=True, exist_ok=True)
    (out / "paper.md").write_text(md or markdown_payload(), encoding="utf-8")
    (out / "paper.json").write_text(json.dumps(layout_payload()), encoding="utf-8")


def make_manifest(tmp_path: Path, entries: int = 1) -> Path:
    pdfs = []
    for index in range(entries):
        arxiv_id = f"2401.0000{index}"
        pdf_path = tmp_path / f"{arxiv_id}.pdf"
        pdf_path.write_bytes(b"%PDF-1.4\nfixture\n")
        pdfs.append(
            {
                "article_key": arxiv_id,
                "arxiv_id": arxiv_id,
                "category": "cs-cl",
                "path": str(pdf_path),
                "sha256": opendl_only._sha256(pdf_path),
                "size_bytes": pdf_path.stat().st_size,
                "target_index": index,
            }
        )
    manifest_path = tmp_path / "corpus-manifest.json"
    manifest_path.write_text(json.dumps({"pdfs": pdfs}), encoding="utf-8")
    return manifest_path


def test_probe_opendataloader_pdf_success_on_import_run(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    pdf_path = tmp_path / "paper.pdf"
    pdf_path.write_bytes(b"%PDF-1.4\n")

    fake_module = SimpleNamespace(
        run=lambda input_path, output_folder, generate_markdown: write_probe_outputs(output_folder)
    )
    monkeypatch.setattr(opendl_only.importlib, "import_module", lambda name: fake_module)

    result = opendl_only._probe_opendataloader_pdf(
        pdf_path, tmp_path / "out", threads=4, format="md"
    )

    assert result["error"] is None
    assert result["runner"] == "import:run"
    assert result["markdown_text"].startswith("# Introduction")
    assert result["json_layout"]["number of pages"] == 2
    assert result["bytes"] > 1024


def test_probe_opendataloader_pdf_success_on_subprocess_fallback(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    pdf_path = tmp_path / "paper.pdf"
    pdf_path.write_bytes(b"%PDF-1.4\n")
    monkeypatch.setattr(
        opendl_only,
        "_run_via_import_api",
        lambda *args, **kwargs: (_ for _ in ()).throw(RuntimeError("boom")),
    )

    def fake_run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        output_dir = Path(command[command.index("-o") + 1])
        write_probe_outputs(str(output_dir))
        return subprocess.CompletedProcess(command, 0, stdout="", stderr="")

    monkeypatch.setattr(opendl_only.subprocess, "run", fake_run)

    result = opendl_only._probe_opendataloader_pdf(
        pdf_path, tmp_path / "out", threads=4, format="md"
    )

    assert result["error"] is None
    assert result["runner"] == "subprocess:python -m opendataloader_pdf"
    assert result["normalized_format"] == "markdown"
    assert result["layout_path"] == "layout/paper.json"


def test_probe_opendataloader_pdf_fail_closed_on_import_error(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    pdf_path = tmp_path / "paper.pdf"
    pdf_path.write_bytes(b"%PDF-1.4\n")

    def missing_module(name: str) -> object:
        raise ModuleNotFoundError(name)

    monkeypatch.setattr(opendl_only.importlib, "import_module", missing_module)

    result = opendl_only._probe_opendataloader_pdf(
        pdf_path, tmp_path / "out", threads=4, format="md"
    )

    assert result["error"] is not None
    assert result["markdown_text"] == ""
    assert result["json_layout"] is None
    assert result["bytes"] == 0


def test_extract_markdown_metrics_small_markdown_low_quality_source() -> None:
    metrics = opendl_only._extract_markdown_metrics(
        "# Tiny\n\n![figure](figure.png)", layout_payload()
    )

    assert metrics["markdown_size_bytes"] < 1024
    assert metrics["low_quality_source"] is True


def test_extract_markdown_metrics_zero_tables_and_zero_images_low_quality_source() -> None:
    metrics = opendl_only._extract_markdown_metrics("# Body\n\n" + ("x" * 1400), layout_payload())

    assert metrics["table_count"] == 0
    assert metrics["image_count"] == 0
    assert metrics["low_quality_source"] is True


def test_low_quality_source_criteria_success_combination() -> None:
    assert (
        opendl_only._low_quality_source_criteria(
            {
                "markdown_size_bytes": 1400,
                "table_count": 0,
                "image_count": 1,
                "section_count": 1,
            }
        )
        is False
    )


def test_low_quality_source_criteria_low_quality_combinations() -> None:
    assert opendl_only._low_quality_source_criteria(
        {"markdown_size_bytes": 10, "table_count": 1, "image_count": 0, "section_count": 1}
    )
    assert opendl_only._low_quality_source_criteria(
        {"markdown_size_bytes": 1400, "table_count": 0, "image_count": 0, "section_count": 1}
    )
    assert opendl_only._low_quality_source_criteria(
        {"markdown_size_bytes": 1400, "table_count": 1, "image_count": 0, "section_count": 0}
    )


def test_probe_opendataloader_only_aggregate_counts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest = make_manifest(tmp_path, entries=3)
    payloads = [
        {"markdown_text": markdown_payload(), "json_layout": layout_payload(), "error": None},
        {"markdown_text": "# Tiny\n![x](x.png)", "json_layout": layout_payload(), "error": None},
        {"markdown_text": "", "json_layout": None, "error": "missing"},
    ]

    def fake_probe(*args: object, **kwargs: object) -> dict[str, object]:
        payload = payloads.pop(0)
        return {
            **payload,
            "format": "md",
            "normalized_format": "markdown",
            "bytes": len(str(payload["markdown_text"]).encode("utf-8")),
            "duration_ms": 1,
            "runner": "test",
            "markdown_path": "markdown/paper.md",
            "layout_path": "layout/paper.json" if payload["json_layout"] else None,
        }

    monkeypatch.setattr(opendl_only, "_probe_opendataloader_pdf", fake_probe)

    summary = opendl_only.probe_opendataloader_only(manifest, tmp_path / "out", max_retries=1)

    assert summary["aggregate_counts"] == {
        "success": 1,
        "low_quality_source": 1,
        "opendataloader_unavailable": 1,
    }
    assert summary["total_pdfs"] == 3
    assert len(list((tmp_path / "out" / "per-pdf").glob("*.json"))) == 3


def test_atomic_write_pattern(tmp_path: Path) -> None:
    target = tmp_path / "out" / "summary.json"
    opendl_only._atomic_write_json(target, {"value": "old"})
    opendl_only._atomic_write_json(target, {"value": "new"})

    assert json.loads(target.read_text(encoding="utf-8")) == {"value": "new"}
    assert not list(target.parent.glob("*.tmp"))


def test_5_safety_defaults_all_false(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    manifest = make_manifest(tmp_path)
    monkeypatch.setattr(
        opendl_only,
        "_probe_opendataloader_pdf",
        lambda *args, **kwargs: {
            "markdown_text": markdown_payload(),
            "json_layout": layout_payload(),
            "format": "md",
            "normalized_format": "markdown",
            "bytes": 1400,
            "duration_ms": 1,
            "error": None,
            "runner": "test",
            "markdown_path": "markdown/paper.md",
            "layout_path": "layout/paper.json",
        },
    )

    summary = opendl_only.probe_opendataloader_only(manifest, tmp_path / "out")
    packet = json.loads(
        next((tmp_path / "out" / "per-pdf").glob("*.json")).read_text(encoding="utf-8")
    )

    assert set(summary["safety_defaults"]) == SAFETY_KEYS
    assert set(packet["safety_defaults"]) == SAFETY_KEYS
    assert all(value is False for value in summary["safety_defaults"].values())
    assert all(value is False for value in packet["safety_defaults"].values())


def test_cli_dry_run_does_not_call_opendataloader(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    manifest = make_manifest(tmp_path)
    blocked = mock.Mock(side_effect=AssertionError("OpenDataLoader should not run in dry-run"))
    monkeypatch.setattr(opendl_only, "_probe_opendataloader_pdf", blocked)

    exit_code = opendl_only.main(
        [
            "--corpus-manifest",
            str(manifest),
            "--output-dir",
            str(tmp_path / "out"),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    assert blocked.call_count == 0
    assert json.loads((tmp_path / "out" / "summary.json").read_text(encoding="utf-8"))["dry_run"]
    assert "dry_run" in capsys.readouterr().out


def test_idempotent_summary_overwrite(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    manifest = make_manifest(tmp_path)
    monkeypatch.setattr(
        opendl_only,
        "_probe_opendataloader_pdf",
        lambda *args, **kwargs: {
            "markdown_text": markdown_payload(),
            "json_layout": layout_payload(),
            "format": "md",
            "normalized_format": "markdown",
            "bytes": 1400,
            "duration_ms": 1,
            "error": None,
            "runner": "test",
            "markdown_path": "markdown/paper.md",
            "layout_path": "layout/paper.json",
        },
    )

    first = opendl_only.probe_opendataloader_only(manifest, tmp_path / "out")
    second = opendl_only.probe_opendataloader_only(manifest, tmp_path / "out")

    assert first == second
    assert len(list((tmp_path / "out" / "per-pdf").glob("*.json"))) == 1
