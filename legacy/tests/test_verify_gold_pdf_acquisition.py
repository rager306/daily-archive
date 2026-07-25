"""Tests for gold PDF acquisition operator (M258 S02)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_gold_pdf_acquisition.py"


def test_gold_acquisition_no_network_reports(tmp_path: Path) -> None:
    catalog_root = tmp_path / "data" / "article_catalog"
    art_dir = catalog_root / "article_catalog" / "arxiv" / "cs-cl" / "2507.19457"
    source = art_dir / "source"
    source.mkdir(parents=True)
    # original.pdf only — no network, should promote
    (source / "original.pdf").write_bytes(b"%PDF-1.4\n" + b"x" * 200 + b"\n%%EOF\n")
    (art_dir / "article.json").write_text("{}", encoding="utf-8")
    index = {
        "articles": [
            {
                "article_key": "2507.19457",
                "article_path": "article_catalog/arxiv/cs-cl/2507.19457/article.json",
                "canonical_url": "https://arxiv.org/abs/2507.19457",
            }
        ]
    }
    index_path = catalog_root / "index.json"
    index_path.parent.mkdir(parents=True, exist_ok=True)
    index_path.write_text(json.dumps(index), encoding="utf-8")
    out = tmp_path / "acq.json"

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--repo-root",
            str(tmp_path),
            "--catalog-root",
            str(catalog_root),
            "--catalog-index",
            str(index_path),
            "--gold-id",
            "2507.19457",
            "--no-network",
            "--output",
            str(out),
            "--json",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["import_eligible"] is False
    assert report["gold_labels_invented"] is False
    assert report["acquired_count"] == 1
    assert report["records"][0]["status"] == "promoted_original_pdf"
    assert (source / "2507.19457.pdf").is_file()


def test_help_mentions_no_invent() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--help"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0
    assert "Never invent" in proc.stdout or "never invent" in proc.stdout.lower()
