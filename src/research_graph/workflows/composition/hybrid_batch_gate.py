"""M213 hybrid batch gate composition root.

Runs single_article_pipeline over a selection.json of local PDFs with optional
live hybrid ports. Application stays pure; no graph import/writes.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.workflows.composition.single_article_pipeline import (
    SingleArticleRunRequest,
    run_single_article_pipeline,
)

DEFAULT_SELECTION = Path("artifacts/m213-hybrid-gate/selection.json")
SCHEMA_VERSION = "m213-hybrid-batch-result.v1"


@dataclass(frozen=True, slots=True)
class HybridBatchGateRequest:
    selection_path: Path = DEFAULT_SELECTION
    work_dir: Path = Path("artifacts/m213-hybrid-gate/runs")
    enable_live_hybrid: bool = False
    ensure_hybrid_containers: bool = True
    repo_root: Path = field(default_factory=lambda: Path("."))
    write_artifacts: bool = True
    # When set, skip live resolve and inject these ports (tests).
    grobid: Any = None
    opendataloader: Any = None
    # Minimum hybrid body successes required for gate_pass (S03).
    min_hybrid_success: int = 0


@dataclass(frozen=True, slots=True)
class PaperGateRow:
    paper_id: str
    pdf_path: str
    body_route: str
    body_chars: int
    hybrid_claimed_success: bool
    import_eligible: bool
    graph_writes_allowed: bool
    readiness_verdict: str | None
    package_path: str | None
    diagnostics: tuple[str, ...]
    error: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "pdf_path": self.pdf_path,
            "body_route": self.body_route,
            "body_chars": self.body_chars,
            "hybrid_claimed_success": self.hybrid_claimed_success,
            "import_eligible": self.import_eligible,
            "graph_writes_allowed": self.graph_writes_allowed,
            "readiness_verdict": self.readiness_verdict,
            "package_path": self.package_path,
            "diagnostics": list(self.diagnostics),
            "error": self.error,
        }


@dataclass(frozen=True, slots=True)
class HybridBatchGateResult:
    schema_version: str
    selection_path: str
    paper_count: int
    rows: tuple[PaperGateRow, ...]
    hybrid_success_count: int
    hybrid_deferred_count: int
    other_route_count: int
    error_count: int
    import_eligible_any: bool
    graph_writes_any: bool
    gate_pass: bool
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible_any or self.graph_writes_any:
            raise ValueError("hybrid batch gate cannot authorize import or graph writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "selection_path": self.selection_path,
            "paper_count": self.paper_count,
            "hybrid_success_count": self.hybrid_success_count,
            "hybrid_deferred_count": self.hybrid_deferred_count,
            "other_route_count": self.other_route_count,
            "error_count": self.error_count,
            "import_eligible_any": False,
            "graph_writes_any": False,
            "gate_pass": self.gate_pass,
            "papers": [r.to_dict() for r in self.rows],
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
        }


def load_selection(path: Path) -> list[dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    papers = payload.get("papers")
    if not isinstance(papers, list) or not papers:
        raise ValueError(f"selection missing papers: {path}")
    return [p for p in papers if isinstance(p, dict)]


def run_hybrid_batch_gate(request: HybridBatchGateRequest) -> HybridBatchGateResult:
    """Run hybrid body path over selection; fail-closed on import/writes."""
    selection_path = request.selection_path
    if not selection_path.is_file():
        selection_path = request.repo_root / selection_path
    papers = load_selection(selection_path)
    work_root = request.work_dir
    work_root.mkdir(parents=True, exist_ok=True)

    rows: list[PaperGateRow] = []
    batch_diag: list[str] = [
        f"enable_live_hybrid:{request.enable_live_hybrid}",
        f"selection:{selection_path}",
    ]

    for paper in papers:
        paper_id = str(paper.get("paper_id") or "unknown")
        rel_pdf = str(paper.get("pdf_path") or "")
        pdf_path = Path(rel_pdf)
        if not pdf_path.is_file():
            pdf_path = request.repo_root / rel_pdf
        paper_work = work_root / paper_id
        paper_work.mkdir(parents=True, exist_ok=True)

        if not pdf_path.is_file():
            rows.append(
                PaperGateRow(
                    paper_id=paper_id,
                    pdf_path=rel_pdf,
                    body_route="unavailable",
                    body_chars=0,
                    hybrid_claimed_success=False,
                    import_eligible=False,
                    graph_writes_allowed=False,
                    readiness_verdict=None,
                    package_path=None,
                    diagnostics=("missing_pdf",),
                    error=f"missing_pdf:{pdf_path}",
                )
            )
            continue

        try:
            result = run_single_article_pipeline(
                SingleArticleRunRequest(
                    source=str(pdf_path.resolve()),
                    work_dir=paper_work,
                    mode="hybrid",
                    prefer="pdf",
                    also_pdf=False,
                    allow_network=False,
                    review_completed=True,
                    repo_root=request.repo_root,
                ),
                grobid=request.grobid,
                opendataloader=request.opendataloader,
                hybrid_pdf_path=pdf_path.resolve(),
                enable_live_hybrid=request.enable_live_hybrid
                and request.grobid is None
                and request.opendataloader is None,
                ensure_hybrid_containers=request.ensure_hybrid_containers,
                write_artifacts=request.write_artifacts,
            )
            payload = result.to_dict()
            verdict = None
            if result.readiness is not None:
                verdict = str(result.readiness.package.verdict)
            rows.append(
                PaperGateRow(
                    paper_id=result.paper_id,
                    pdf_path=rel_pdf,
                    body_route=str(result.body_route),
                    body_chars=int(result.body.body_chars),
                    hybrid_claimed_success=bool(payload.get("hybrid_claimed_success")),
                    import_eligible=False,
                    graph_writes_allowed=False,
                    readiness_verdict=verdict,
                    package_path=str(result.package_path) if result.package_path else None,
                    diagnostics=tuple(result.body.diagnostics),
                    error=None,
                )
            )
        except Exception as exc:  # noqa: BLE001 - batch continues fail-closed per paper
            rows.append(
                PaperGateRow(
                    paper_id=paper_id,
                    pdf_path=rel_pdf,
                    body_route="unavailable",
                    body_chars=0,
                    hybrid_claimed_success=False,
                    import_eligible=False,
                    graph_writes_allowed=False,
                    readiness_verdict=None,
                    package_path=None,
                    diagnostics=(f"error:{type(exc).__name__}",),
                    error=f"{type(exc).__name__}:{exc}",
                )
            )

    hybrid_ok = sum(1 for r in rows if r.hybrid_claimed_success and r.body_route == "hybrid")
    deferred = sum(1 for r in rows if r.body_route == "hybrid_deferred")
    errors = sum(1 for r in rows if r.error)
    other = len(rows) - hybrid_ok - deferred - errors
    body_chars_total = sum(r.body_chars for r in rows)
    # Structural pass: no per-paper crash path that sets error when min threshold is the success bar;
    # hard fail on any import/write; require min_hybrid_success hybrid body successes.
    fail_closed_ok = all((not r.import_eligible) and (not r.graph_writes_allowed) for r in rows)
    gate_pass = (
        fail_closed_ok
        and errors == 0
        and hybrid_ok >= int(request.min_hybrid_success)
    )
    batch_diag.append(f"hybrid_ok:{hybrid_ok}")
    batch_diag.append(f"min_hybrid_success:{request.min_hybrid_success}")
    batch_diag.append(f"body_chars_total:{body_chars_total}")

    result = HybridBatchGateResult(
        schema_version=SCHEMA_VERSION,
        selection_path=str(selection_path),
        paper_count=len(rows),
        rows=tuple(rows),
        hybrid_success_count=hybrid_ok,
        hybrid_deferred_count=deferred,
        other_route_count=max(0, other),
        error_count=errors,
        import_eligible_any=False,
        graph_writes_any=False,
        gate_pass=gate_pass,
        diagnostics=tuple(batch_diag),
    )

    if request.write_artifacts:
        out = work_root / "batch-summary.json"
        out.write_text(json.dumps(result.to_dict(), indent=2, sort_keys=True) + "\n", encoding="utf-8")

    return result


__all__ = [
    "DEFAULT_SELECTION",
    "HybridBatchGateRequest",
    "HybridBatchGateResult",
    "PaperGateRow",
    "SCHEMA_VERSION",
    "load_selection",
    "run_hybrid_batch_gate",
]
