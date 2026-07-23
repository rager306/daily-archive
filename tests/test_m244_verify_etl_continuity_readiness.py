"""M244 S02: operator script for continuity readiness report."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_etl_continuity_readiness.py"

_BODY = """# Graph Neural Networks

## Abstract
Graph neural networks process graph-structured data using message passing.

## Method
We evaluate citation graphs and molecular graphs for prediction tasks.

## Results
Enough scholarly prose for quality scoring and language detection.
"""


def _index(path: Path, articles: list[dict]) -> None:
    path.write_text(
        json.dumps({"schema_version": "article-catalog-index.v1", "articles": articles}),
        encoding="utf-8",
    )


def test_script_temp_fixture(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    arts = [
        {
            "article_key": f"p{i}",
            "article_ref": f"arxiv/cs-cl/p{i}",
            "source_code": "arxiv",
        }
        for i in range(1, 12)
    ]
    _index(idx, arts)
    body_root = tmp_path / "bodies"
    for i in range(1, 11):
        p = body_root / f"p{i}" / "body" / f"p{i}.hybrid.body.md"
        p.parent.mkdir(parents=True)
        p.write_text(_BODY, encoding="utf-8")

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--catalog-index",
            str(idx),
            "--catalog-root",
            str(tmp_path),
            "--body-root",
            str(body_root),
            "--repo-root",
            str(tmp_path),
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
    assert report["readiness_signal"] in {
        "blocked",
        "repair",
        "ready_for_review",
    }
    assert report["coverage"]["hybrid_body_found"] == 10
    assert report["preprocess"]["body_count"] == 10


def test_script_summary_line(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    _index(
        idx,
        [
            {
                "article_key": "p1",
                "article_ref": "arxiv/cs-cl/p1",
                "source_code": "arxiv",
            }
        ],
    )
    body_root = tmp_path / "bodies"
    p = body_root / "p1" / "body" / "p1.hybrid.body.md"
    p.parent.mkdir(parents=True)
    p.write_text(_BODY, encoding="utf-8")

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--catalog-index",
            str(idx),
            "--catalog-root",
            str(tmp_path),
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
    assert "etl-continuity-readiness" in proc.stdout
    assert "signal:" in proc.stdout
    assert "import_eligible: false" in proc.stdout


def test_live_smoke_if_catalog_present() -> None:
    index = ROOT / "data" / "article_catalog" / "index.json"
    if not index.is_file():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT)],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "import_eligible: false" in proc.stdout
    assert "etl-continuity-readiness" in proc.stdout
    assert "signal:" in proc.stdout
