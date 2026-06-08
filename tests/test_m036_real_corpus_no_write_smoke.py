from __future__ import annotations

import json
from pathlib import Path

import pytest

from scripts.run_m036_real_corpus_no_write_smoke import run_smoke


def _manifest(tmp_path: Path) -> Path:
    article_dir = tmp_path / "article_catalog" / "arxiv" / "cs-ai" / "demo"
    source_dir = article_dir / "source"
    loader_dir = article_dir / "loader"
    source_dir.mkdir(parents=True)
    loader_dir.mkdir()
    article_json = article_dir / "article.json"
    source_html = source_dir / "article.html"
    loader_summary = loader_dir / "summary.json"
    article_json.write_text('{"article_key":"demo"}\n', encoding="utf-8")
    source_html.write_text("<html>metadata fixture</html>\n", encoding="utf-8")
    loader_summary.write_text('{"status":"ok"}\n', encoding="utf-8")
    manifest = {
        "schema_version": "m036-real-corpus-smoke-manifest.v1",
        "article_count": 1,
        "catalog_ref": "artifact:data/article_catalog/catalog.json",
        "safety_flags": {
            "graph_write_allowed": False,
            "promotion_allowed": False,
            "production_import_attempted": False,
            "import_eligible": False,
        },
        "diagnostics": [],
        "articles": [
            {
                "article_key": "demo",
                "candidate_id": "real-article:demo",
                "candidate_type": "real_article_metadata",
                "article_ref": "artifact:test/article.json",
                "evidence_refs": [
                    "artifact:test/article.json",
                    "artifact:test/source/article.html",
                    "artifact:test/loader/summary.json",
                ],
                "diagnostics": ["metadata_only_smoke"],
                "source_file_count": 1,
                "loader_ref_count": 1,
                "safety_flags": {
                    "graph_write_allowed": False,
                    "promotion_allowed": False,
                    "production_import_attempted": False,
                    "import_eligible": False,
                },
            }
        ],
    }
    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    return path


def test_run_smoke_writes_per_article_handoffs_and_summary(tmp_path: Path) -> None:
    manifest = _manifest(tmp_path)
    output_dir = tmp_path / "run"

    summary = run_smoke(manifest, output_dir=output_dir, clean=True)

    assert summary["article_count"] == 1
    assert summary["completed_handoff_count"] == 1
    assert summary["graph_write_allowed"] is False
    assert summary["promotion_allowed"] is False
    assert summary["production_import_attempted"] is False
    assert summary["import_eligible"] is False
    article_summary = summary["articles"][0]
    assert article_summary["candidate_id"] == "real-article:demo"
    assert article_summary["queue_status"] == "ready"
    article_dir = output_dir / "articles" / "real-article_demo"
    for name in ["candidate.json", "review_packet.json", "review_trace.json", "queue_inspect.json", "readiness_handoff.json"]:
        assert (article_dir / name).exists(), name
    handoff = json.loads((article_dir / "readiness_handoff.json").read_text(encoding="utf-8"))
    assert handoff["model"] == "MiniMax-M3-512k"
    assert handoff["dry_run_only"] is True
    assert handoff["graph_write_allowed"] is False
    assert handoff["promotion_allowed"] is False
    assert handoff["production_import_attempted"] is False


def test_run_smoke_clean_refuses_output_outside_manifest_directory(tmp_path: Path) -> None:
    manifest = _manifest(tmp_path)

    with pytest.raises(ValueError, match="output_dir must be inside"):
        run_smoke(manifest, output_dir=tmp_path.parent / "unsafe-clean-target", clean=True)


def test_run_smoke_rejects_manifest_that_claims_import_authority(tmp_path: Path) -> None:
    manifest = _manifest(tmp_path)
    data = json.loads(manifest.read_text(encoding="utf-8"))
    data["safety_flags"]["import_eligible"] = True
    manifest.write_text(json.dumps(data), encoding="utf-8")

    with pytest.raises(ValueError, match="manifest safety flag"):
        run_smoke(manifest, output_dir=tmp_path / "run", clean=True)
