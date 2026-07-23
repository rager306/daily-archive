"""Composition-boundary YAKE keyword extraction (M230/M231/M232).

Uses infrastructure KeywordExtractor so application corpus stays free of YAKE
(ADR-036). Returns plain keyword strings for inject into
preprocess_summary_for_body(keywords=...). Never authorizes import.

M231: map preprocess language codes to YAKE ``lan`` values.
M232: prefer cleaned body (HTML main-content + clean) as YAKE input so
keywords align with span/window stages on cleaned text.
"""

from __future__ import annotations

from research_graph.application.corpus.body_text_clean import clean_body_text
from research_graph.application.corpus.html_main_content import (
    extract_html_main_content,
)
from research_graph.infrastructure.retrieval.keyword_extractor import KeywordExtractor

# Preprocess detect codes we pass through to YAKE lan when supported.
_YAKE_LAN: frozenset[str] = frozenset({"en", "ru", "de", "fr"})


def yake_language_code(language: str | None) -> str:
    """Map preprocess language code to a YAKE lan value.

    Unknown / empty / unsupported codes fall back to ``en`` so KeywordExtractor
    never receives an invalid lan.
    """
    if not language:
        return "en"
    raw = str(language).strip().casefold().replace("_", "-")
    primary = raw.split("-", 1)[0] if raw else ""
    if primary in _YAKE_LAN:
        return primary
    return "en"


def cleaned_body_for_yake(text: str, *, is_html: bool = False) -> str:
    """Mirror preprocess package body path for YAKE input alignment.

    Order matches ``build_article_preprocess_package``:
    optional HTML main-content → ``clean_body_text``.
    """
    working = text if text else ""
    if is_html and working:
        working = extract_html_main_content(working).text
    return clean_body_text(working).text


def yake_keywords_for_text(
    text: str,
    *,
    language: str = "en",
    top_k: int = 12,
) -> list[str]:
    """Extract YAKE keyword strings from body text at the composition boundary.

    Empty/whitespace text returns []. Failures return [] (enrichment only).
    ``language`` is normalized via :func:`yake_language_code` before YAKE.
    Callers should pass :func:`cleaned_body_for_yake` output when aligning
    with preprocess spans.
    """
    stripped = text.strip()
    if not stripped:
        return []
    lan = yake_language_code(language)
    try:
        extractor = KeywordExtractor(language=lan, top_k=top_k)
        return extractor.extract_for_text_parts([stripped])
    except Exception:  # noqa: BLE001 - enrichment must not break operators
        return []


__all__ = [
    "cleaned_body_for_yake",
    "yake_keywords_for_text",
    "yake_language_code",
]
