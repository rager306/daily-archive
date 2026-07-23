"""Pure multi-source catalog inventory (M222).

Scans article.v00.01 records and source_variants into fail-closed coverage.
Does not fetch network, load bodies, or authorize import.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from typing import Any

SCHEMA_VERSION = "m222-catalog-source-inventory.v1"


def _str(value: Any) -> str:
    return str(value).strip() if value is not None else ""


@dataclass(frozen=True, slots=True)
class VariantObservation:
    article_key: str
    source_code: str
    source_format: str
    source_role: str
    capture_status: str
    is_content_bearing: bool
    is_metadata_only: bool
    loader_outcome: str
    path_present: bool
    has_url: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "article_key": self.article_key,
            "source_code": self.source_code,
            "source_format": self.source_format,
            "source_role": self.source_role,
            "capture_status": self.capture_status,
            "is_content_bearing": self.is_content_bearing,
            "is_metadata_only": self.is_metadata_only,
            "loader_outcome": self.loader_outcome,
            "path_present": self.path_present,
            "has_url": self.has_url,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


@dataclass(frozen=True, slots=True)
class CatalogSourceInventoryPackage:
    schema_version: str
    article_count: int
    variant_count: int
    by_source_code: dict[str, int]
    by_source_format: dict[str, int]
    by_capture_status: dict[str, int]
    content_bearing_captured: int
    content_bearing_missing: int
    metadata_only_variants: int
    pdf_variants: int
    html_variants: int
    markdown_variants: int
    non_arxiv_articles: int
    gaps: tuple[str, ...]
    samples: tuple[VariantObservation, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("catalog source inventory cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "article_count": self.article_count,
            "variant_count": self.variant_count,
            "by_source_code": dict(self.by_source_code),
            "by_source_format": dict(self.by_source_format),
            "by_capture_status": dict(self.by_capture_status),
            "content_bearing_captured": self.content_bearing_captured,
            "content_bearing_missing": self.content_bearing_missing,
            "metadata_only_variants": self.metadata_only_variants,
            "pdf_variants": self.pdf_variants,
            "html_variants": self.html_variants,
            "markdown_variants": self.markdown_variants,
            "non_arxiv_articles": self.non_arxiv_articles,
            "gaps": list(self.gaps),
            "samples": [s.to_dict() for s in self.samples],
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": "catalog readiness only; not body ETL; not graph import",
        }


def _iter_articles(records: list[dict[str, Any]] | tuple[dict[str, Any], ...]):
    for raw in records:
        if isinstance(raw, dict):
            yield raw


def observe_variants(article: dict[str, Any]) -> list[VariantObservation]:
    article_key = _str(article.get("article_key") or article.get("catalog_path") or "?")
    source_code = _str(article.get("source_code") or "unknown") or "unknown"
    variants = article.get("source_variants")
    if not isinstance(variants, list):
        return [
            VariantObservation(
                article_key=article_key,
                source_code=source_code,
                source_format="missing_variants",
                source_role="none",
                capture_status="unknown",
                is_content_bearing=False,
                is_metadata_only=True,
                loader_outcome="not_loaded",
                path_present=False,
                has_url=False,
            )
        ]
    rows: list[VariantObservation] = []
    for v in variants:
        if not isinstance(v, dict):
            continue
        path = v.get("path")
        rows.append(
            VariantObservation(
                article_key=article_key,
                source_code=source_code,
                source_format=_str(v.get("source_format") or "unknown") or "unknown",
                source_role=_str(v.get("source_role") or "unknown") or "unknown",
                capture_status=_str(v.get("capture_status") or "unknown") or "unknown",
                is_content_bearing=bool(v.get("is_content_bearing")),
                is_metadata_only=bool(v.get("is_metadata_only")),
                loader_outcome=_str(v.get("loader_outcome") or "unknown") or "unknown",
                path_present=bool(path),
                has_url=bool(_str(v.get("url"))),
            )
        )
    return rows


def build_catalog_source_inventory(
    articles: list[dict[str, Any]] | tuple[dict[str, Any], ...],
    *,
    max_samples: int = 12,
) -> CatalogSourceInventoryPackage:
    """Aggregate multi-source readiness from in-memory article records."""
    by_code: Counter[str] = Counter()
    by_fmt: Counter[str] = Counter()
    by_cap: Counter[str] = Counter()
    observations: list[VariantObservation] = []
    article_count = 0
    non_arxiv = 0

    for article in _iter_articles(articles):
        article_count += 1
        code = _str(article.get("source_code") or "unknown") or "unknown"
        by_code[code] += 1
        if code != "arxiv":
            non_arxiv += 1
        observations.extend(observe_variants(article))

    content_captured = 0
    content_missing = 0
    metadata_only = 0
    pdf_n = html_n = md_n = 0
    for obs in observations:
        by_fmt[obs.source_format] += 1
        by_cap[obs.capture_status] += 1
        fmt = obs.source_format.lower()
        if "pdf" in fmt:
            pdf_n += 1
        if "html" in fmt:
            html_n += 1
        if "markdown" in fmt or fmt in {"md", "text/markdown"}:
            md_n += 1
        if obs.is_metadata_only:
            metadata_only += 1
        if obs.is_content_bearing:
            # captured / captured_local / captured_local_file_preexisting
            captured = obs.capture_status.startswith("captured") and obs.path_present
            if captured:
                content_captured += 1
            else:
                content_missing += 1

    gaps: list[str] = []
    if non_arxiv == 0:
        gaps.append("no_non_arxiv_articles")
    if html_n == 0:
        gaps.append("no_html_variants")
    if md_n == 0:
        gaps.append("no_markdown_variants")
    if content_missing > 0:
        gaps.append(f"content_bearing_missing:{content_missing}")
    if by_code.get("arxiv", 0) and pdf_n == 0:
        gaps.append("arxiv_without_pdf_variants")
    # known pilot gaps as explicit diagnostics when present
    if by_code.get("nature"):
        gaps.append("nature_pilot_present_check_metadata_only")
    if by_code.get("stanford"):
        gaps.append("stanford_pilot_present_check_not_captured")
    if by_code.get("company_blog"):
        gaps.append("company_blog_pilot_present")

    samples = tuple(observations[: max(0, max_samples)])
    return CatalogSourceInventoryPackage(
        schema_version=SCHEMA_VERSION,
        article_count=article_count,
        variant_count=len(observations),
        by_source_code=dict(sorted(by_code.items())),
        by_source_format=dict(sorted(by_fmt.items())),
        by_capture_status=dict(sorted(by_cap.items())),
        content_bearing_captured=content_captured,
        content_bearing_missing=content_missing,
        metadata_only_variants=metadata_only,
        pdf_variants=pdf_n,
        html_variants=html_n,
        markdown_variants=md_n,
        non_arxiv_articles=non_arxiv,
        gaps=tuple(gaps),
        samples=samples,
        diagnostics=(
            "source:article.v00.01",
            "fail_closed",
            f"articles:{article_count}",
            f"variants:{len(observations)}",
            "not_body_etl",
            "not_graph_import",
        ),
    )


__all__ = [
    "SCHEMA_VERSION",
    "CatalogSourceInventoryPackage",
    "VariantObservation",
    "build_catalog_source_inventory",
    "observe_variants",
]
