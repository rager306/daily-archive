"""Composition-boundary YAKE keyword extraction (M230 S02).

Uses infrastructure KeywordExtractor so application corpus stays free of YAKE
(ADR-036). Returns plain keyword strings for inject into
preprocess_summary_for_body(keywords=...). Never authorizes import.
"""

from __future__ import annotations

from research_graph.infrastructure.retrieval.keyword_extractor import KeywordExtractor


def yake_keywords_for_text(
    text: str,
    *,
    language: str = "en",
    top_k: int = 12,
) -> list[str]:
    """Extract YAKE keyword strings from body text at the composition boundary.

    Empty/whitespace text returns []. Failures return [] (enrichment only).
    """
    stripped = text.strip()
    if not stripped:
        return []
    try:
        extractor = KeywordExtractor(language=language, top_k=top_k)
        return extractor.extract_for_text_parts([stripped])
    except Exception:  # noqa: BLE001 - enrichment must not break operators
        return []


__all__ = ["yake_keywords_for_text"]
