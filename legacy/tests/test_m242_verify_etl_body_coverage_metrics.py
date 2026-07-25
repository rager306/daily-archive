"""M242 S02: operator summary shows unique hybrid ids vs artifact files."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_etl_body_coverage.py"


def _index(path: Path, articles: list[dict]) -> None:
    path.write_text(
        json.dumps({"schema_version": "article-catalog-index.v1", "articles": articles}),
        encoding="utf-8",
    )


def test_script_summary_includes_unique_and_files(tmp_path: Path) -> None:
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
    r1 = tmp_path / "r1"
    r2 = tmp_path / "r2"
    for root in (r1, r2):
        p = root / "p1" / "body" / "p1.hybrid.body.md"
        p.parent.mkdir(parents=True)
        p.write_text("x\n", encoding="utf-8")

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--catalog-index",
            str(idx),
            "--catalog-root",
            str(tmp_path),
            "--body-root",
            str(r1),
            "--body-root",
            str(r2),
            "--repo-root",
            str(tmp_path),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "hybrid_found: 1" in proc.stdout
    assert "hybrid_unique_ids: 1" in proc.stdout
    assert "hybrid_files: 2" in proc.stdout
    assert "import_eligible: false" in proc.stdout
    assert "files_by_root:" in proc.stdout


def test_live_script_json_has_m242_fields() -> None:
    index = ROOT / "data" / "article_catalog" / "index.json"
    if not index.is_file():
        return
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    report = json.loads(proc.stdout)
    pkg = report["package"]
    assert pkg["import_eligible"] is False
    assert "hybrid_body_artifact_files" in pkg
    assert "hybrid_body_unique_paper_ids" in pkg
    assert pkg["hybrid_body_artifact_files"] >= pkg["hybrid_body_unique_paper_ids"]
    assert pkg["hybrid_body_found"] <= pkg["hybrid_body_unique_paper_ids"] or pkg[
        "hybrid_body_unique_paper_ids"
    ] == 0
