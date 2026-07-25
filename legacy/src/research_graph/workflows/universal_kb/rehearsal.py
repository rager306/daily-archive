# Formerly: src/arxiv_archive/universal_kb_rehearsal.py

"""End-to-end no-write rehearsal for the M035 Universal KB prototype.

The rehearsal intentionally writes only local metadata artifacts. It does not
initialize a GraphDB, promote candidates, run production imports, persist raw
prompts, persist raw corpus text, or treat MiniMax output as source-of-truth.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.domain.graph_projection_schema import GraphProjectionSchemaGate
from research_graph.domain.ports import ProjectionRequest
from research_graph.infrastructure.graph.networkx_probe import NetworkXProjectionAdapter
from research_graph.infrastructure.llm.minimax_structured import DEFAULT_MINIMAX_MODEL
from research_graph.workflows.universal_kb.contracts import CandidatePacket
from research_graph.workflows.universal_kb.queue import UniversalKBQueue
from research_graph.workflows.universal_kb.review_assistance import (
    build_review_assistance_packet,
    build_review_tool_invocation_record,
)
from research_graph.workflows.universal_kb.sidecar_boundary import (
    candidate_packet_from_sidecar_json,
)
from research_graph.workflows.universal_kb.substrate_rehearsal import NoWriteSubstrateRehearsal

_REHEARSAL_INPUT_HASH = "sha256:m035-metadata-only-fixture"
_REVIEW_DIAGNOSTIC = "needs_locator"
_ARTIFACT_NAMES = (
    "candidate.json",
    "review_packet.json",
    "review_trace.json",
    "queue_inspect.json",
    "readiness_handoff.json",
    "schema_gate_result.json",
    "projection_result.json",
    "summary.json",
)


def _sidecar_fixture() -> dict[str, Any]:
    return {
        "candidate": {
            "id": "sidecar-candidate-1",
            "type": "section_summary",
            "evidence_refs": ["artifact:fixture-paper:section:1"],
        },
        "review": {"state": "pending"},
        "source": {"sidecar": "opendataloader-fixture", "version": "v1"},
    }


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _assert_clean_artifact_dir(artifact_dir: Path) -> None:
    artifact_dir.mkdir(parents=True, exist_ok=True)
    existing = [artifact_dir / name for name in _ARTIFACT_NAMES if (artifact_dir / name).exists()]
    if existing:
        raise FileExistsError(f"rehearsal artifact already exists: {existing[0]}")


def _projection_candidate(candidate: CandidatePacket) -> CandidatePacket:
    return CandidatePacket(
        candidate_id=candidate.candidate_id,
        evidence_refs=candidate.evidence_refs,
        candidate_type=candidate.candidate_type,
        review_state=candidate.review_state,
        schema_version=candidate.schema_version,
        graph_node_refs=("node:paper:sidecar-candidate-1", "node:claim:sidecar-candidate-1"),
        graph_edge_refs=("edge:paper:sidecar-candidate-1->claim:sidecar-candidate-1",),
        provenance_refs=("source:opendataloader-fixture:v1",),
        diagnostics=candidate.diagnostics,
    )


@dataclass(frozen=True, slots=True)
class RehearsalResult:
    """Inspectable result for one local no-write rehearsal run."""

    candidate_id: str
    queue_job_id: str
    model: str
    artifact_paths: tuple[Path, ...]
    graph_write_allowed: bool = False
    promotion_allowed: bool = False
    production_import_attempted: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "candidate_id": self.candidate_id,
            "queue_job_id": self.queue_job_id,
            "model": self.model,
            "artifact_paths": [str(path) for path in self.artifact_paths],
            "graph_write_allowed": self.graph_write_allowed,
            "promotion_allowed": self.promotion_allowed,
            "production_import_attempted": self.production_import_attempted,
        }


def run_universal_kb_no_write_rehearsal(artifact_dir: str | Path) -> RehearsalResult:
    """Run source-to-handoff rehearsal and persist metadata-only artifacts.

    The queue database is local to ``artifact_dir``. The returned artifacts are
    intentionally small JSON files suitable for future-agent inspection.
    """

    output_dir = Path(artifact_dir)
    _assert_clean_artifact_dir(output_dir)

    candidate = _projection_candidate(candidate_packet_from_sidecar_json(_sidecar_fixture()))
    candidate.assert_no_write()

    review_packet = build_review_assistance_packet(
        candidate=candidate,
        diagnostics=(_REVIEW_DIAGNOSTIC,),
        confidence=0.7,
        flags=("needs_human_review",),
    )
    review_trace = build_review_tool_invocation_record(
        invocation_id="s07-review-trace-1",
        model=DEFAULT_MINIMAX_MODEL,
        input_hash=_REHEARSAL_INPUT_HASH,
        review_packet=review_packet,
    )

    queue = UniversalKBQueue(output_dir / "queue.sqlite").initialize()
    try:
        queue.enqueue(
            job_id=candidate.candidate_id,
            stage="review_assistance",
            input_refs=candidate.evidence_refs,
            input_hash=_REHEARSAL_INPUT_HASH,
            tool_version=DEFAULT_MINIMAX_MODEL,
            contract_version=review_packet.schema_version,
            output_paths=("readiness_handoff.json",),
        )
        queue.unblock_ready_jobs()
        queue_inspect = queue.inspect(candidate.candidate_id)
        handoff = NoWriteSubstrateRehearsal(queue).build_handoff(
            candidate=candidate,
            review_trace=review_trace,
            queue_job_id=candidate.candidate_id,
        )
    finally:
        queue.close()

    candidate_path = output_dir / "candidate.json"
    review_packet_path = output_dir / "review_packet.json"
    review_trace_path = output_dir / "review_trace.json"
    queue_inspect_path = output_dir / "queue_inspect.json"
    handoff_path = output_dir / "readiness_handoff.json"
    schema_gate_path = output_dir / "schema_gate_result.json"
    projection_path = output_dir / "projection_result.json"
    summary_path = output_dir / "summary.json"

    projection_request = ProjectionRequest(candidate_packet=candidate)
    schema_gate_result = GraphProjectionSchemaGate().validate(projection_request)
    schema_gate_result.assert_no_write()
    projection_result = NetworkXProjectionAdapter().project(projection_request)
    projection_result.assert_no_write()

    _write_json(candidate_path, candidate.to_dict())
    _write_json(review_packet_path, review_packet.to_dict())
    _write_json(review_trace_path, review_trace.to_sanitized_dict())
    _write_json(queue_inspect_path, queue_inspect)
    handoff_payload = handoff.to_dict()
    _write_json(handoff_path, handoff_payload)
    schema_gate_payload = schema_gate_result.to_dict()
    _write_json(schema_gate_path, schema_gate_payload)
    projection_payload = projection_result.to_dict()
    _write_json(projection_path, projection_payload)

    artifact_paths = (
        candidate_path,
        review_packet_path,
        review_trace_path,
        queue_inspect_path,
        handoff_path,
        schema_gate_path,
        projection_path,
        summary_path,
    )
    summary = {
        "candidate_id": candidate.candidate_id,
        "queue_job_id": candidate.candidate_id,
        "queue_status": queue_inspect["job"]["status"],
        "model": DEFAULT_MINIMAX_MODEL,
        "artifact_count": len(artifact_paths) - 1,
        "dry_run_only": handoff_payload["dry_run_only"],
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "helper_evidence_only": True,
        "minimax_source_of_truth": False,
        "artifact_paths": [path.name for path in artifact_paths if path != summary_path],
        "schema_gate_accepted": schema_gate_payload["accepted"],
        "schema_gate_migration_required": schema_gate_payload["migration_required"],
        "schema_gate_diagnostics": schema_gate_payload["diagnostics"],
        "projection_backend": projection_payload["backend"],
        "projection_import_eligible": projection_payload["safety_flags"]["import_eligible"],
        "projection_diagnostics": [
            diagnostic["code"] for diagnostic in projection_payload["diagnostics"]
        ],
    }
    _write_json(summary_path, summary)

    return RehearsalResult(
        candidate_id=candidate.candidate_id,
        queue_job_id=candidate.candidate_id,
        model=DEFAULT_MINIMAX_MODEL,
        artifact_paths=artifact_paths,
    )
