"""M241 S02: composition + operator script for ETL body coverage."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from research_graph.workflows.composition.etl_body_coverage import (
    EtlBodyCoverageRequest,
    run_etl_body_coverage_audit,
)

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_etl_body_coverage.py"


def _write_index(path: Path, articles: list[dict]) -> None:
    path.write_text(
        json.dumps({"schema_version": "article-catalog-index.v1", "articles": articles}),
        encoding="utf-8",
    )


def test_composition_audit_temp_tree(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    _write_index(
        idx,
        [
            {
                "article_key": "p1",
                "article_ref": "arxiv/cs-cl/p1",
                "source_code": "arxiv",
                "article_path": "missing.json",
            }
        ],
    )
    body_root = tmp_path / "bodies"
    p = body_root / "p1" / "body" / "p1.hybrid.body.md"
    p.parent.mkdir(parents=True)
    p.write_text("x\n", encoding="utf-8")

    result = run_etl_body_coverage_audit(
        EtlBodyCoverageRequest(
            catalog_index_path=idx,
            catalog_root=tmp_path,
            body_roots=(body_root,),
            repo_root=tmp_path,
        )
    )
    assert result.import_eligible is False
    assert result.package.hybrid_body_found == 1
    assert result.package.article_count == 1
    payload = result.to_dict()
    assert payload["import_eligible"] is False


def test_script_temp_fixture_exit_zero(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    _write_index(
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
    assert report["package"]["article_count"] == 1
    assert report["package"]["hybrid_body_found"] == 0


def test_script_live_catalog_smoke_import_false() -> None:
    """Optional live smoke: real catalog if present; never import-eligible."""
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
    assert "etl-body-coverage" in proc.stdout
