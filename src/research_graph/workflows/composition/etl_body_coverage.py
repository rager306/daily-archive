"""Composition: Wave A ETL body coverage audit (M241).

Read-only orchestration over catalog index + hybrid body roots.
Never authorizes import or graph writes.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from research_graph.application.corpus.etl_body_coverage_audit import (
    EtlBodyCoveragePackage,
    audit_catalog_body_coverage,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags

DEFAULT_CATALOG_INDEX = Path("data/article_catalog/index.json")
DEFAULT_CATALOG_ROOT = Path("data/article_catalog")
DEFAULT_BODY_ROOTS: tuple[Path, ...] = (
    # Expand batch work_dir first so new hybrid bodies join coverage (M247).
    Path("artifacts/m213-hybrid-gate/runs-live-expand"),
    Path("artifacts/m213-hybrid-gate/runs-live-20"),
    Path("artifacts/m213-hybrid-gate/runs-live"),
    Path("artifacts/m213-hybrid-gate/runs-live-scholarly-20"),
    Path("artifacts/m213-hybrid-gate/runs-live-scholarly"),
)


@dataclass(frozen=True, slots=True)
class EtlBodyCoverageRequest:
    catalog_index_path: Path = DEFAULT_CATALOG_INDEX
    catalog_root: Path = DEFAULT_CATALOG_ROOT
    body_roots: tuple[Path, ...] = DEFAULT_BODY_ROOTS
    repo_root: Path = field(default_factory=lambda: Path("."))
    sample_limit: int = 12


@dataclass(frozen=True, slots=True)
class EtlBodyCoverageResult:
    package: EtlBodyCoveragePackage
    body_roots_used: tuple[str, ...]
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("etl body coverage result cannot authorize import/writes")
        if self.package.import_eligible or self.package.graph_writes_allowed:
            raise ValueError("etl body coverage package cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "package": self.package.to_dict(),
            "body_roots_used": list(self.body_roots_used),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
        }


def _resolve(path: Path, repo_root: Path) -> Path:
    p = Path(path)
    if p.is_absolute():
        return p
    return (repo_root / p).resolve()


def run_etl_body_coverage_audit(
    request: EtlBodyCoverageRequest | None = None,
) -> EtlBodyCoverageResult:
    """Run read-only catalog↔hybrid body coverage audit."""
    req = request or EtlBodyCoverageRequest()
    repo = Path(req.repo_root)
    index_path = _resolve(req.catalog_index_path, repo)
    catalog_root = _resolve(req.catalog_root, repo)
    roots = tuple(_resolve(r, repo) for r in req.body_roots)
    existing_roots = tuple(r for r in roots if r.is_dir())

    package = audit_catalog_body_coverage(
        catalog_index_path=index_path,
        body_roots=existing_roots,
        catalog_root=catalog_root,
        sample_limit=req.sample_limit,
    )
    diagnostics = (
        *package.diagnostics,
        f"body_roots_configured:{len(roots)}",
        f"body_roots_existing:{len(existing_roots)}",
        "composition_read_only",
    )
    return EtlBodyCoverageResult(
        package=package,
        body_roots_used=tuple(str(r) for r in existing_roots),
        diagnostics=diagnostics,
    )


__all__ = [
    "DEFAULT_BODY_ROOTS",
    "DEFAULT_CATALOG_INDEX",
    "DEFAULT_CATALOG_ROOT",
    "EtlBodyCoverageRequest",
    "EtlBodyCoverageResult",
    "run_etl_body_coverage_audit",
]
