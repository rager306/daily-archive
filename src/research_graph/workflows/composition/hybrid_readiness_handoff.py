"""M216: hybrid coverage + graph-data readiness handoff composition.

Orchestrates:
1) hybrid selection → catalog coverage (M215)
2) resolve hybrid body markdown paths under a body_root
3) no-write graph-data readiness on available bodies (M209)

Never authorizes import/writes. Does not start GROBID/ODL (bodies must pre-exist).
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.workflows.composition.graph_data_readiness import (
    GraphDataReadinessRequest,
    GraphDataReadinessResult,
    SourceInput,
    run_graph_data_readiness_pipeline,
)
from research_graph.workflows.composition.hybrid_catalog_coverage import (
    HybridCatalogCoverageRequest,
    HybridCatalogCoverageResult,
    run_hybrid_catalog_coverage,
)

DEFAULT_SELECTION = Path("artifacts/m213-hybrid-gate/selection-20.json")
DEFAULT_BODY_ROOT = Path("artifacts/m213-hybrid-gate/runs-live-20")
DEFAULT_CATALOG_INDEX = Path("data/article_catalog/index.json")
DEFAULT_CATALOG_ROOT = Path("data/article_catalog")
SCHEMA_VERSION = "m216-hybrid-readiness-handoff.v1"

HandoffVerdict = Literal["ready_for_review", "repair", "blocked"]


@dataclass(frozen=True, slots=True)
class BodyPathResolution:
    paper_id: str
    pdf_path: str
    body_path: str | None
    found: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "pdf_path": self.pdf_path,
            "body_path": self.body_path,
            "found": self.found,
        }


def resolve_hybrid_body_paths(
    hybrid_selection: dict[str, Any],
    *,
    body_root: Path,
) -> tuple[BodyPathResolution, ...]:
    """Resolve `{body_root}/{paper_id}/body/{paper_id}.hybrid.body.md` paths.

    Pure path logic (no network). Missing bodies are reported, not invented.
    """
    papers = hybrid_selection.get("papers")
    if not isinstance(papers, list):
        return ()
    rows: list[BodyPathResolution] = []
    for raw in papers:
        if not isinstance(raw, dict):
            continue
        paper_id = str(raw.get("paper_id") or "").strip()
        pdf_path = str(raw.get("pdf_path") or "")
        if not paper_id:
            rows.append(
                BodyPathResolution(
                    paper_id="",
                    pdf_path=pdf_path,
                    body_path=None,
                    found=False,
                )
            )
            continue
        candidate = body_root / paper_id / "body" / f"{paper_id}.hybrid.body.md"
        found = candidate.is_file()
        rows.append(
            BodyPathResolution(
                paper_id=paper_id,
                pdf_path=pdf_path,
                body_path=str(candidate) if found else str(candidate),
                found=found,
            )
        )
    return tuple(rows)


def _combine_verdict(
    *,
    coverage_verdict: str,
    readiness_verdict: str | None,
    missing_body_count: int,
    paper_count: int,
) -> HandoffVerdict:
    if coverage_verdict == "blocked":
        return "blocked"
    if missing_body_count == paper_count and paper_count > 0:
        return "blocked"
    if coverage_verdict == "repair" or missing_body_count > 0:
        return "repair"
    if readiness_verdict == "blocked":
        return "blocked"
    if readiness_verdict == "repair":
        return "repair"
    if readiness_verdict == "ready_for_review" and missing_body_count == 0:
        return "ready_for_review"
    return "repair"


@dataclass(frozen=True, slots=True)
class HybridReadinessHandoffRequest:
    hybrid_selection_path: Path = DEFAULT_SELECTION
    body_root: Path = DEFAULT_BODY_ROOT
    catalog_index_path: Path = DEFAULT_CATALOG_INDEX
    catalog_root: Path = DEFAULT_CATALOG_ROOT
    check_article_json: bool = True
    review_completed: bool = True
    require_min_chunks: int = 1
    output_path: Path | None = None
    repo_root: Path = field(default_factory=lambda: Path("."))
    # When True, skip readiness if no bodies (still emit coverage).
    allow_empty_readiness: bool = True


@dataclass(frozen=True, slots=True)
class HybridReadinessHandoffResult:
    schema_version: str
    handoff_verdict: HandoffVerdict
    coverage: HybridCatalogCoverageResult
    readiness: GraphDataReadinessResult | None
    body_resolutions: tuple[BodyPathResolution, ...]
    bodies_found: int
    bodies_missing: int
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()
    output_path: str | None = None

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("hybrid readiness handoff cannot authorize import or writes")
        if self.coverage.package.report.import_eligible:
            raise ValueError("coverage package cannot authorize import inside handoff")
        if self.readiness is not None and self.readiness.package.import_eligible:
            raise ValueError("readiness package cannot authorize import inside handoff")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "handoff_verdict": self.handoff_verdict,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "bodies_found": self.bodies_found,
            "bodies_missing": self.bodies_missing,
            "coverage": self.coverage.to_dict(),
            "readiness": {
                "package": self.readiness.package.to_dict() if self.readiness else None,
                "continuity_report_markdown": (
                    self.readiness.continuity_report_markdown if self.readiness else None
                ),
            },
            "body_resolutions": [row.to_dict() for row in self.body_resolutions],
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
            "output_path": self.output_path,
        }


def _resolve(path: Path, repo_root: Path) -> Path:
    if path.is_file() or path.is_dir() or path.is_absolute():
        return path
    return repo_root / path


def run_hybrid_readiness_handoff(
    request: HybridReadinessHandoffRequest,
) -> HybridReadinessHandoffResult:
    """Compose catalog coverage + optional readiness over hybrid body artifacts."""
    repo = request.repo_root
    sel_path = _resolve(request.hybrid_selection_path, repo)
    body_root = _resolve(request.body_root, repo)
    if not sel_path.is_file():
        raise FileNotFoundError(f"hybrid selection missing: {sel_path}")

    hybrid_selection = json.loads(sel_path.read_text(encoding="utf-8"))
    bodies = resolve_hybrid_body_paths(hybrid_selection, body_root=body_root)
    found = sum(1 for row in bodies if row.found)
    missing = sum(1 for row in bodies if not row.found)

    coverage = run_hybrid_catalog_coverage(
        HybridCatalogCoverageRequest(
            hybrid_selection_path=sel_path,
            catalog_index_path=_resolve(request.catalog_index_path, repo),
            catalog_root=_resolve(request.catalog_root, repo),
            check_article_json=request.check_article_json,
            output_path=None,
            repo_root=repo,
        )
    )

    readiness: GraphDataReadinessResult | None = None
    readiness_verdict: str | None = None
    if found > 0:
        sources = tuple(
            SourceInput(
                path=str(row.body_path),
                paper_id=row.paper_id,
                source_type="markdown",
            )
            for row in bodies
            if row.found and row.body_path
        )
        readiness = run_graph_data_readiness_pipeline(
            GraphDataReadinessRequest(
                sources=sources,
                review_completed=request.review_completed,
                repo_root=str(repo),
                require_min_chunks=request.require_min_chunks,
            )
        )
        readiness_verdict = readiness.package.verdict
    elif not request.allow_empty_readiness:
        raise FileNotFoundError(f"no hybrid bodies under {body_root}")

    handoff_verdict = _combine_verdict(
        coverage_verdict=coverage.package.verdict,
        readiness_verdict=readiness_verdict,
        missing_body_count=missing,
        paper_count=len(bodies),
    )

    diagnostics = (
        f"bodies_found:{found}",
        f"bodies_missing:{missing}",
        f"coverage_verdict:{coverage.package.verdict}",
        f"readiness_verdict:{readiness_verdict or 'skipped_no_bodies'}",
        f"handoff_verdict:{handoff_verdict}",
        "import_write_fail_closed",
        "no_live_sidecar_start",
    )

    out_path = request.output_path
    if out_path is not None:
        out_path = _resolve(out_path, repo)
        out_path.parent.mkdir(parents=True, exist_ok=True)

    result = HybridReadinessHandoffResult(
        schema_version=SCHEMA_VERSION,
        handoff_verdict=handoff_verdict,
        coverage=coverage,
        readiness=readiness,
        body_resolutions=bodies,
        bodies_found=found,
        bodies_missing=missing,
        diagnostics=diagnostics,
        output_path=str(out_path) if out_path else None,
    )
    if out_path is not None:
        out_path.write_text(
            json.dumps(result.to_dict(), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    return result


__all__ = [
    "DEFAULT_BODY_ROOT",
    "DEFAULT_SELECTION",
    "BodyPathResolution",
    "HybridReadinessHandoffRequest",
    "HybridReadinessHandoffResult",
    "SCHEMA_VERSION",
    "resolve_hybrid_body_paths",
    "run_hybrid_readiness_handoff",
]
