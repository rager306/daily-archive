"""Wave A: hybrid-missing catalog papers vs local PDF readiness.

Read-only queue signal for hybrid expand: among catalog articles without a
hybrid body, how many already have a local PDF under the catalog tree.

Never network, never authorizes import, never starts hybrid batch.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.application.corpus.etl_body_coverage_audit import (
    _load_articles,
    find_hybrid_body,
    paper_id_for_article,
)

SCHEMA_VERSION = "etl-hybrid-missing-pdf-readiness.v1"


def _find_local_pdf(paper_id: str, catalog_root: Path, article: Mapping[str, Any]) -> Path | None:
    """Best-effort local PDF resolution under catalog_root (no network)."""
    if not paper_id:
        return None
    root = Path(catalog_root)
    # Convention: article_catalog/arxiv/<topic>/<id>/source/<id>.pdf
    hits = list(root.rglob(f"{paper_id}.pdf"))
    for hit in hits:
        if hit.is_file():
            return hit
    # Also try path relative to article_path parent/source
    rel = str(article.get("article_path") or "").strip()
    if rel:
        article_json = root / rel
        if not article_json.is_file():
            article_json = root / "article_catalog" / rel.replace("article_catalog/", "", 1)
        if article_json.is_file():
            source_dir = article_json.parent / "source"
            candidate = source_dir / f"{paper_id}.pdf"
            if candidate.is_file():
                return candidate
    return None


@dataclass(frozen=True, slots=True)
class HybridMissingPdfSample:
    paper_id: str
    source_code: str
    article_ref: str
    local_pdf_present: bool
    local_pdf_path: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "source_code": self.source_code,
            "article_ref": self.article_ref,
            "local_pdf_present": self.local_pdf_present,
            "local_pdf_path": self.local_pdf_path,
            "import_eligible": False,
        }


@dataclass(frozen=True, slots=True)
class HybridMissingPdfReadinessPackage:
    schema_version: str
    article_count: int
    hybrid_found_count: int
    hybrid_missing_count: int
    missing_with_local_pdf_count: int
    missing_without_local_pdf_count: int
    expand_ready_sample: tuple[HybridMissingPdfSample, ...]
    expand_blocked_sample: tuple[HybridMissingPdfSample, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("hybrid-missing pdf readiness cannot authorize import/writes")

    @property
    def expand_ready_fraction_of_missing(self) -> float:
        if self.hybrid_missing_count <= 0:
            return 0.0
        return round(self.missing_with_local_pdf_count / self.hybrid_missing_count, 4)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "article_count": self.article_count,
            "hybrid_found_count": self.hybrid_found_count,
            "hybrid_missing_count": self.hybrid_missing_count,
            "missing_with_local_pdf_count": self.missing_with_local_pdf_count,
            "missing_without_local_pdf_count": self.missing_without_local_pdf_count,
            "expand_ready_fraction_of_missing": self.expand_ready_fraction_of_missing,
            "expand_ready_sample": [s.to_dict() for s in self.expand_ready_sample],
            "expand_blocked_sample": [s.to_dict() for s in self.expand_blocked_sample],
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave A expand queue signal only. "
                "missing_with_local_pdf = hybrid expand candidates with local PDF. "
                "Does not start hybrid batch; does not authorize import."
            ),
        }


def audit_hybrid_missing_pdf_readiness(
    *,
    catalog_index_path: Path,
    catalog_root: Path,
    body_roots: Sequence[Path] = (),
    sample_limit: int = 12,
) -> HybridMissingPdfReadinessPackage:
    """Inventory hybrid-missing articles by local PDF presence (read-only)."""
    index_path = Path(catalog_index_path)
    articles = _load_articles(index_path) if index_path.is_file() else []
    roots = tuple(Path(r) for r in body_roots)
    cat_root = Path(catalog_root)

    missing_with = 0
    missing_without = 0
    ready_all: list[HybridMissingPdfSample] = []
    blocked_all: list[HybridMissingPdfSample] = []
    hybrid_found = 0
    for art in articles:
        paper_id = paper_id_for_article(art)
        source = str(art.get("source_code") or "unknown").strip() or "unknown"
        ref = str(art.get("article_ref") or art.get("article_key") or paper_id)
        if find_hybrid_body(paper_id, roots) is not None:
            hybrid_found += 1
            continue
        pdf = _find_local_pdf(paper_id, cat_root, art)
        sample = HybridMissingPdfSample(
            paper_id=paper_id,
            source_code=source,
            article_ref=ref,
            local_pdf_present=pdf is not None,
            local_pdf_path=str(pdf) if pdf is not None else "",
        )
        if pdf is not None:
            missing_with += 1
            if len(ready_all) < sample_limit:
                ready_all.append(sample)
        else:
            missing_without += 1
            if len(blocked_all) < sample_limit:
                blocked_all.append(sample)

    missing = missing_with + missing_without
    diagnostics = (
        f"articles:{len(articles)}",
        f"hybrid_found:{hybrid_found}",
        f"hybrid_missing:{missing}",
        f"missing_with_pdf:{missing_with}",
        f"missing_without_pdf:{missing_without}",
        "import_write_fail_closed",
        "wave_a_expand_queue_only",
    )
    return HybridMissingPdfReadinessPackage(
        schema_version=SCHEMA_VERSION,
        article_count=len(articles),
        hybrid_found_count=hybrid_found,
        hybrid_missing_count=missing,
        missing_with_local_pdf_count=missing_with,
        missing_without_local_pdf_count=missing_without,
        expand_ready_sample=tuple(ready_all),
        expand_blocked_sample=tuple(blocked_all),
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "HybridMissingPdfReadinessPackage",
    "HybridMissingPdfSample",
    "audit_hybrid_missing_pdf_readiness",
]
