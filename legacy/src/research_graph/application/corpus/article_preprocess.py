"""Article preprocess package builder (M224 S03 / M225 S03).

Composes pure clean + optional HTML main-content + body quality +
language detect + outline signals into a versioned skeleton package.
Never authorizes import or graph writes.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.body_quality import (
    BodyQualityProfile,
    assess_body_quality,
)
from research_graph.application.corpus.body_text_clean import clean_body_text
from research_graph.application.corpus.html_main_content import extract_html_main_content
from research_graph.application.corpus.language_detect import detect_text_language
from research_graph.application.corpus.outline_signals import extract_outline_signals

SCHEMA_VERSION = "article-preprocess.v1"


@dataclass(frozen=True, slots=True)
class ArticlePreprocessPackage:
    """Deterministic preprocess package for LLM-prep skeleton context."""

    schema_version: str
    source_id: str
    source_class: str
    profile: BodyQualityProfile
    cleaned_text: str
    clean_ops: tuple[str, ...]
    quality_status: str
    quality_rule_hits: tuple[str, ...]
    quality_scores: dict[str, float]
    word_count: int
    html_main_content_ratio: float | None = None
    html_region: str | None = None
    language: str = "unknown"
    language_confidence: float = 0.0
    outline_heading_count: int = 0
    outline_heading_titles: tuple[str, ...] = ()
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("article preprocess cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "source_id": self.source_id,
            "source_class": self.source_class,
            "profile": self.profile,
            "cleaned_text_chars": len(self.cleaned_text),
            "clean_ops": list(self.clean_ops),
            "quality_status": self.quality_status,
            "quality_rule_hits": list(self.quality_rule_hits),
            "quality_scores": dict(self.quality_scores),
            "word_count": self.word_count,
            "html_main_content_ratio": self.html_main_content_ratio,
            "html_region": self.html_region,
            "language": self.language,
            "language_confidence": self.language_confidence,
            "outline_heading_count": self.outline_heading_count,
            "outline_heading_titles": list(self.outline_heading_titles),
            "import_eligible": self.import_eligible,
            "graph_writes_allowed": self.graph_writes_allowed,
        }


def build_article_preprocess_package(
    *,
    source_id: str,
    text: str,
    source_class: str = "unknown",
    profile: BodyQualityProfile = "scholarly",
    is_html: bool = False,
) -> ArticlePreprocessPackage:
    """Build fail-closed preprocess package from raw text or HTML.

    Order: optional HTML main-content → body clean → quality → language → outline.
    """
    ops: list[str] = []
    working = text
    html_ratio: float | None = None
    html_region: str | None = None

    if is_html:
        html_result = extract_html_main_content(text)
        working = html_result.text
        html_ratio = html_result.main_content_ratio
        html_region = html_result.region
        ops.append("html_main_content")

    cleaned = clean_body_text(working)
    ops.extend(cleaned.ops)

    quality = assess_body_quality(cleaned.text, profile=profile)
    lang = detect_text_language(cleaned.text)
    ops.append("language_detect")
    outline = extract_outline_signals(cleaned.text)
    ops.append("outline_signals")
    titles = tuple(h.text for h in outline.headings[:32])

    return ArticlePreprocessPackage(
        schema_version=SCHEMA_VERSION,
        source_id=source_id,
        source_class=source_class,
        profile=profile,
        cleaned_text=cleaned.text,
        clean_ops=tuple(ops),
        quality_status=quality.status,
        quality_rule_hits=quality.rule_hits,
        quality_scores=dict(quality.scores),
        word_count=quality.word_count,
        html_main_content_ratio=html_ratio,
        html_region=html_region,
        language=lang.language,
        language_confidence=lang.confidence,
        outline_heading_count=len(outline.headings),
        outline_heading_titles=titles,
    )


__all__ = [
    "SCHEMA_VERSION",
    "ArticlePreprocessPackage",
    "build_article_preprocess_package",
]
