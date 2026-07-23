"""Composition-boundary YAKE keyword extraction (M230/M231).

Uses infrastructure KeywordExtractor so application corpus stays free of YAKE
(ADR-036). Returns plain keyword strings for inject into
preprocess_summary_for_body(keywords=...). Never authorizes import.

M231: map preprocess language codes to YAKE ``lan`` values.
"""

from __future__ import annotations

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


def yake_keywords_for_text(
    text: str,
    *,
    language: str = "en",
    top_k: int = 12,
) -> list[str]:
    """Extract YAKE keyword strings from body text at the composition boundary.

    Empty/whitespace text returns []. Failures return [] (enrichment only).
    ``language`` is normalized via :func:`yake_language_code` before YAKE.
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


__all__ = ["yake_keywords_for_text", "yake_language_code"]
