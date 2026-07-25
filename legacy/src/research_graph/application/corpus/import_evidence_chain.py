"""Import evidence chain (M284 S04) — assemble readiness, never write.

Composes evidence_dashboard + prediction resolvability + structure + import_hold
+ promotion posture into a single operator-facing chain. D127: user_go alone
never flips import_eligible. Graph write requires explicit separate go after
the chain is green.

Never import. Never GraphDBPort. Never DSPy.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

SCHEMA_VERSION = "import-evidence-chain.v1"


@dataclass(frozen=True, slots=True)
class ImportEvidenceChainPackage:
    schema_version: str
    evidence_ready_ok: bool
    verification_ready_ok: bool
    prediction_resolvability_rate: float | None
    page_or_bbox_count: int
    char_only_count: int
    structure_signal: str | None
    weak_structure_ir: bool
    import_hold_verdict: str | None
    import_hold_hits: int
    e5_header_entities: int
    user_go: bool
    chain_green: bool
    import_eligible: bool
    graph_write_allowed: bool
    blockers: tuple[str, ...]
    alerts: tuple[str, ...]
    diagnostics: tuple[str, ...]
    seams: tuple[dict[str, Any], ...]

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_write_allowed:
            raise ValueError(
                "import evidence chain cannot authorize import/writes "
                "(D127: user_go alone never flips import_eligible)"
            )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "evidence_ready_ok": self.evidence_ready_ok,
            "verification_ready_ok": self.verification_ready_ok,
            "prediction_resolvability_rate": self.prediction_resolvability_rate,
            "page_or_bbox_count": self.page_or_bbox_count,
            "char_only_count": self.char_only_count,
            "structure_signal": self.structure_signal,
            "weak_structure_ir": self.weak_structure_ir,
            "import_hold_verdict": self.import_hold_verdict,
            "import_hold_hits": self.import_hold_hits,
            "e5_header_entities": self.e5_header_entities,
            "user_go": self.user_go,
            "chain_green": self.chain_green,
            "import_eligible": False,
            "graph_write_allowed": False,
            "blockers": list(self.blockers),
            "alerts": list(self.alerts),
            "diagnostics": list(self.diagnostics),
            "seams": list(self.seams),
            "note": (
                "Import evidence chain: engineering readiness only. "
                "import_eligible always false here. Graph write requires "
                "chain_green + explicit user yes after this package (D127). "
                "user_go alone never flips import_eligible."
            ),
        }


def build_import_evidence_chain(
    *,
    evidence_dashboard: Mapping[str, Any] | None = None,
    prediction_resolvability: Mapping[str, Any] | None = None,
    structure_readiness: Mapping[str, Any] | None = None,
    import_hold: Mapping[str, Any] | None = None,
    e5_optional: Mapping[str, Any] | None = None,
    user_go: bool = False,
    prediction_target_rate: float = 0.70,
) -> ImportEvidenceChainPackage:
    """Compose import evidence chain (fail-closed; never sets import_eligible)."""
    dash = dict(evidence_dashboard or {})
    pred = dict(prediction_resolvability or {})
    struct = dict(structure_readiness or {})
    hold = dict(import_hold or {})
    e5 = dict(e5_optional or {})

    evidence_ready_ok = bool(dash.get("evidence_ready_ok"))
    page_bbox = int(dash.get("page_or_bbox_count") or pred.get("page_or_bbox_count") or 0)
    char_only = int(dash.get("char_only_count") or pred.get("char_only_count") or 0)
    weak_ir = bool(dash.get("weak_structure_ir") or struct.get("weak_structure_ir"))
    structure_signal = (
        dash.get("structure_signal")
        or struct.get("structure_signal")
        or struct.get("signal")
    )
    if structure_signal is not None:
        structure_signal = str(structure_signal)

    try:
        pred_rate = (
            float(pred["resolvability_rate"])
            if pred.get("resolvability_rate") is not None
            else None
        )
    except (TypeError, ValueError):
        pred_rate = None

    hold_verdict = hold.get("verdict")
    if hold_verdict is not None:
        hold_verdict = str(hold_verdict)
    hits = hold.get("enablement_hits", 0)
    try:
        hits_n = int(hits) if not isinstance(hits, list) else len(hits)
    except (TypeError, ValueError):
        hits_n = 0

    e5_header = int(
        e5.get("header_entities_total")
        or (e5.get("coverage_delta") or {}).get("header_entity_count")
        or 0
    )

    # Verification-ready: prediction resolvability present + rate above floor
    # (not gold F1 — prediction traceability). Held-out isolation assumed.
    verification_ready_ok = (
        pred_rate is not None
        and pred_rate >= float(prediction_target_rate)
        and page_bbox > 0
        and not weak_ir
    )

    blockers: list[str] = []
    if not evidence_ready_ok:
        blockers.append("evidence_not_ready")
        for b in dash.get("evidence_ready_blockers") or []:
            blockers.append(f"evidence:{b}")
    if pred_rate is None:
        blockers.append("prediction_resolvability_missing")
    elif pred_rate < float(prediction_target_rate):
        blockers.append(
            f"prediction_resolvability_below_floor:{pred_rate}<{prediction_target_rate}"
        )
    if page_bbox <= 0:
        blockers.append("page_or_bbox_count_zero")
    if weak_ir:
        blockers.append("weak_structure_ir")
    if hold_verdict not in (None, "pass", "ok", True) or hits_n > 0:
        blockers.append(f"import_hold:{hold_verdict or 'hits'}:{hits_n}")
    if not user_go:
        blockers.append("user_go_required_for_graph_write")

    # chain_green = engineering chain ready for human decision (user_go still separate)
    engineering_blockers = [b for b in blockers if b != "user_go_required_for_graph_write"]
    chain_green = len(engineering_blockers) == 0

    alerts: list[str] = []
    for a in dash.get("alerts") or []:
        alerts.append(f"evidence:{a}")
    for a in pred.get("alerts") or []:
        alerts.append(f"prediction:{a}")
    for a in e5.get("alerts") or []:
        alerts.append(f"e5:{a}")
    if user_go and not chain_green:
        alerts.append("user_go_true_but_chain_not_green")
    if user_go:
        # Explicit: user_go does NOT flip import_eligible
        alerts.append("user_go_does_not_flip_import_eligible_d127")

    seams = (
        {
            "name": "evidence_dashboard",
            "passed": evidence_ready_ok,
            "page_or_bbox_count": page_bbox,
            "char_only_count": char_only,
        },
        {
            "name": "prediction_resolvability",
            "passed": pred_rate is not None
            and pred_rate >= float(prediction_target_rate),
            "rate": pred_rate,
            "target_floor": prediction_target_rate,
        },
        {
            "name": "structure_readiness",
            "passed": not weak_ir,
            "structure_signal": structure_signal,
            "weak_structure_ir": weak_ir,
        },
        {
            "name": "import_hold",
            "passed": hold_verdict in (None, "pass", "ok", True) and hits_n == 0,
            "verdict": hold_verdict,
            "enablement_hits": hits_n,
        },
        {
            "name": "e5_optional",
            "passed": True,  # optional — never blocks import chain alone
            "header_entities": e5_header,
            "optional": True,
        },
        {
            "name": "user_go",
            "passed": bool(user_go),
            "note": "required for graph write; never alone flips import_eligible",
        },
        {
            "name": "import_eligible_lock",
            "passed": True,
            "import_eligible": False,
            "note": "always false in this package (D127)",
        },
    )

    diagnostics = (
        f"evidence_ready_ok:{str(evidence_ready_ok).lower()}",
        f"verification_ready_ok:{str(verification_ready_ok).lower()}",
        f"prediction_resolvability_rate:{pred_rate}",
        f"page_or_bbox_count:{page_bbox}",
        f"char_only_count:{char_only}",
        f"weak_structure_ir:{str(weak_ir).lower()}",
        f"import_hold_verdict:{hold_verdict}",
        f"import_hold_hits:{hits_n}",
        f"e5_header_entities:{e5_header}",
        f"user_go:{str(bool(user_go)).lower()}",
        f"chain_green:{str(chain_green).lower()}",
        f"blockers:{len(blockers)}",
        "import_eligible:false",
        "graph_write_allowed:false",
        "d127_user_go_alone_never_import",
    )
    return ImportEvidenceChainPackage(
        schema_version=SCHEMA_VERSION,
        evidence_ready_ok=evidence_ready_ok,
        verification_ready_ok=verification_ready_ok,
        prediction_resolvability_rate=pred_rate,
        page_or_bbox_count=page_bbox,
        char_only_count=char_only,
        structure_signal=structure_signal,
        weak_structure_ir=weak_ir,
        import_hold_verdict=hold_verdict,
        import_hold_hits=hits_n,
        e5_header_entities=e5_header,
        user_go=bool(user_go),
        chain_green=chain_green,
        import_eligible=False,
        graph_write_allowed=False,
        blockers=tuple(blockers),
        alerts=tuple(alerts),
        diagnostics=diagnostics,
        seams=seams,
    )


__all__ = [
    "SCHEMA_VERSION",
    "ImportEvidenceChainPackage",
    "build_import_evidence_chain",
]
