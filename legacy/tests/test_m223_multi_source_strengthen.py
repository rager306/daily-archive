"""M223 multi-source strengthen: company_blog proof + GNN catalog register."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.non_arxiv_source_records import (
    build_gnn_textbook_article_record,
    build_multi_source_selection,
    gnn_chapter_article_key,
)
from research_graph.workflows.composition.gnn_textbook_catalog_register import (
    GnnTextbookCatalogRegisterRequest,
    register_gnn_textbook_chapters,
)
from research_graph.workflows.composition.non_arxiv_html_source_proof import (
    NonArxivHtmlSourceProofRequest,
    run_non_arxiv_html_source_proof,
)

ROOT = Path(__file__).resolve().parents[1]
BLOG_ARTICLE = (
    ROOT
    / "data/article_catalog/article_catalog/company_blog/cs-ir/"
    / "pageindex_zhang2025pageindex/article.json"
)
GNN_SOURCE = ROOT / "artifacts/m222-gnn-textbook/source"
FIXTURE_CHAPTER = ROOT / "tests/fixtures/gnn_textbook/chapter_01_intro.html"


def test_gnn_article_record_shape() -> None:
    rec = build_gnn_textbook_article_record(
        chapter_slug="chapters/01-intro-to-graphs/",
        title="Chapter 01",
        html_sha256="abc",
        html_byte_size=10,
    )
    assert rec["source_code"] == "gnn_textbook"
    assert rec["domain_profile"] == "textbook"
    assert rec["article_key"] == gnn_chapter_article_key("chapters/01-intro-to-graphs/")
    assert rec["catalog_path"].startswith("gnn_textbook/html/")
    assert rec["safety_flags"]["trusted_kg_import_allowed"] is False
    assert rec["source_variants"][0]["source_format"] == "html"


def test_multi_source_selection_fail_closed() -> None:
    sel = build_multi_source_selection(
        [
            {
                "article_ref": "company_blog/cs-ir/x",
                "source_code": "company_blog",
                "title": "X",
            }
        ]
    )
    assert sel["import_eligible"] is False
    assert sel["article_count"] == 1


def test_company_blog_html_proof() -> None:
    if not BLOG_ARTICLE.is_file():
        return
    result = run_non_arxiv_html_source_proof(
        NonArxivHtmlSourceProofRequest(
            article_json_path=BLOG_ARTICLE,
            repo_root=ROOT,
            min_body_chars=500,
            min_chunks=1,
        )
    )
    assert result.import_eligible is False
    assert result.hybrid_claimed_success is False
    assert result.source_code == "company_blog"
    assert result.load_outcome == "loaded"
    assert result.proof_pass is True
    assert result.body_chars >= 500
    assert result.structure is not None
    assert result.structure.source_kind == "html"
    assert result.structure.chunk_count >= 1


def test_register_gnn_into_tmp_catalog(tmp_path: Path) -> None:
    # Prefer live m222 sources; fall back to fixture
    src_dir = tmp_path / "source"
    src_dir.mkdir()
    if GNN_SOURCE.is_dir() and any(GNN_SOURCE.glob("*.html")):
        for p in GNN_SOURCE.glob("*.html"):
            (src_dir / p.name).write_bytes(p.read_bytes())
    else:
        assert FIXTURE_CHAPTER.is_file()
        (src_dir / "chapters__01-intro-to-graphs.html").write_bytes(
            FIXTURE_CHAPTER.read_bytes()
        )

    catalog_root = tmp_path / "catalog"
    # minimal catalog layout for rebuild
    (catalog_root / "article_catalog").mkdir(parents=True)
    catalog_manifest = {
        "schema_version": "article-catalog.v00.01",
        "catalog_id": "test",
    }
    (catalog_root / "catalog.json").write_text(
        json.dumps(catalog_manifest), encoding="utf-8"
    )
    existing_index = {
        "schema_version": "article-catalog-index.v00.01",
        "catalog_schema_version": "article-catalog.v00.01",
        "article_schema_version": "article.v00.01",
        "index_id": "test-index",
        "articles": [],
        "indexes": {},
        "safety_flags": {
            "metadata_manifests_embed_raw_text": False,
            "metadata_manifests_embed_raw_binary": False,
            "graph_import_allowed": False,
            "production_ladybugdb_write_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }
    (catalog_root / "index.json").write_text(
        json.dumps(existing_index), encoding="utf-8"
    )

    chapter_map = {
        p.name: (
            (
                "chapters/01-intro-to-graphs/"
                if "01-intro" in p.name
                else p.stem.replace("__", "/") + "/"
            ),
            f"Title {p.stem}",
        )
        for p in sorted(src_dir.glob("*.html"))
    }
    result = register_gnn_textbook_chapters(
        GnnTextbookCatalogRegisterRequest(
            catalog_root=catalog_root,
            source_dir=src_dir,
            chapter_map=chapter_map,
            rebuild_index=True,
            output_path=tmp_path / "register.json",
            selection_output_path=tmp_path / "selection.json",
            repo_root=tmp_path,
        )
    )
    assert result.import_eligible is False
    assert len(result.registered) >= 1
    assert result.index_updated is True
    assert result.index_article_count is not None
    assert result.index_article_count >= 1
    # article files exist
    for reg in result.registered:
        art = catalog_root / "article_catalog" / reg.article_ref / "article.json"
        assert art.is_file(), reg.article_ref
        html = catalog_root / "article_catalog" / reg.article_ref / "source" / "chapter.html"
        assert html.is_file()
    idx = json.loads((catalog_root / "index.json").read_text(encoding="utf-8"))
    codes = {a["source_code"] for a in idx["articles"]}
    assert "gnn_textbook" in codes
    sel = json.loads((tmp_path / "selection.json").read_text(encoding="utf-8"))
    assert sel["import_eligible"] is False
    assert sel["article_count"] >= 1
