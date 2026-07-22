"""No-write graph data readiness composition root (M209).

Composes existing universal_source, projection, and promotion seams into one
application-owned path that prepares graph-ready *data evidence* without
touching FalkorDB or authorizing production import/writes.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

from research_graph.application.graph.falkor_operation_plan import build_falkor_operation_plan
from research_graph.application.graph.promotion_boundary import (
    PilotEligibilityDecision,
    decide_pilot_eligibility,
    observe_import_boundary,
    observe_readiness_handoff,
    observe_review_post_check,
    observe_schema_gate,
    trace_promotion_boundary_gaps,
)
from research_graph.application.pipeline_continuity import (
    ContinuityAudit,
    build_continuity_audit,
    render_continuity_report,
)
from research_graph.workflows.composition.universal_source import (
    StructuredSourceBundle,
    statistical_candidates_from_bundle,
    structure_loaded_source,
    validate_source_kind_provenance,
)
from research_graph.domain.graph_projection_schema import GraphProjectionSchemaGate
from research_graph.domain.ports import ProjectionRequest
from research_graph.domain.universal_kb.contracts import CandidatePacket, SafetyFlags
from research_graph.infrastructure.corpus.ingestion.loader import load_article_source
from research_graph.infrastructure.graph.networkx_probe import NetworkXProjectionAdapter
from research_graph.infrastructure.graph.projection_backends import DisabledFalkorProjectionAdapter

StageName = Literal[
    "load",
    "structure",
    "candidate",
    "projection",
    "promotion",
    "package",
]
StageStatus = Literal["pending", "done", "skipped", "failed"]
ReadinessVerdict = Literal["ready_for_review", "repair", "blocked"]


@dataclass(frozen=True, slots=True)
class SourceInput:
    """One local source for the no-write readiness pipeline."""

    path: str
    paper_id: str
    source_type: str = "auto"  # auto|html|markdown|text


@dataclass(frozen=True, slots=True)
class StageResult:
    stage: StageName
    status: StageStatus
    diagnostics: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "stage": self.stage,
            "status": self.status,
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class PerSourceReadiness:
    paper_id: str
    source_kind: str
    load_ok: bool
    structure_ok: bool
    candidate_ok: bool
    projection_ok: bool
    pilot_eligible: bool
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    chunk_count: int = 0
    evidence_count: int = 0
    blockers: tuple[str, ...] = ()
    stages: tuple[StageResult, ...] = ()

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("per-source readiness cannot authorize import or writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "source_kind": self.source_kind,
            "load_ok": self.load_ok,
            "structure_ok": self.structure_ok,
            "candidate_ok": self.candidate_ok,
            "projection_ok": self.projection_ok,
            "pilot_eligible": self.pilot_eligible,
            "import_eligible": self.import_eligible,
            "graph_writes_allowed": self.graph_writes_allowed,
            "chunk_count": self.chunk_count,
            "evidence_count": self.evidence_count,
            "blockers": list(self.blockers),
            "stages": [stage.to_dict() for stage in self.stages],
        }


@dataclass(frozen=True, slots=True)
class GraphDataReadinessPackage:
    """Metadata-only readiness package for graph-data preparation (not import auth)."""

    sources: tuple[PerSourceReadiness, ...]
    continuity: ContinuityAudit
    verdict: ReadinessVerdict
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    production_import_attempted: bool = False
    falkor_touched: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed or self.production_import_attempted:
            raise ValueError("readiness package cannot authorize import or writes")
        if self.falkor_touched:
            raise ValueError("M209 readiness package must not touch FalkorDB")

    def to_dict(self) -> dict[str, Any]:
        payload = {
            "sources": [source.to_dict() for source in self.sources],
            "continuity": self.continuity.to_dict(),
            "verdict": self.verdict,
            "import_eligible": self.import_eligible,
            "graph_writes_allowed": self.graph_writes_allowed,
            "production_import_attempted": self.production_import_attempted,
            "falkor_touched": self.falkor_touched,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
            "counts": {
                "source_count": len(self.sources),
                "structure_ok": sum(1 for source in self.sources if source.structure_ok),
                "pilot_eligible": sum(1 for source in self.sources if source.pilot_eligible),
                "blocked": sum(1 for source in self.sources if source.blockers),
            },
        }
        text = str(payload).lower()
        for forbidden in ("api_key", "password", "embedding", "raw_text", "sk-"):
            if forbidden in text:
                raise ValueError(f"readiness package leaked forbidden token: {forbidden}")
        return payload


@dataclass(frozen=True, slots=True)
class GraphDataReadinessRequest:
    sources: tuple[SourceInput, ...]
    review_completed: bool = True
    repo_root: str = "."
    require_min_chunks: int = 1


@dataclass(frozen=True, slots=True)
class GraphDataReadinessResult:
    package: GraphDataReadinessPackage
    continuity_report_markdown: str
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        self.package.safety_flags.assert_no_write()


def _candidate_packet(bundle: StructuredSourceBundle) -> CandidatePacket:
    nodes = tuple(f"node:section:{chunk_id}" for chunk_id in bundle.chunk_ids[:4]) or (
        f"node:paper:{bundle.paper_id}",
    )
    edges = ()
    if len(nodes) >= 2:
        edges = (f"edge:{nodes[0]}->{nodes[1]}",)
    evidence = bundle.evidence_ids[:4] or (f"artifact:{bundle.paper_id}",)
    return CandidatePacket(
        candidate_id=f"candidate:{bundle.paper_id}:{bundle.source_kind}",
        evidence_refs=evidence,
        candidate_type="graph_candidate",
        schema_version="universal-kb-candidate.v1",
        graph_node_refs=nodes,
        graph_edge_refs=edges,
        provenance_refs=(
            f"source:{bundle.source_kind}:{bundle.paper_id}",
            f"source_kind:{bundle.source_kind}",
        ),
    )


def _run_one_source(
    source: SourceInput,
    *,
    review_completed: bool,
    require_min_chunks: int,
) -> PerSourceReadiness:
    stages: list[StageResult] = []
    blockers: list[str] = []
    path = Path(source.path)
    load = load_article_source(path, source_type=source.source_type, paper_id=source.paper_id)
    load_ok = load.outcome == "loaded" and bool(load.text)
    stages.append(
        StageResult(
            stage="load",
            status="done" if load_ok else "failed",
            diagnostics=(f"outcome:{load.outcome}", f"source_type:{load.source_type}"),
        )
    )
    if not load_ok:
        blockers.append(f"load:{load.failure_reason or load.outcome}")
        return PerSourceReadiness(
            paper_id=source.paper_id,
            source_kind=str(load.source_type or "unknown"),
            load_ok=False,
            structure_ok=False,
            candidate_ok=False,
            projection_ok=False,
            pilot_eligible=False,
            blockers=tuple(blockers),
            stages=tuple(stages),
        )

    try:
        bundle = structure_loaded_source(load, paper_id=source.paper_id)
        structure_ok = bundle.chunk_count >= require_min_chunks and bundle.evidence_count >= 1
        stages.append(
            StageResult(
                stage="structure",
                status="done" if structure_ok else "failed",
                diagnostics=(
                    f"chunks:{bundle.chunk_count}",
                    f"evidence:{bundle.evidence_count}",
                    f"source_kind:{bundle.source_kind}",
                ),
            )
        )
        if not structure_ok:
            blockers.append("structure:insufficient_chunks_or_evidence")
    except Exception as exc:  # noqa: BLE001 - stage failure is data
        stages.append(
            StageResult(
                stage="structure",
                status="failed",
                diagnostics=(f"error:{type(exc).__name__}",),
            )
        )
        return PerSourceReadiness(
            paper_id=source.paper_id,
            source_kind=str(load.source_type or "unknown"),
            load_ok=True,
            structure_ok=False,
            candidate_ok=False,
            projection_ok=False,
            pilot_eligible=False,
            blockers=("structure:exception",),
            stages=tuple(stages),
        )

    candidate = statistical_candidates_from_bundle(bundle)
    prov = validate_source_kind_provenance(candidate.to_dict())
    candidate_ok = bool(candidate.evidence_path_ids) and prov.accepted
    stages.append(
        StageResult(
            stage="candidate",
            status="done" if candidate_ok else "failed",
            diagnostics=(
                f"source_kind:{candidate.source_kind}",
                f"entities:{len(candidate.entity_labels)}",
                f"prov:{prov.accepted}",
            ),
        )
    )
    if not candidate_ok:
        blockers.append("candidate:missing_evidence_or_source_kind")

    packet = _candidate_packet(bundle)
    request = ProjectionRequest(candidate_packet=packet)
    nx_result = NetworkXProjectionAdapter().project(request)
    fk_result = DisabledFalkorProjectionAdapter(dry_run=True).project(request)
    plan = build_falkor_operation_plan(request)
    projection_ok = bool(nx_result.node_refs) and bool(plan.plan_fingerprint)
    stages.append(
        StageResult(
            stage="projection",
            status="done" if projection_ok else "failed",
            diagnostics=(
                f"nx_nodes:{len(nx_result.node_refs)}",
                f"fk_backend:{fk_result.backend}",
                f"plan:{plan.plan_fingerprint}",
            ),
        )
    )
    if not projection_ok:
        blockers.append("projection:empty_or_no_plan")

    schema = observe_schema_gate(GraphProjectionSchemaGate().validate(request))
    gaps = trace_promotion_boundary_gaps(
        candidate_id=packet.candidate_id,
        review=observe_review_post_check(completed=review_completed),
        import_boundary=observe_import_boundary(valid_rehearsal=True, accepted_count=0),
        schema=schema,
        handoff=observe_readiness_handoff(readiness_state="diagnostics_only"),
    )
    decision: PilotEligibilityDecision = decide_pilot_eligibility(gaps)
    stages.append(
        StageResult(
            stage="promotion",
            status="done" if decision.pilot_eligible else "failed",
            diagnostics=(
                f"decision:{decision.decision}",
                f"import_eligible:{decision.import_eligible}",
            ),
        )
    )
    if not decision.pilot_eligible:
        blockers.append("promotion:not_pilot_eligible")

    # package stage marker
    stages.append(StageResult(stage="package", status="done", diagnostics=("per_source_packaged",)))

    return PerSourceReadiness(
        paper_id=source.paper_id,
        source_kind=bundle.source_kind,
        load_ok=True,
        structure_ok=structure_ok,
        candidate_ok=candidate_ok,
        projection_ok=projection_ok,
        pilot_eligible=decision.pilot_eligible,
        import_eligible=False,
        graph_writes_allowed=False,
        chunk_count=bundle.chunk_count,
        evidence_count=bundle.evidence_count,
        blockers=tuple(blockers),
        stages=tuple(stages),
    )


def _package_verdict(sources: Sequence[PerSourceReadiness]) -> ReadinessVerdict:
    if not sources:
        return "blocked"
    if all(source.pilot_eligible and source.structure_ok for source in sources):
        return "ready_for_review"
    if any(source.structure_ok for source in sources):
        return "repair"
    return "blocked"


def run_graph_data_readiness_pipeline(
    request: GraphDataReadinessRequest,
) -> GraphDataReadinessResult:
    """Run no-write composition root for graph-data readiness (no Falkor)."""
    continuity = build_continuity_audit(repo_root=request.repo_root)
    per_source = tuple(
        _run_one_source(
            source,
            review_completed=request.review_completed,
            require_min_chunks=request.require_min_chunks,
        )
        for source in request.sources
    )
    verdict = _package_verdict(per_source)
    package = GraphDataReadinessPackage(
        sources=per_source,
        continuity=continuity,
        verdict=verdict,
        diagnostics=(
            "graph_data_readiness_no_write",
            "falkor_deferred_by_policy",
            f"verdict:{verdict}",
            f"sources:{len(per_source)}",
        ),
    )
    return GraphDataReadinessResult(
        package=package,
        continuity_report_markdown=render_continuity_report(continuity),
    )


__all__ = [
    "GraphDataReadinessPackage",
    "GraphDataReadinessRequest",
    "GraphDataReadinessResult",
    "PerSourceReadiness",
    "ReadinessVerdict",
    "SourceInput",
    "StageName",
    "StageResult",
    "StageStatus",
    "run_graph_data_readiness_pipeline",
]
