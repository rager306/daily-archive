"""Selection-vs-catalog coverage reconciliation (M210).

Classifies selection refs against the canonical article catalog index and
optional article.json presence. Missing rows become typed_catalog_blocker,
never already_cataloged. No network, no Falkor, no import authorization.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

from research_graph.domain.universal_kb.contracts import SafetyFlags

CatalogCoverageStatus = Literal[
    "cataloged",
    "missing_row",
    "missing_article_json",
    "orphan_index",
    "invalid_selection_ref",
]

BlockerCode = Literal[
    "typed_catalog_blocker",
    "missing_article_json",
    "invalid_selection_ref",
    "orphan_index_row",
]

ArticlePresenceFn = Callable[[str, Mapping[str, Any]], bool]


@dataclass(frozen=True, slots=True)
class CatalogCoverageRow:
    article_ref: str
    status: CatalogCoverageStatus
    source_code: str | None = None
    title: str | None = None
    blocker_code: BlockerCode | None = None
    notes: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "article_ref": self.article_ref,
            "status": self.status,
            "source_code": self.source_code,
            "title": self.title,
            "blocker_code": self.blocker_code,
            "notes": self.notes,
        }


@dataclass(frozen=True, slots=True)
class CatalogCoverageReport:
    """Selection-vs-catalog reconciliation report (metadata-only)."""

    selection_id: str
    rows: tuple[CatalogCoverageRow, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    production_import_attempted: bool = False
    falkor_touched: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed or self.production_import_attempted:
            raise ValueError("catalog coverage report cannot authorize import or writes")
        if self.falkor_touched:
            raise ValueError("M210 catalog coverage must not touch FalkorDB")

    def counts(self) -> dict[str, int]:
        out: dict[str, int] = {}
        for row in self.rows:
            out[row.status] = out.get(row.status, 0) + 1
        return out

    def blockers(self) -> tuple[CatalogCoverageRow, ...]:
        return tuple(row for row in self.rows if row.blocker_code is not None)

    def to_dict(self) -> dict[str, Any]:
        return {
            "selection_id": self.selection_id,
            "rows": [row.to_dict() for row in self.rows],
            "counts": self.counts(),
            "blocker_count": len(self.blockers()),
            "import_eligible": self.import_eligible,
            "graph_writes_allowed": self.graph_writes_allowed,
            "production_import_attempted": self.production_import_attempted,
            "falkor_touched": self.falkor_touched,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class CatalogCoveragePackage:
    """Operator-facing package wrapping a coverage report."""

    report: CatalogCoverageReport
    verdict: Literal["covered", "repair", "blocked"]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    production_import_attempted: bool = False
    falkor_touched: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        self.report.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed or self.production_import_attempted:
            raise ValueError("catalog coverage package cannot authorize import or writes")
        if self.falkor_touched:
            raise ValueError("M210 catalog coverage package must not touch FalkorDB")

    def to_dict(self) -> dict[str, Any]:
        payload = {
            "verdict": self.verdict,
            "report": self.report.to_dict(),
            "import_eligible": self.import_eligible,
            "graph_writes_allowed": self.graph_writes_allowed,
            "production_import_attempted": self.production_import_attempted,
            "falkor_touched": self.falkor_touched,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
            "counts": self.report.counts(),
            "blocker_count": len(self.report.blockers()),
        }
        text = str(payload).lower()
        for forbidden in ("api_key", "password", "embedding", "raw_text", "sk-"):
            if forbidden in text:
                raise ValueError(f"catalog coverage package leaked forbidden token: {forbidden}")
        return payload


def _index_by_ref(catalog_index: Mapping[str, Any]) -> dict[str, dict[str, Any]]:
    articles = catalog_index.get("articles", [])
    out: dict[str, dict[str, Any]] = {}
    if not isinstance(articles, list):
        return out
    for row in articles:
        if not isinstance(row, dict):
            continue
        ref = row.get("article_ref")
        if isinstance(ref, str) and ref:
            out[ref] = row
    return out


def _selection_articles(selection: Mapping[str, Any]) -> list[dict[str, Any]]:
    articles = selection.get("articles", [])
    if not isinstance(articles, list):
        return []
    return [row for row in articles if isinstance(row, dict)]


def default_article_json_present(
    article_ref: str,
    index_row: Mapping[str, Any],
    *,
    catalog_root: Path,
) -> bool:
    """Stdlib path check for article.json under catalog root."""
    article_path = index_row.get("article_path")
    if isinstance(article_path, str) and article_path:
        path = Path(article_path)
        if not path.is_absolute():
            path = catalog_root / path
        return path.is_file()
    # fallback: article_catalog/<ref>/article.json
    return (catalog_root / "article_catalog" / article_ref / "article.json").is_file() or (
        catalog_root / article_ref / "article.json"
    ).is_file()


def reconcile_selection_against_catalog(
    selection: Mapping[str, Any],
    catalog_index: Mapping[str, Any],
    *,
    article_present: ArticlePresenceFn | None = None,
    include_orphan_index_rows: bool = False,
) -> CatalogCoverageReport:
    """Reconcile selection refs against catalog index (+ optional article.json)."""
    selection_id = str(selection.get("selection_id") or "selection:unknown")
    index_map = _index_by_ref(catalog_index)
    selected = _selection_articles(selection)
    rows: list[CatalogCoverageRow] = []
    seen_refs: set[str] = set()

    for item in selected:
        ref = item.get("article_ref")
        source_code = item.get("source_code")
        title = item.get("title")
        if not isinstance(ref, str) or not ref:
            rows.append(
                CatalogCoverageRow(
                    article_ref=str(ref or ""),
                    status="invalid_selection_ref",
                    source_code=str(source_code) if source_code else None,
                    title=str(title) if title else None,
                    blocker_code="invalid_selection_ref",
                    notes="selection row missing article_ref",
                )
            )
            continue
        seen_refs.add(ref)
        index_row = index_map.get(ref)
        if index_row is None:
            rows.append(
                CatalogCoverageRow(
                    article_ref=ref,
                    status="missing_row",
                    source_code=str(source_code) if source_code else None,
                    title=str(title) if title else None,
                    blocker_code="typed_catalog_blocker",
                    notes="selection ref not in catalog index",
                )
            )
            continue
        if article_present is not None and not article_present(ref, index_row):
            rows.append(
                CatalogCoverageRow(
                    article_ref=ref,
                    status="missing_article_json",
                    source_code=str(index_row.get("source_code") or source_code or "") or None,
                    title=str(index_row.get("title") or title or "") or None,
                    blocker_code="missing_article_json",
                    notes="index row present but article.json missing",
                )
            )
            continue
        rows.append(
            CatalogCoverageRow(
                article_ref=ref,
                status="cataloged",
                source_code=str(index_row.get("source_code") or source_code or "") or None,
                title=str(index_row.get("title") or title or "") or None,
                notes="selection ref present in catalog index",
            )
        )

    if include_orphan_index_rows:
        for ref, index_row in sorted(index_map.items()):
            if ref in seen_refs:
                continue
            rows.append(
                CatalogCoverageRow(
                    article_ref=ref,
                    status="orphan_index",
                    source_code=str(index_row.get("source_code") or "") or None,
                    title=str(index_row.get("title") or "") or None,
                    blocker_code="orphan_index_row",
                    notes="index row not in selection",
                )
            )

    return CatalogCoverageReport(
        selection_id=selection_id,
        rows=tuple(rows),
        diagnostics=(
            "selection_vs_catalog_reconciliation",
            "falkor_deferred_by_policy",
            "import_write_fail_closed",
            f"selection_count:{len(selected)}",
            f"index_count:{len(index_map)}",
        ),
    )


def build_catalog_coverage_package(report: CatalogCoverageReport) -> CatalogCoveragePackage:
    """Wrap report with operator verdict; never authorizes import."""
    counts = report.counts()
    if counts.get("cataloged", 0) == len(report.rows) and report.rows:
        verdict: Literal["covered", "repair", "blocked"] = "covered"
    elif counts.get("cataloged", 0) > 0:
        verdict = "repair"
    else:
        verdict = "blocked"
    return CatalogCoveragePackage(
        report=report,
        verdict=verdict,
        diagnostics=(
            "catalog_coverage_package",
            f"verdict:{verdict}",
            f"blockers:{len(report.blockers())}",
        ),
    )


def reconcile_paths(
    *,
    selection: Mapping[str, Any],
    catalog_index: Mapping[str, Any],
    catalog_root: str | Path | None = None,
    check_article_json: bool = True,
    include_orphan_index_rows: bool = False,
) -> CatalogCoveragePackage:
    """Convenience path: optional article.json presence under catalog_root."""
    present_fn: ArticlePresenceFn | None = None
    if check_article_json:
        root = Path(catalog_root or ".")

        def present_fn(ref: str, row: Mapping[str, Any], _root: Path = root) -> bool:
            return default_article_json_present(ref, row, catalog_root=_root)

    report = reconcile_selection_against_catalog(
        selection,
        catalog_index,
        article_present=present_fn,
        include_orphan_index_rows=include_orphan_index_rows,
    )
    return build_catalog_coverage_package(report)


__all__ = [
    "ArticlePresenceFn",
    "BlockerCode",
    "CatalogCoveragePackage",
    "CatalogCoverageReport",
    "CatalogCoverageRow",
    "CatalogCoverageStatus",
    "build_catalog_coverage_package",
    "default_article_json_present",
    "reconcile_paths",
    "reconcile_selection_against_catalog",
]
