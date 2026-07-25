"""Tests for hybrid join leak audit operator (M258 S03)."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_hybrid_join_leak_audit.py"


def test_join_leak_audit_tmp(tmp_path: Path) -> None:
    catalog_root = tmp_path / "data" / "article_catalog"
    catalog_root.mkdir(parents=True)
    index = {
        "articles": [
            {
                "article_key": "1111.11111",
                "article_path": "article_catalog/arxiv/cs-cl/1111.11111/article.json",
                "source_code": "arxiv",
            }
        ]
    }
    index_path = catalog_root / "index.json"
    index_path.write_text(json.dumps(index), encoding="utf-8")
    (catalog_root / "article_catalog" / "arxiv" / "cs-cl" / "1111.11111").mkdir(
        parents=True
    )
    (
        catalog_root
        / "article_catalog"
        / "arxiv"
        / "cs-cl"
        / "1111.11111"
        / "article.json"
    ).write_text("{}", encoding="utf-8")

    body_root = tmp_path / "bodies"
    body_root.mkdir()
    # Layout matches find_hybrid_body: <root>/<id>/body/<id>.hybrid.body.md
    joined = body_root / "1111.11111" / "body"
    joined.mkdir(parents=True)
    (joined / "1111.11111.hybrid.body.md").write_text("body", encoding="utf-8")
    # orphan body not in catalog
    orphan = body_root / "9999.99999" / "body"
    orphan.mkdir(parents=True)
    (orphan / "9999.99999.hybrid.body.md").write_text("body", encoding="utf-8")
    out = tmp_path / "leak.json"

    proc = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--repo-root",
            str(tmp_path),
            "--catalog-index",
            str(index_path),
            "--catalog-root",
            str(catalog_root),
            "--body-root",
            str(body_root),
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
    assert report["hybrid_found"] == 1
    assert report["hybrid_unique_body_ids"] == 2
    assert report["orphan_body_id_count"] == 1
    assert report["delta_unique_minus_found"] == 1
