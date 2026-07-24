"""M245 S02: operator script for hybrid selection expand plan."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_hybrid_selection_expand.py"


def _pdf(path: Path, n: int = 120) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    # minimal PDF-ish bytes (hash only; not parsed)
    path.write_bytes(b"%PDF-1.4\n" + b"x" * n + b"\n%%EOF\n")


def test_script_temp_fixture(tmp_path: Path) -> None:
    catalog = tmp_path / "article_catalog"
    for pid, cat in (("keep1", "cs-cl"), ("keep2", "cs-ai"), ("skip_sel", "cs-cv")):
        _pdf(catalog / "arxiv" / cat / pid / "source" / f"{pid}.pdf")

    selection = tmp_path / "selection.json"
    selection.write_text(
        json.dumps(
            {
                "schema_version": "hybrid-gate-selection.v1",
                "papers": [{"paper_id": "skip_sel", "category": "cs-cv"}],
            }
        ),
        encoding="utf-8",
    )
    body_root = tmp_path / "bodies"
    # no bodies

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--catalog-root",
            str(catalog),
            "--selection",
            str(selection),
            "--body-root",
            str(body_root),
            "--repo-root",
            str(tmp_path),
            "--target-count",
            "10",
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
    assert report["proposed_count"] == 2
    ids = {p["paper_id"] for p in report["proposed_papers"]}
    assert ids == {"keep1", "keep2"}
    assert "skip_sel" not in ids
    assert report["selection_proposal"]["import_eligible"] is False


def test_script_summary_line(tmp_path: Path) -> None:
    catalog = tmp_path / "article_catalog"
    _pdf(catalog / "arxiv" / "cs-cl" / "p1" / "source" / "p1.pdf")
    selection = tmp_path / "selection.json"
    selection.write_text(json.dumps({"papers": []}), encoding="utf-8")
    body_root = tmp_path / "bodies"

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--catalog-root",
            str(catalog),
            "--selection",
            str(selection),
            "--body-root",
            str(body_root),
            "--repo-root",
            str(tmp_path),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "hybrid-selection-expand" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "batch: false" in proc.stdout


def test_script_write_selection(tmp_path: Path) -> None:
    catalog = tmp_path / "article_catalog"
    _pdf(catalog / "arxiv" / "cs-cl" / "p9" / "source" / "p9.pdf")
    selection = tmp_path / "selection.json"
    selection.write_text(json.dumps({"papers": []}), encoding="utf-8")
    out = tmp_path / "proposal.json"
    body_root = tmp_path / "bodies"

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--catalog-root",
            str(catalog),
            "--selection",
            str(selection),
            "--body-root",
            str(body_root),
            "--repo-root",
            str(tmp_path),
            "--write",
            str(out),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    sel = json.loads(out.read_text(encoding="utf-8"))
    assert sel["import_eligible"] is False
    assert sel["count"] == 1
    assert sel["papers"][0]["paper_id"] == "p9"


def test_live_smoke_if_catalog_present() -> None:
    catalog = ROOT / "data" / "article_catalog" / "article_catalog"
    if not (catalog / "arxiv").is_dir():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--target-count", "20"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "import_eligible: false" in proc.stdout
    assert "hybrid-selection-expand" in proc.stdout
    assert "batch: false" in proc.stdout
