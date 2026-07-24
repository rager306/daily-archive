"""ETL unattended fleet dashboard (M266/M271 process glue).

Composes continuity pack + ship matrix + import-hold + quality n-contract.
Never import. Never graph writes.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.wave_b_quality_n_contract import (
    evaluate_quality_n_contract,
    extract_joined_count,
)

SCHEMA_VERSION = "etl-fleet.v2"


@dataclass(frozen=True, slots=True)
class EtlFleetPackage:
    schema_version: str
    continuity: dict[str, Any]
    ship_matrix: dict[str, Any] | None
    import_hold: dict[str, Any]
    quality_n: dict[str, Any] | None
    alerts: tuple[str, ...]
    diagnostics: tuple[str, ...]
    operator_status: str
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("etl fleet cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "continuity": dict(self.continuity),
            "ship_matrix": dict(self.ship_matrix) if self.ship_matrix else None,
            "import_hold": dict(self.import_hold),
            "quality_n": dict(self.quality_n) if self.quality_n else None,
            "alerts": list(self.alerts),
            "diagnostics": list(self.diagnostics),
            "operator_status": self.operator_status,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Unattended fleet glue: pack + ship matrix + import-hold + quality n. "
                "Never expands, never imports. Expand uses default pack refresh. "
                "Use --rescore-quality for live same-n matrix/grounding."
            ),
        }


def build_etl_fleet_package(
    *,
    continuity: Mapping[str, Any],
    import_hold: Mapping[str, Any],
    ship_matrix: Mapping[str, Any] | None = None,
    quality_n: Mapping[str, Any] | None = None,
    grounding: Mapping[str, Any] | None = None,
) -> EtlFleetPackage:
    """Pure compose of already-built operator payloads."""
    cont = dict(continuity or {})
    hold = dict(import_hold or {})
    matrix = dict(ship_matrix) if ship_matrix else None
    ground = dict(grounding) if grounding else None

    # Derive quality n if not provided
    qn = dict(quality_n) if quality_n else None
    if qn is None and matrix is not None:
        worlds = matrix.get("worlds") if isinstance(matrix.get("worlds"), Mapping) else {}
        q_embedded = worlds.get("quality_n_contract") if isinstance(worlds, Mapping) else None
        if isinstance(q_embedded, Mapping):
            qn = dict(q_embedded)
        else:
            header_n = extract_joined_count(matrix)
            gepa_n = None
            llm_n = None
            if isinstance(worlds, Mapping):
                raw_ctx = worlds.get("context")
                ctx: Mapping[str, Any] = raw_ctx if isinstance(raw_ctx, Mapping) else {}
                header_n = header_n if header_n is not None else ctx.get("joined_count")
                gepa_n = ctx.get("gepa_joined_count")
                llm_n = ctx.get("compare_joined_count")
            ground_n = extract_joined_count(ground) if ground else None
            qn = evaluate_quality_n_contract(
                header_n=int(header_n) if header_n is not None else None,
                llm_n=int(llm_n) if llm_n is not None else None,
                gepa_n=int(gepa_n) if gepa_n is not None else None,
                grounding_n=int(ground_n) if ground_n is not None else None,
                matrix_n=int(header_n) if header_n is not None else None,
                canonical=int(header_n) if header_n is not None else None,
            ).to_dict()

    alerts: list[str] = []
    for a in cont.get("alerts") or []:
        alerts.append(f"continuity:{a}")
    hits = hold.get("enablement_hits", 0)
    try:
        hits_n = int(hits) if not isinstance(hits, list) else len(hits)
    except (TypeError, ValueError):
        hits_n = 0
    if hold.get("verdict") not in (None, "pass", "ok", True) and hits_n:
        alerts.append(f"import_hold_hits:{hits_n}")
    if matrix and matrix.get("gepa_justified") is True:
        alerts.append("gepa_justified_true_review_deploy")
    if cont.get("import_eligible") is True or hold.get("import_eligible") is True:
        alerts.append("import_eligible_true_fail_closed")
    if qn and qn.get("all_match") is False:
        alerts.append(
            "quality_n_mismatch:"
            + ",".join(str(x) for x in (qn.get("mismatches") or [])[:4])
        )

    dash = cont.get("dashboard") if isinstance(cont.get("dashboard"), Mapping) else cont
    hybrid_found = dash.get("hybrid_found")
    hybrid_fraction = dash.get("hybrid_fraction")
    same_inode = dash.get("multi_root_same_inode_count")
    ship_path = (matrix or {}).get("ship_path")
    diagnostics = (
        f"hybrid_found:{hybrid_found}",
        f"hybrid_fraction:{hybrid_fraction}",
        f"multi_root_same_inode:{same_inode}",
        f"ship_path:{ship_path}",
        f"import_hold_verdict:{hold.get('verdict')}",
        f"quality_n_all_match:{(qn or {}).get('all_match')}",
        f"quality_n_canonical:{(qn or {}).get('canonical_joined_count')}",
        f"alerts:{len(alerts)}",
        "import_write_fail_closed",
        "etl_fleet_only",
    )
    status = "ok" if not alerts else "alerts"
    return EtlFleetPackage(
        schema_version=SCHEMA_VERSION,
        continuity=cont,
        ship_matrix=matrix,
        import_hold=hold,
        quality_n=qn,
        alerts=tuple(alerts),
        diagnostics=diagnostics,
        operator_status=status,
    )


__all__ = [
    "SCHEMA_VERSION",
    "EtlFleetPackage",
    "build_etl_fleet_package",
]
