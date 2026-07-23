"""Pure helpers for non-arxiv / multi-source catalog records (M223).

Builds article.v00.01-shaped dicts for GNN textbook HTML chapters and
multi-source selection rows. No filesystem, no network, no import auth.
"""

from __future__ import annotations

import hashlib
from typing import Any

from research_graph.application.profiles.textbook import (
    DOMAIN_PROFILE,
    GNN_TEXTBOOK_AUTHOR,
    GNN_TEXTBOOK_BASE_URL,
    GNN_TEXTBOOK_SOURCE_CODE,
    GNN_TEXTBOOK_TITLE,
)

ARTICLE_SCHEMA_VERSION = "article.v00.01"
SELECTION_SCHEMA_VERSION = "m223-multi-source-selection.v1"


def _sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def gnn_chapter_article_key(chapter_slug: str) -> str:
    """Stable article_key from chapter slug (e.g. 01-intro-to-graphs)."""
    slug = chapter_slug.strip().strip("/").replace("/", "__")
    if not slug:
        raise ValueError("chapter_slug required")
    return f"gnn-ch-{slug}"


def build_gnn_textbook_article_record(
    *,
    chapter_slug: str,
    title: str,
    html_rel_path: str = "source/chapter.html",
    html_sha256: str | None = None,
    html_byte_size: int | None = None,
    canonical_url: str | None = None,
    topic_tags: list[str] | None = None,
) -> dict[str, Any]:
    """Build article.v00.01 record for one GNN textbook HTML chapter."""
    article_key = gnn_chapter_article_key(chapter_slug)
    coarse = "html"
    catalog_path = f"{GNN_TEXTBOOK_SOURCE_CODE}/{coarse}/{article_key}"
    url = canonical_url or (
        GNN_TEXTBOOK_BASE_URL.rstrip("/") + "/" + chapter_slug.strip().lstrip("/")
    )
    if not url.endswith("/") and "chapters/" in url:
        url = url + "/"
    variant_id = f"{article_key}:source:html-chapter"
    return {
        "schema_version": ARTICLE_SCHEMA_VERSION,
        "article_key": article_key,
        "catalog_path": catalog_path,
        "source_code": GNN_TEXTBOOK_SOURCE_CODE,
        "source_type": "textbook",
        "publisher": GNN_TEXTBOOK_AUTHOR,
        "coarse_topic_code": coarse,
        "topic_tags": topic_tags
        or ["GNN", "textbook", "graph-neural-networks", DOMAIN_PROFILE],
        "identity": {
            "title": title,
            "authors": [GNN_TEXTBOOK_AUTHOR],
            "container": GNN_TEXTBOOK_TITLE,
            "canonical_url": url,
            "citation_key": article_key.replace("-", "_"),
        },
        "domain_profile": DOMAIN_PROFILE,
        "source_strategy": {
            "primary_source_variant_id": variant_id,
            "preferred_content_order": ["textbook_html_chapter"],
            "metadata_order": ["html_meta", "manual"],
            "pdf_policy": "no_pdf_expected_for_html_chapter",
            "fallback_policy": "manual_review_when_html_missing_or_low_quality",
            "parser_readiness": "local_html_via_universal_source",
            "chunk_readiness": "structure_loaded_source",
            "graph_readiness": "not_claimed",
        },
        "source_variants": [
            {
                "variant_id": variant_id,
                "source_role": "textbook_html_chapter",
                "source_format": "html",
                "source_origin": "gnn_textbook_mkdocs",
                "is_primary": True,
                "is_content_bearing": True,
                "is_metadata_only": False,
                "path": html_rel_path,
                "url": url,
                "media_type": "text/html",
                "sha256": html_sha256,
                "byte_size": html_byte_size,
                "capture_status": "captured_local",
                "capture_policy": "local_or_bounded_fetch_into_catalog",
                "loader_outcome": "not_loaded",
                "requires_conversion": False,
                "conversion_hint": None,
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
                "network_fetch_attempted": False,
                "local_path": html_rel_path,
                "parser_readiness_claimed": False,
            }
        ],
        "expected_profile": {
            "should_load": True,
            "should_parse_text": True,
            "should_chunk": True,
            "should_have_arxiv_metadata": False,
            "domain_profile": DOMAIN_PROFILE,
            "known_risks": [
                "html_nav_boilerplate_may_need_cleanup",
                "not_graph_import_eligible",
            ],
        },
        "safety_flags": {
            "trusted_kg_import_allowed": False,
            "production_ladybugdb_write_allowed": False,
            "raw_text_embedded_in_metadata": False,
            "raw_binary_embedded_in_metadata": False,
        },
        "registration_summary": {
            "status": "registered",
            "milestone": "M223",
            "note": "ADR-032 textbook chapter; import_eligible false",
        },
    }


def build_multi_source_selection(
    rows: list[dict[str, Any]] | tuple[dict[str, Any], ...],
    *,
    selection_id: str = "m223-multi-source-strengthen-v1",
) -> dict[str, Any]:
    """Build selection package for non-arxiv / multi-source operator paths."""
    articles: list[dict[str, Any]] = []
    for raw in rows:
        if not isinstance(raw, dict):
            continue
        ref = str(raw.get("article_ref") or "").strip()
        code = str(raw.get("source_code") or "").strip()
        articles.append(
            {
                "article_ref": ref,
                "source_code": code,
                "article_key": raw.get("article_key"),
                "title": raw.get("title"),
                "primary_path": raw.get("primary_path"),
                "source_format": raw.get("source_format"),
                "domain_profile": raw.get("domain_profile"),
            }
        )
    return {
        "schema_version": SELECTION_SCHEMA_VERSION,
        "selection_id": selection_id,
        "articles": articles,
        "article_count": len(articles),
        "import_eligible": False,
        "graph_writes_allowed": False,
        "note": "multi-source strengthen selection; not hybrid TEI; not graph import",
    }


def fingerprint_html_bytes(raw: bytes) -> tuple[str, int]:
    """Return (sha256_hex, byte_size) for HTML payload."""
    return hashlib.sha256(raw).hexdigest(), len(raw)


# re-export helper name used by composition
sha256_text = _sha256_text

__all__ = [
    "ARTICLE_SCHEMA_VERSION",
    "SELECTION_SCHEMA_VERSION",
    "build_gnn_textbook_article_record",
    "build_multi_source_selection",
    "fingerprint_html_bytes",
    "gnn_chapter_article_key",
    "sha256_text",
]
