"""Audit reviewed gold labels against hybrid body grounding.

Flags gold entity labels that are missing from the joined hybrid body window
(or from deterministic candidates). Application-pure; never import/writes.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    build_body_candidates,
    surface_in_body,
)
from research_graph.application.corpus.wave_b_gold_hybrid_llm_pilot import (
    truncate_body_for_pilot,
)

SCHEMA_VERSION = "wave-b-gold-body-grounding-audit.v1"


def _norm(value: str) -> str:
    return " ".join(value.casefold().strip().split())


@dataclass(frozen=True, slots=True)
class GoldBodyGroundingAuditPackage:
    schema_version: str
    case_count: int
    gold_entity_total: int
    grounded_in_body: int
    grounded_in_candidates: int
    body_coverage_ratio: float
    candidate_coverage_ratio: float
    ungrounded: tuple[dict[str, Any], ...]
    per_case: tuple[dict[str, Any], ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    dspy_optimizer_enabled: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("gold body grounding audit cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("gold body grounding audit cannot enable DSPy")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "case_count": self.case_count,
            "gold_entity_total": self.gold_entity_total,
            "grounded_in_body": self.grounded_in_body,
            "grounded_in_candidates": self.grounded_in_candidates,
            "body_coverage_ratio": self.body_coverage_ratio,
            "candidate_coverage_ratio": self.candidate_coverage_ratio,
            "ungrounded": list(self.ungrounded),
            "per_case": list(self.per_case),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "dspy_optimizer_enabled": False,
            "note": (
                "Audit only: gold labels must appear in hybrid body/candidates. "
                "Not import authority."
            ),
        }


def audit_gold_body_grounding(
    *,
    cases: Sequence[Mapping[str, Any]],
    max_body_chars: int = 8000,
) -> GoldBodyGroundingAuditPackage:
    """Audit each gold entity label for body + candidate grounding."""
    per_case: list[dict[str, Any]] = []
    ungrounded: list[dict[str, Any]] = []
    gold_total = 0
    in_body = 0
    in_cands = 0

    for case in cases:
        gold = dict(case.get("gold") or {})
        body = str(case.get("body_text") or "")
        case_id = str(case.get("case_id") or gold.get("case_id") or "unknown")
        paper_id = str(case.get("paper_id") or gold.get("paper_id") or "")
        window = truncate_body_for_pilot(body, max_chars=max_body_chars)
        candidates = build_body_candidates(window, paper_id=paper_id)
        cand_norms = {
            str(c.get("surface_norm") or "")
            for c in candidates
            if isinstance(c, Mapping)
        }
        case_rows: list[dict[str, Any]] = []
        case_body = 0
        case_cand = 0
        case_gold = 0
        for ent in gold.get("entities") or []:
            if not isinstance(ent, Mapping):
                continue
            label = str(ent.get("label") or "").strip()
            if not label:
                continue
            case_gold += 1
            gold_total += 1
            body_ok = surface_in_body(label, window)
            cand_ok = _norm(label) in cand_norms
            if body_ok:
                case_body += 1
                in_body += 1
            if cand_ok:
                case_cand += 1
                in_cands += 1
            row = {
                "label": label,
                "type": str(ent.get("type") or ""),
                "in_body": body_ok,
                "in_candidates": cand_ok,
            }
            case_rows.append(row)
            if not body_ok or not cand_ok:
                ungrounded.append(
                    {
                        "case_id": case_id,
                        "paper_id": paper_id,
                        **row,
                    }
                )
        per_case.append(
            {
                "case_id": case_id,
                "paper_id": paper_id,
                "gold_entity_count": case_gold,
                "grounded_in_body": case_body,
                "grounded_in_candidates": case_cand,
                "candidate_count": len(candidates),
                "entities": case_rows,
            }
        )

    body_ratio = (in_body / gold_total) if gold_total else 1.0
    cand_ratio = (in_cands / gold_total) if gold_total else 1.0
    diagnostics = (
        f"case_count:{len(per_case)}",
        f"gold_entity_total:{gold_total}",
        f"body_coverage_ratio:{body_ratio}",
        f"candidate_coverage_ratio:{cand_ratio}",
        f"ungrounded_count:{len(ungrounded)}",
        "import_write_fail_closed",
        "dspy:false",
    )
    return GoldBodyGroundingAuditPackage(
        schema_version=SCHEMA_VERSION,
        case_count=len(per_case),
        gold_entity_total=gold_total,
        grounded_in_body=in_body,
        grounded_in_candidates=in_cands,
        body_coverage_ratio=body_ratio,
        candidate_coverage_ratio=cand_ratio,
        ungrounded=tuple(ungrounded),
        per_case=tuple(per_case),
        diagnostics=diagnostics,
        import_eligible=False,
        graph_writes_allowed=False,
        dspy_optimizer_enabled=False,
    )


__all__ = [
    "SCHEMA_VERSION",
    "GoldBodyGroundingAuditPackage",
    "audit_gold_body_grounding",
]
