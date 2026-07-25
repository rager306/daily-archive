"""Wave C structure readiness package (M262).

Composes structure-layer continuity seams, ETL hybrid/closeout context, and
optional citation review verdict into one import-blocked readiness surface.
Never authorizes import or graph writes.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any, Literal

SCHEMA_VERSION = "structure-readiness.v1"

StructureSignal = Literal["blocked", "partial", "ready_for_structure_review"]


@dataclass(frozen=True, slots=True)
class StructureReadinessPackage:
    schema_version: str
    structure_signal: StructureSignal
    structure_layer_health: str
    structure_present_seams: tuple[str, ...]
    structure_missing_seams: tuple[str, ...]
    structure_gaps: tuple[str, ...]
    hybrid_found: int | None
    hybrid_fraction: float | None
    closeout_signal: str | None
    citation_verdict: str | None
    pipeline_overall: str | None
    diagnostics: tuple[str, ...]
    alerts: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    # M282: structure gate v2 IR signal
    ir_hard_count: int | None = None
    newline_demoted_count: int | None = None
    weak_structure_ir: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("structure readiness cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "structure_signal": self.structure_signal,
            "structure_layer_health": self.structure_layer_health,
            "structure_present_seams": list(self.structure_present_seams),
            "structure_missing_seams": list(self.structure_missing_seams),
            "structure_gaps": list(self.structure_gaps),
            "hybrid_found": self.hybrid_found,
            "hybrid_fraction": self.hybrid_fraction,
            "ir_hard_count": self.ir_hard_count,
            "newline_demoted_count": self.newline_demoted_count,
            "weak_structure_ir": self.weak_structure_ir,
            "closeout_signal": self.closeout_signal,
            "citation_verdict": self.citation_verdict,
            "pipeline_overall": self.pipeline_overall,
            "diagnostics": list(self.diagnostics),
            "alerts": list(self.alerts),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave C structure readiness only. Bridges M209 structure layer "
                "with ETL hybrid/closeout context. Citation verdict is optional "
                "context. Never import authorization."
            ),
        }


def build_structure_readiness_package(
    *,
    structure_layer: Mapping[str, Any] | None = None,
    pipeline_overall: str | None = None,
    hybrid_found: int | None = None,
    hybrid_fraction: float | None = None,
    closeout_signal: str | None = None,
    citation_verdict: str | None = None,
    etl_dashboard: Mapping[str, Any] | None = None,
    chunk_quality_gate: Mapping[str, Any] | None = None,
) -> StructureReadinessPackage:
    """Compose structure readiness from injected layer/ETL/citation context (pure)."""
    layer = dict(structure_layer or {})
    dash = dict(etl_dashboard or {})

    health = str(layer.get("health") or "gap")
    present = tuple(str(x) for x in (layer.get("present_seams") or ()))
    missing = tuple(str(x) for x in (layer.get("missing_seams") or ()))
    gaps_list = [str(x) for x in (layer.get("gaps") or ())]
    gate = dict(chunk_quality_gate or {})
    if gate.get("continuity_gap_cleared") is True:
        gaps_list = [
            g
            for g in gaps_list
            if g != "real_corpus_chunk_quality_not_continuously_gated"
        ]
    elif gate and gate.get("gate_signal") in {"partial", "blocked", "pass"}:
        # keep gap if gate ran but did not clear
        if (
            "real_corpus_chunk_quality_not_continuously_gated" not in gaps_list
            and gate.get("continuity_gap_cleared") is False
        ):
            # do not re-add if layer already dropped it
            pass
    gaps = tuple(gaps_list)

    if hybrid_found is None and dash.get("hybrid_found") is not None:
        hybrid_found = int(dash["hybrid_found"])
    if hybrid_fraction is None and dash.get("hybrid_fraction") is not None:
        hybrid_fraction = float(dash["hybrid_fraction"])
    if closeout_signal is None and dash.get("closeout_signal") is not None:
        closeout_signal = str(dash["closeout_signal"])

    ir_hard = gate.get("ir_hard_count")
    try:
        ir_hard_i = int(ir_hard) if ir_hard is not None else None
    except (TypeError, ValueError):
        ir_hard_i = None
    nl_demoted = gate.get("newline_demoted_count")
    try:
        nl_i = int(nl_demoted) if nl_demoted is not None else None
    except (TypeError, ValueError):
        nl_i = None
    # Weak IR: gate may pass on soft_legacy newlines with zero hard IR structure.
    weak_ir = bool(
        gate.get("gate_signal") == "pass"
        and ir_hard_i is not None
        and ir_hard_i == 0
    )

    alerts: list[str] = []
    if missing:
        alerts.append(f"structure_missing_seams:{len(missing)}")
    if gaps:
        alerts.append(f"structure_gaps:{len(gaps)}")
    if hybrid_found is not None and hybrid_found <= 0:
        alerts.append("no_hybrid_bodies")
    if closeout_signal and closeout_signal != "wave_a_closed":
        alerts.append(f"wave_a_not_closed:{closeout_signal}")
    if citation_verdict in {"blocked", "repair"}:
        alerts.append(f"citation_verdict:{citation_verdict}")
    if weak_ir:
        alerts.append("weak_structure_ir:gate_pass_ir_hard_0")

    # Structure signal: seams blocked → blocked; missing/gaps/partial → partial;
    # present seams + hybrid context → ready_for_structure_review (still not import).
    # M282: weak IR demotes ready → partial (do not claim structure-ready on newlines).
    gap_cleared = bool(gate.get("continuity_gap_cleared"))
    if health == "blocked" or (not present and missing):
        signal: StructureSignal = "blocked"
    elif missing:
        signal = "partial"
    elif gaps and not gap_cleared:
        signal = "partial"
    elif health in {"gap", "partial"} and gaps:
        signal = "partial"
    elif weak_ir:
        signal = "partial"
    elif (
        (health in {"present", "partial", "gap"} or present)
        and (hybrid_found is None or hybrid_found > 0)
        and (not gaps or gap_cleared)
        and gate.get("gate_signal") == "pass"
    ):
        signal = "ready_for_structure_review"
    elif health == "present" and (hybrid_found is None or hybrid_found > 0) and not gaps:
        signal = "ready_for_structure_review"
    else:
        signal = "partial"

    diagnostics = (
        f"structure_signal:{signal}",
        f"structure_layer_health:{health}",
        f"present_seams:{len(present)}",
        f"missing_seams:{len(missing)}",
        f"gaps:{len(gaps)}",
        f"hybrid_found:{hybrid_found}",
        f"hybrid_fraction:{hybrid_fraction}",
        f"closeout_signal:{closeout_signal}",
        f"citation_verdict:{citation_verdict}",
        f"pipeline_overall:{pipeline_overall}",
        f"ir_hard_count:{ir_hard_i}",
        f"newline_demoted_count:{nl_i}",
        f"weak_structure_ir:{str(weak_ir).lower()}",
        f"alerts:{len(alerts)}",
        f"chunk_gate:{gate.get('gate_signal')}",
        f"chunk_gap_cleared:{gate.get('continuity_gap_cleared')}",
        "import_write_fail_closed",
        "wave_c_structure_readiness_only",
    )

    return StructureReadinessPackage(
        schema_version=SCHEMA_VERSION,
        structure_signal=signal,
        structure_layer_health=health,
        structure_present_seams=present,
        structure_missing_seams=missing,
        structure_gaps=gaps,
        hybrid_found=hybrid_found,
        hybrid_fraction=hybrid_fraction,
        closeout_signal=closeout_signal,
        citation_verdict=citation_verdict,
        pipeline_overall=pipeline_overall,
        diagnostics=diagnostics,
        alerts=tuple(alerts),
        ir_hard_count=ir_hard_i,
        newline_demoted_count=nl_i,
        weak_structure_ir=weak_ir,
    )


def extract_structure_layer(
    continuity_audit: Mapping[str, Any] | Any,
) -> dict[str, Any]:
    """Pull structure layer dict from ContinuityAudit or its to_dict() form."""
    if hasattr(continuity_audit, "to_dict"):
        data = continuity_audit.to_dict()
    elif isinstance(continuity_audit, Mapping):
        data = dict(continuity_audit)
    else:
        return {}
    layers = data.get("layers") or []
    for layer in layers:
        if isinstance(layer, Mapping) and layer.get("layer") == "structure":
            return dict(layer)
        if hasattr(layer, "layer") and getattr(layer, "layer", None) == "structure":
            return layer.to_dict() if hasattr(layer, "to_dict") else {}
    return {}


__all__ = [
    "SCHEMA_VERSION",
    "StructureSignal",
    "StructureReadinessPackage",
    "build_structure_readiness_package",
    "extract_structure_layer",
]
