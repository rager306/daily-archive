"""Composition: hybrid selection → catalog coverage package.

Loads hybrid gate selection + catalog index JSON (stdlib), maps via application
helper, reconciles with M210 reconcile_paths. Never authorizes import/writes.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from research_graph.application.corpus.catalog_coverage_reconciliation import (
    CatalogCoveragePackage,
    reconcile_paths,
)
from research_graph.application.corpus.hybrid_selection_map import (
    map_hybrid_selection_to_catalog_selection,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags

DEFAULT_HYBRID_SELECTION = Path("artifacts/m213-hybrid-gate/selection-20.json")
DEFAULT_CATALOG_INDEX = Path("data/article_catalog/index.json")
DEFAULT_CATALOG_ROOT = Path("data/article_catalog")
SCHEMA_VERSION = "hybrid-catalog-coverage.v1"


@dataclass(frozen=True, slots=True)
class HybridCatalogCoverageRequest:
    hybrid_selection_path: Path = DEFAULT_HYBRID_SELECTION
    catalog_index_path: Path = DEFAULT_CATALOG_INDEX
    catalog_root: Path = DEFAULT_CATALOG_ROOT
    check_article_json: bool = True
    include_orphan_index_rows: bool = False
    output_path: Path | None = None
    repo_root: Path = field(default_factory=lambda: Path("."))


@dataclass(frozen=True, slots=True)
class HybridCatalogCoverageResult:
    schema_version: str
    package: CatalogCoveragePackage
    mapped_selection_id: str
    paper_count: int
    cataloged_count: int
    blocker_count: int
    hybrid_selection_path: str
    catalog_index_path: str
    output_path: str | None
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.package.report.import_eligible or self.package.report.graph_writes_allowed:
            raise ValueError("hybrid catalog coverage cannot authorize import or writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "mapped_selection_id": self.mapped_selection_id,
            "paper_count": self.paper_count,
            "cataloged_count": self.cataloged_count,
            "blocker_count": self.blocker_count,
            "verdict": self.package.verdict,
            "hybrid_selection_path": self.hybrid_selection_path,
            "catalog_index_path": self.catalog_index_path,
            "output_path": self.output_path,
            "package": self.package.to_dict(),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
        }


def _resolve(path: Path, repo_root: Path) -> Path:
    if path.is_file() or path.is_absolute():
        return path
    return repo_root / path


def run_hybrid_catalog_coverage(
    request: HybridCatalogCoverageRequest,
) -> HybridCatalogCoverageResult:
    """Map hybrid selection and reconcile against catalog index."""
    sel_path = _resolve(request.hybrid_selection_path, request.repo_root)
    idx_path = _resolve(request.catalog_index_path, request.repo_root)
    catalog_root = _resolve(request.catalog_root, request.repo_root)
    if not sel_path.is_file():
        raise FileNotFoundError(f"hybrid selection missing: {sel_path}")
    if not idx_path.is_file():
        raise FileNotFoundError(f"catalog index missing: {idx_path}")

    hybrid_selection = json.loads(sel_path.read_text(encoding="utf-8"))
    catalog_index = json.loads(idx_path.read_text(encoding="utf-8"))
    mapped = map_hybrid_selection_to_catalog_selection(hybrid_selection)
    package = reconcile_paths(
        selection=mapped,
        catalog_index=catalog_index,
        catalog_root=catalog_root,
        check_article_json=request.check_article_json,
        include_orphan_index_rows=request.include_orphan_index_rows,
    )
    counts = package.report.counts()
    cataloged = int(counts.get("cataloged", 0))
    blockers = len(package.report.blockers())
    out_path = request.output_path
    if out_path is not None:
        out_path = _resolve(out_path, request.repo_root)
        out_path.parent.mkdir(parents=True, exist_ok=True)

    result = HybridCatalogCoverageResult(
        schema_version=SCHEMA_VERSION,
        package=package,
        mapped_selection_id=str(mapped.get("selection_id") or ""),
        paper_count=int(mapped.get("paper_count") or 0),
        cataloged_count=cataloged,
        blocker_count=blockers,
        hybrid_selection_path=str(sel_path),
        catalog_index_path=str(idx_path),
        output_path=str(out_path) if out_path else None,
        diagnostics=tuple(mapped.get("diagnostics") or ())
        + (
            f"verdict:{package.verdict}",
            f"cataloged:{cataloged}",
            f"blockers:{blockers}",
            "import_write_fail_closed",
        ),
    )
    if out_path is not None:
        out_path.write_text(
            json.dumps(result.to_dict(), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    return result


__all__ = [
    "DEFAULT_CATALOG_INDEX",
    "DEFAULT_CATALOG_ROOT",
    "DEFAULT_HYBRID_SELECTION",
    "HybridCatalogCoverageRequest",
    "HybridCatalogCoverageResult",
    "SCHEMA_VERSION",
    "run_hybrid_catalog_coverage",
]
