"""Shared preprocess summary for composition enrichment (M227/M228).

Builds a JSON-serializable diagnostic dict from body text via
ArticlePreprocessPackage + content fingerprint + keyword spans +
term-dense evidence window. Keywords are token-frequency (no YAKE in
application). Never authorizes import.
"""

from __future__ import annotations

import re
from collections import Counter
from typing import Any

from research_graph.application.corpus.article_preprocess import (
    build_article_preprocess_package,
)
from research_graph.application.corpus.body_quality import BodyQualityProfile
from research_graph.application.corpus.content_fingerprint import (
    fingerprint_cleaned_body,
)
from research_graph.application.corpus.keyword_spans import locate_keyword_spans
from research_graph.application.corpus.term_dense_window import term_dense_window

_TOKEN_RE = re.compile(r"[A-Za-z\u0400-\u04FF][A-Za-z\u0400-\u04FF'\-]{2,}")
_STOP = frozenset(
    """
    the and for that with this from are was were been being have has had
    not but you all can her was one our out day get has him his how its
    may new now old see two way who boy did its let put say she too use
    into than then them they will what when which your about after also
    just more most other some such only over also very
    """.split()
)


def _content_keywords(text: str, *, top_k: int = 12) -> list[str]:
    """Simple content tokens by frequency (stdlib only; not YAKE)."""
    tokens = [
        m.group(0).casefold()
        for m in _TOKEN_RE.finditer(text)
        if m.group(0).casefold() not in _STOP
    ]
    if not tokens:
        return []
    counts = Counter(tokens)
    return [w for w, _ in counts.most_common(top_k)]


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
    keywords = _content_keywords(pkg.cleaned_text)
    span_result = locate_keyword_spans(pkg.cleaned_text, keywords)
    window = term_dense_window(
        pkg.cleaned_text, spans=span_result.spans, max_chars=320
    )
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
        "content_keywords": keywords,
        "keyword_span_count": len(span_result.spans),
        "evidence_window": {
            "start": window.start,
            "end": window.end,
            "hit_count": window.hit_count,
            "snippet": window.snippet,
            "snippet_chars": len(window.snippet),
        },
        "import_eligible": False,
    }


__all__ = ["preprocess_summary_for_body"]
