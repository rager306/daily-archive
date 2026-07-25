"""Evidence-ready governor dashboard fields (M283).

Pure composition of resolvability + structure gate signals for operators.
Never authorizes import. Distinguishes demo vs real metrics and blocks
vanity evidence-ready when only char-only spans exist without explicit OK.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True, slots=True)
class EvidenceDashboardPackage:
    schema_version: str
    metric_mode: str
    demo_metric: bool
    resolvability_rate: float | None
    target_rate: float
    target_met: bool
    page_or_bbox_count: int
    char_only_count: int
    total_rows: int
    relation_grounded_ratio: float | None
    weak_structure_ir: bool
    ir_hard_count: int | None
    structure_signal: str | None
    evidence_ready_ok: bool
    evidence_ready_blockers: tuple[str, ...]
    alerts: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "metric_mode": self.metric_mode,
            "demo_metric": self.demo_metric,
            "resolvability_rate": self.resolvability_rate,
            "target_rate": self.target_rate,
            "target_met": self.target_met,
            "page_or_bbox_count": self.page_or_bbox_count,
            "char_only_count": self.char_only_count,
            "total_rows": self.total_rows,
            "relation_grounded_ratio": self.relation_grounded_ratio,
            "weak_structure_ir": self.weak_structure_ir,
            "ir_hard_count": self.ir_hard_count,
            "structure_signal": self.structure_signal,
            "evidence_ready_ok": self.evidence_ready_ok,
            "evidence_ready_blockers": list(self.evidence_ready_blockers),
            "alerts": list(self.alerts),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Governor dashboard: metric_mode + page_or_bbox_count always shown. "
                "evidence_ready_ok false when demo, target not met, char_only without "
                "page/bbox (unless allow_char_only_ok), or weak_structure_ir. Never import."
            ),
        }


def build_evidence_dashboard(
    *,
    resolvability: Mapping[str, Any] | None = None,
    structure_readiness: Mapping[str, Any] | None = None,
    chunk_quality_gate: Mapping[str, Any] | None = None,
    allow_char_only_ok: bool = False,
    target_rate: float = 0.95,
) -> EvidenceDashboardPackage:
    """Compose operator-facing evidence readiness (fail-closed)."""
    r = dict(resolvability or {})
    s = dict(structure_readiness or {})
    g = dict(chunk_quality_gate or {})

    mode = str(r.get("metric_mode") or "unspecified")
    demo = bool(r.get("demo_metric"))
    rate = r.get("resolvability_rate")
    try:
        rate_f = float(rate) if rate is not None else None
    except (TypeError, ValueError):
        rate_f = None
    target = float(r.get("target_rate") or target_rate)
    target_met = bool(r.get("target_met"))
    page_bbox = int(r.get("page_or_bbox_count") or 0)
    char_only = int(r.get("char_only_count") or 0)
    total = int(r.get("total_rows") or 0)
    rel_ratio = r.get("relation_grounded_ratio")
    try:
        rel_f = float(rel_ratio) if rel_ratio is not None else None
    except (TypeError, ValueError):
        rel_f = None

    weak_ir = bool(s.get("weak_structure_ir"))
    if not weak_ir and g.get("gate_signal") == "pass" and int(g.get("ir_hard_count") or -1) == 0:
        weak_ir = True
    ir_hard = s.get("ir_hard_count")
    if ir_hard is None:
        ir_hard = g.get("ir_hard_count")
    try:
        ir_hard_i = int(ir_hard) if ir_hard is not None else None
    except (TypeError, ValueError):
        ir_hard_i = None
    structure_signal = s.get("structure_signal")

    alerts: list[str] = []
    for a in r.get("alerts") or ():
        alerts.append(str(a))
    for a in s.get("alerts") or ():
        alerts.append(str(a))
    if weak_ir and "weak_structure_ir" not in " ".join(alerts):
        alerts.append("weak_structure_ir:hard_visible")

    blockers: list[str] = []
    if demo:
        blockers.append("demo_metric")
    if not target_met:
        blockers.append("target_not_met")
    if page_bbox == 0 and char_only > 0 and not allow_char_only_ok:
        blockers.append("char_only_no_page_bbox")
    if weak_ir:
        blockers.append("weak_structure_ir")
    if total <= 0:
        blockers.append("no_rows")

    evidence_ready_ok = len(blockers) == 0
    diagnostics = (
        f"metric_mode:{mode}",
        f"demo_metric:{str(demo).lower()}",
        f"resolvability_rate:{rate_f}",
        f"page_or_bbox_count:{page_bbox}",
        f"char_only_count:{char_only}",
        f"target_met:{str(target_met).lower()}",
        f"weak_structure_ir:{str(weak_ir).lower()}",
        f"ir_hard_count:{ir_hard_i}",
        f"structure_signal:{structure_signal}",
        f"evidence_ready_ok:{str(evidence_ready_ok).lower()}",
        f"blockers:{len(blockers)}",
        "import_write_fail_closed",
    )
    return EvidenceDashboardPackage(
        schema_version="evidence-dashboard.v1",
        metric_mode=mode,
        demo_metric=demo,
        resolvability_rate=rate_f,
        target_rate=target,
        target_met=target_met,
        page_or_bbox_count=page_bbox,
        char_only_count=char_only,
        total_rows=total,
        relation_grounded_ratio=rel_f,
        weak_structure_ir=weak_ir,
        ir_hard_count=ir_hard_i,
        structure_signal=str(structure_signal) if structure_signal is not None else None,
        evidence_ready_ok=evidence_ready_ok,
        evidence_ready_blockers=tuple(blockers),
        alerts=tuple(alerts),
        diagnostics=diagnostics,
    )


__all__ = [
    "EvidenceDashboardPackage",
    "build_evidence_dashboard",
]
