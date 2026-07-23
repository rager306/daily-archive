"""Shared preprocess summary for composition enrichment (M227 S01).

Builds a JSON-serializable diagnostic dict from body text via
ArticlePreprocessPackage + content fingerprint. Never authorizes import.
"""

from __future__ import annotations

from typing import Any

from research_graph.application.corpus.article_preprocess import (
    build_article_preprocess_package,
)
from research_graph.application.corpus.body_quality import BodyQualityProfile
from research_graph.application.corpus.content_fingerprint import (
    fingerprint_cleaned_body,
)


def preprocess_summary_for_body(
    *,
    source_id: str,
    text: str,
    source_class: str = "unknown",
    profile: BodyQualityProfile = "scholarly",
    is_html: bool = False,
) -> dict[str, Any]:
    """Return fail-closed preprocess summary for one body text."""
    pkg = build_article_preprocess_package(
        source_id=source_id,
        text=text,
        source_class=source_class,
        profile=profile,
        is_html=is_html,
    )
    fp = fingerprint_cleaned_body(pkg.cleaned_text)
    return {
        "source_id": source_id,
        "source_class": source_class,
        "schema_version": pkg.schema_version,
        "language": pkg.language,
        "language_confidence": pkg.language_confidence,
        "quality_status": pkg.quality_status,
        "quality_rule_hits": list(pkg.quality_rule_hits),
        "word_count": pkg.word_count,
        "outline_heading_count": pkg.outline_heading_count,
        "clean_ops": list(pkg.clean_ops),
        "html_main_content_ratio": pkg.html_main_content_ratio,
        "content_fingerprint_sha256": fp.sha256,
        "cleaned_text_chars": len(pkg.cleaned_text),
        "import_eligible": False,
    }


__all__ = ["preprocess_summary_for_body"]
