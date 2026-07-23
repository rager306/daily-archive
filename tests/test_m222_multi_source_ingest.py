"""M222 multi-source catalog inventory + GNN textbook HTML path."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.application.corpus.catalog_source_inventory import (
    build_catalog_source_inventory,
)
from research_graph.application.profiles.textbook import (
    DOMAIN_PROFILE,
    GNN_TEXTBOOK_BASE_URL,
    GNN_TEXTBOOK_SOURCE_CODE,
    textbook_profile_dict,
)
from research_graph.workflows.composition.catalog_source_inventory import (
    CatalogSourceInventoryRequest,
    run_catalog_source_inventory,
)
from research_graph.workflows.composition.gnn_textbook_ingest import (
    GnnTextbookIngestRequest,
    ingest_local_gnn_chapter,
    parse_sitemap_locs,
    run_gnn_textbook_ingest,
)

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests" / "fixtures" / "gnn_textbook" / "chapter_01_intro.html"
CATALOG = ROOT / "data" / "article_catalog" / "article_catalog"


def test_textbook_profile_constants() -> None:
    assert DOMAIN_PROFILE == "textbook"
    assert GNN_TEXTBOOK_SOURCE_CODE == "gnn_textbook"
    assert "graph-neural-networks-textbook" in GNN_TEXTBOOK_BASE_URL
    d = textbook_profile_dict()
    assert d["import_eligible"] is False
    assert d["domain_profile"] == "textbook"


def test_build_catalog_inventory_from_records() -> None:
    articles = [
        {
            "article_key": "a1",
            "source_code": "arxiv",
            "source_variants": [
                {
                    "source_format": "pdf",
                    "source_role": "arxiv_pdf",
                    "capture_status": "captured",
                    "is_content_bearing": True,
                    "is_metadata_only": False,
                    "loader_outcome": "loaded_metadata_only",
                    "path": "source/a1.pdf",
                    "url": "https://arxiv.org/pdf/a1",
                }
            ],
        },
        {
            "article_key": "n1",
            "source_code": "nature",
            "source_variants": [
                {
                    "source_format": "html_metadata",
                    "source_role": "nature_html",
                    "capture_status": "captured",
                    "is_content_bearing": False,
                    "is_metadata_only": True,
                    "loader_outcome": "not_loaded_metadata_only",
                    "path": "source/article.html",
                    "url": "https://www.nature.com/x",
                }
            ],
        },
        {
            "article_key": "s1",
            "source_code": "stanford",
            "source_variants": [
                {
                    "source_format": "pdf",
                    "source_role": "external_pdf",
                    "capture_status": "not_captured",
                    "is_content_bearing": True,
                    "is_metadata_only": False,
                    "loader_outcome": "not_loaded",
                    "path": None,
                    "url": "https://example.edu/x.pdf",
                }
            ],
        },
    ]
    pkg = build_catalog_source_inventory(articles)
    assert pkg.article_count == 3
    assert pkg.non_arxiv_articles == 2
    assert pkg.pdf_variants == 2
    assert pkg.html_variants == 1
    assert pkg.content_bearing_captured == 1
    assert pkg.content_bearing_missing == 1
    assert pkg.import_eligible is False
    assert "nature_pilot_present_check_metadata_only" in pkg.gaps
    assert "stanford_pilot_present_check_not_captured" in pkg.gaps


def test_composition_catalog_inventory_real_tree() -> None:
    if not CATALOG.is_dir():
        return
    result = run_catalog_source_inventory(
        CatalogSourceInventoryRequest(
            catalog_root=CATALOG,
            repo_root=ROOT,
            max_samples=5,
        )
    )
    assert result.import_eligible is False
    assert result.articles_loaded >= 200
    pkg = result.package
    assert pkg.by_source_code.get("arxiv", 0) >= 200
    assert pkg.non_arxiv_articles >= 1
    assert pkg.pdf_variants >= 1
    # hybrid operator path is PDF-heavy; inventory must still surface HTML pilots
    assert pkg.html_variants >= 1 or "no_html_variants" in pkg.gaps


def test_gnn_local_chapter_ingest() -> None:
    assert FIXTURE.is_file()
    chapter = ingest_local_gnn_chapter(FIXTURE, chapter_id="gnn-ch01")
    assert chapter.import_eligible is False
    assert chapter.domain_profile == "textbook"
    assert chapter.source_code == "gnn_textbook"
    assert chapter.source_kind == "html"
    assert chapter.load_outcome == "loaded"
    assert chapter.body_chars > 200
    assert chapter.structure is not None
    assert chapter.structure.source_kind == "html"
    assert chapter.structure.chunk_count >= 1


def test_run_gnn_textbook_ingest_offline(tmp_path: Path) -> None:
    out = tmp_path / "package.json"
    package = run_gnn_textbook_ingest(
        GnnTextbookIngestRequest(
            chapter_path=FIXTURE,
            chapter_id="gnn-ch01",
            work_dir=tmp_path / "work",
            allow_network=False,
            output_path=out,
            repo_root=tmp_path,
        )
    )
    assert package.import_eligible is False
    assert package.domain_profile == "textbook"
    assert len(package.chapters) == 1
    assert package.chapters[0].load_outcome == "loaded"
    assert package.fetched_paths == ()
    payload = json.loads(out.read_text(encoding="utf-8"))
    assert payload["import_eligible"] is False
    assert payload["chapters"][0]["source_kind"] == "html"


def test_parse_sitemap_locs() -> None:
    xml = """<?xml version="1.0" encoding="UTF-8"?>
    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
      <url><loc>https://example.com/a/</loc></url>
      <url><loc>https://example.com/b/</loc></url>
    </urlset>
    """
    locs = parse_sitemap_locs(xml)
    assert locs == (
        "https://example.com/a/",
        "https://example.com/b/",
    )


def test_package_rejects_import_true() -> None:
    from research_graph.application.corpus.catalog_source_inventory import (
        CatalogSourceInventoryPackage,
    )

    with pytest.raises(ValueError):
        CatalogSourceInventoryPackage(
            schema_version="x",
            article_count=0,
            variant_count=0,
            by_source_code={},
            by_source_format={},
            by_capture_status={},
            content_bearing_captured=0,
            content_bearing_missing=0,
            metadata_only_variants=0,
            pdf_variants=0,
            html_variants=0,
            markdown_variants=0,
            non_arxiv_articles=0,
            gaps=(),
            samples=(),
            diagnostics=(),
            import_eligible=True,
        )
