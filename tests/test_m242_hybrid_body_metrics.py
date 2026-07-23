"""M242 S01: unique hybrid paper_ids vs raw artifact file counts."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.etl_body_coverage_audit import (
    audit_catalog_body_coverage,
    scan_hybrid_body_artifacts,
)


def _index(path: Path, articles: list[dict]) -> None:
    path.write_text(
        json.dumps({"schema_version": "article-catalog-index.v1", "articles": articles}),
        encoding="utf-8",
    )


def _body(root: Path, paper_id: str) -> None:
    p = root / paper_id / "body" / f"{paper_id}.hybrid.body.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("# body\n", encoding="utf-8")


def test_scan_counts_files_and_unique_ids(tmp_path: Path) -> None:
    r1 = tmp_path / "r1"
    r2 = tmp_path / "r2"
    _body(r1, "p1")
    _body(r2, "p1")  # same paper in two roots
    _body(r1, "p2")
    files, unique, by_root = scan_hybrid_body_artifacts((r1, r2))
    assert files == 3
    assert unique == 2
    assert by_root[str(r1)] == 2
    assert by_root[str(r2)] == 1


def test_audit_separates_catalog_join_from_artifact_volume(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    _index(
        idx,
        [
            {
                "article_key": "p1",
                "article_ref": "arxiv/cs-cl/p1",
                "source_code": "arxiv",
            },
            {
                "article_key": "p2",
                "article_ref": "arxiv/cs-cl/p2",
                "source_code": "arxiv",
            },
            {
                "article_key": "p3",
                "article_ref": "arxiv/cs-cl/p3",
                "source_code": "arxiv",
            },
        ],
    )
    r1 = tmp_path / "runs-a"
    r2 = tmp_path / "runs-b"
    _body(r1, "p1")
    _body(r2, "p1")  # duplicate artifact
    _body(r1, "p2")
    # orphan artifact not in catalog
    _body(r1, "orphan")

    pkg = audit_catalog_body_coverage(
        catalog_index_path=idx,
        body_roots=(r1, r2),
        catalog_root=tmp_path,
    )
    assert pkg.hybrid_body_found == 2  # p1, p2 unique catalog join
    assert pkg.hybrid_body_missing == 1  # p3
    assert pkg.hybrid_body_artifact_files == 4  # p1x2 + p2 + orphan
    assert pkg.hybrid_body_unique_paper_ids == 3  # p1, p2, orphan
    assert pkg.import_eligible is False
    d = pkg.to_dict()
    assert d["hybrid_body_artifact_files"] == 4
    assert d["hybrid_body_unique_paper_ids"] == 3
    assert d["hybrid_body_found"] == 2
    assert sum(d["hybrid_body_files_by_root"].values()) == 4


def test_m241_empty_index_still_works(tmp_path: Path) -> None:
    idx = tmp_path / "index.json"
    _index(idx, [])
    pkg = audit_catalog_body_coverage(catalog_index_path=idx, body_roots=())
    assert pkg.hybrid_body_artifact_files == 0
    assert pkg.hybrid_body_unique_paper_ids == 0
