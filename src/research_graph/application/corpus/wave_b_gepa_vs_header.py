"""Same-n header vs GEPA-instruction constrained select comparison (M268).

Pure scoring helpers. Never invents free labels. Never authorizes import.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.wave_b_constrained_select import (
    header_priority_select,
)
from research_graph.application.corpus.wave_b_gepa_constrained_spike import (
    COMPONENT_ENTITY,
    COMPONENT_RELATION,
    make_select_fn_from_candidate,
)
from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    score_gold_hybrid_constrained_pilot,
)

SCHEMA_VERSION = "wave-b-gepa-vs-header.v1"
DEFAULT_MAX_VAL_GAP = 0.35


@dataclass(frozen=True, slots=True)
class GepaVsHeaderPackage:
    schema_version: str
    joined_count: int
    header: dict[str, Any]
    gepa: dict[str, Any]
    delta_vs_header: dict[str, Any]
    promote_ready: bool
    promote_blockers: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("gepa-vs-header cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "joined_count": self.joined_count,
            "header": dict(self.header),
            "gepa": dict(self.gepa),
            "delta_vs_header": dict(self.delta_vs_header),
            "promote_ready": self.promote_ready,
            "promote_blockers": list(self.promote_blockers),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Same-n comparison of header_priority_select vs GEPA instruction "
                "rule select (candidate_id only). Promote requires dual F1 win and "
                "val-gap guard (D128). Never import."
            ),
        }


def candidate_from_gepa_artifact(payload: Mapping[str, Any]) -> dict[str, str]:
    """Extract GEPA instruction candidate from spike artifact or best_candidate."""
    best = payload.get("best_candidate")
    if isinstance(best, Mapping):
        ent = str(best.get(COMPONENT_ENTITY) or best.get("entity_select_instruction") or "")
        rel = str(best.get(COMPONENT_RELATION) or best.get("relation_link_instruction") or "")
        if ent or rel:
            return {COMPONENT_ENTITY: ent, COMPONENT_RELATION: rel}
    # flat keys
    ent = str(payload.get(COMPONENT_ENTITY) or payload.get("entity_select_instruction") or "")
    rel = str(payload.get(COMPONENT_RELATION) or payload.get("relation_link_instruction") or "")
    return {COMPONENT_ENTITY: ent, COMPONENT_RELATION: rel}


def evaluate_val_gap_guard(
    *,
    train_entity_f1: float | None,
    val_entity_f1: float | None,
    max_val_gap: float = DEFAULT_MAX_VAL_GAP,
) -> tuple[bool, str | None]:
    """Return (ok, blocker). ok=False when train-val gap too large."""
    if train_entity_f1 is None or val_entity_f1 is None:
        return True, None  # no split signal → do not block solely on missing val
    gap = float(train_entity_f1) - float(val_entity_f1)
    if gap > float(max_val_gap):
        return False, f"val_gap:{gap:.4f}>{float(max_val_gap):.4f}"
    return True, None


def compare_header_vs_gepa_instruction(
    *,
    cases: Sequence[Mapping[str, Any]],
    gepa_candidate: Mapping[str, Any],
    floor_metrics: Mapping[str, Any] | None = None,
    max_val_gap: float = DEFAULT_MAX_VAL_GAP,
) -> GepaVsHeaderPackage:
    """Score header_priority_select and GEPA instruction select on the same cases."""
    cases_list = list(cases)
    n = len(cases_list)
    header_pkg = score_gold_hybrid_constrained_pilot(
        cases=cases_list,
        select_fn=header_priority_select,
        floor_metrics=floor_metrics,
        model_id="header_priority_select",
    )
    instr_only = {
        COMPONENT_ENTITY: str(gepa_candidate.get(COMPONENT_ENTITY) or ""),
        COMPONENT_RELATION: str(gepa_candidate.get(COMPONENT_RELATION) or ""),
    }
    gepa_fn = make_select_fn_from_candidate(instr_only)
    gepa_pkg = score_gold_hybrid_constrained_pilot(
        cases=cases_list,
        select_fn=gepa_fn,
        floor_metrics=floor_metrics,
        model_id="gepa_instruction_rule_select",
    )
    h = header_pkg.to_dict()
    g = gepa_pkg.to_dict()
    h_metrics = dict(h.get("metrics") or {})
    g_metrics = dict(g.get("metrics") or {})
    # attach train/val from GEPA best_metrics if present on candidate payload
    he = float(h_metrics.get("entity_f1") or 0.0)
    hr = float(h_metrics.get("relation_f1") or 0.0)
    ge = float(g_metrics.get("entity_f1") or 0.0)
    gr = float(g_metrics.get("relation_f1") or 0.0)
    de = round(ge - he, 6)
    dr = round(gr - hr, 6)

    train_e = g_metrics.get("train_entity_f1")
    val_e = g_metrics.get("val_entity_f1")
    # pilot package may not split — allow optional keys from gepa spike best_metrics merge
    if train_e is None and isinstance(gepa_candidate.get("train_entity_f1"), (int, float)):
        train_e = gepa_candidate.get("train_entity_f1")
    if val_e is None and isinstance(gepa_candidate.get("val_entity_f1"), (int, float)):
        val_e = gepa_candidate.get("val_entity_f1")

    blockers: list[str] = []
    if de <= 0:
        blockers.append(f"entity_delta_not_positive:{de}")
    if dr <= 0:
        blockers.append(f"relation_delta_not_positive:{dr}")
    gap_ok, gap_blocker = evaluate_val_gap_guard(
        train_entity_f1=float(train_e) if train_e is not None else None,
        val_entity_f1=float(val_e) if val_e is not None else None,
        max_val_gap=max_val_gap,
    )
    if not gap_ok and gap_blocker:
        blockers.append(gap_blocker)

    promote = len(blockers) == 0
    header_view = {
        "entity_f1": he,
        "relation_f1": hr,
        "model_id": "header_priority_select",
        "metrics": h_metrics,
    }
    gepa_view = {
        "entity_f1": ge,
        "relation_f1": gr,
        "model_id": "gepa_instruction_rule_select",
        "metrics": g_metrics,
        "train_entity_f1": train_e,
        "val_entity_f1": val_e,
        "entity_select_instruction": str(gepa_candidate.get(COMPONENT_ENTITY) or "")[:500],
        "relation_link_instruction": str(gepa_candidate.get(COMPONENT_RELATION) or "")[:500],
    }
    diagnostics = (
        f"joined_count:{n}",
        f"header_entity_f1:{he}",
        f"header_relation_f1:{hr}",
        f"gepa_entity_f1:{ge}",
        f"gepa_relation_f1:{gr}",
        f"delta_entity:{de}",
        f"delta_relation:{dr}",
        f"promote_ready:{promote}",
        f"blockers:{len(blockers)}",
        "import_write_fail_closed",
        "wave_b_gepa_vs_header_only",
    )
    return GepaVsHeaderPackage(
        schema_version=SCHEMA_VERSION,
        joined_count=n,
        header=header_view,
        gepa=gepa_view,
        delta_vs_header={"entity_f1": de, "relation_f1": dr},
        promote_ready=promote,
        promote_blockers=tuple(blockers),
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "DEFAULT_MAX_VAL_GAP",
    "GepaVsHeaderPackage",
    "candidate_from_gepa_artifact",
    "compare_header_vs_gepa_instruction",
    "evaluate_val_gap_guard",
]
