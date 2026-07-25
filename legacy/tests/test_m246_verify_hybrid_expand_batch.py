"""M246 S02: operator hybrid expand preflight + optional limited batch."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_hybrid_expand_batch.py"


def _pdf(path: Path, n: int = 200) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"%PDF-1.4\n" + b"y" * n + b"\n%%EOF\n")


def test_script_writes_proposal_and_preflight(tmp_path: Path) -> None:
    catalog = tmp_path / "article_catalog"
    for pid, cat in (("n1", "cs-cl"), ("n2", "cs-ai")):
        _pdf(catalog / "arxiv" / cat / pid / "source" / f"{pid}.pdf")

    selection = tmp_path / "selection-20.json"
    selection.write_text(json.dumps({"papers": []}), encoding="utf-8")
    body_root = tmp_path / "bodies"
    proposal = tmp_path / "selection-40-proposal.json"

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
            str(proposal),
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
    assert report["preflight"]["preflight_signal"] == "ready_to_batch"
    assert report["preflight"]["ready_count"] == 2
    assert report["batch"] is None
    assert proposal.is_file()
    sel = json.loads(proposal.read_text(encoding="utf-8"))
    assert sel["import_eligible"] is False
    assert sel["count"] == 2


def test_script_summary_line(tmp_path: Path) -> None:
    catalog = tmp_path / "article_catalog"
    _pdf(catalog / "arxiv" / "cs-cl" / "p1" / "source" / "p1.pdf")
    selection = tmp_path / "selection.json"
    selection.write_text(json.dumps({"papers": []}), encoding="utf-8")
    body_root = tmp_path / "bodies"
    proposal = tmp_path / "proposal.json"

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
            str(proposal),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "hybrid-expand-batch" in proc.stdout
    assert "preflight: ready_to_batch" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "limit: 0" in proc.stdout


def test_limit_without_live_flag_skips_batch(tmp_path: Path) -> None:
    catalog = tmp_path / "article_catalog"
    _pdf(catalog / "arxiv" / "cs-cl" / "p1" / "source" / "p1.pdf")
    selection = tmp_path / "selection.json"
    selection.write_text(json.dumps({"papers": []}), encoding="utf-8")
    body_root = tmp_path / "bodies"
    proposal = tmp_path / "proposal.json"

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
            str(proposal),
            "--limit",
            "1",
            "--json",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    assert report["batch"]["skipped"] is True
    assert report["import_eligible"] is False


def test_live_preflight_smoke_if_catalog_present() -> None:
    catalog = ROOT / "data" / "article_catalog" / "article_catalog"
    if not (catalog / "arxiv").is_dir():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--limit", "0"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "hybrid-expand-batch" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "preflight:" in proc.stdout
    proposal = ROOT / "artifacts" / "m213-hybrid-gate" / "selection-40-proposal.json"
    assert proposal.is_file()



def test_refresh_continuity_pack_flag_in_help() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--help"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0
    assert "--refresh-continuity-pack" in proc.stdout



def test_refresh_continuity_pack_default_on() -> None:
    """M266: continuity pack refresh is default; opt-out via --no-refresh."""
    script = Path(__file__).resolve().parents[1] / "scripts" / "verify_hybrid_expand_batch.py"
    import subprocess, sys
    proc = subprocess.run(
        [sys.executable, str(script), "--help"],
        cwd=Path(__file__).resolve().parents[1],
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0
    assert "--refresh-continuity-pack" in proc.stdout
    assert "--no-refresh-continuity-pack" in proc.stdout
