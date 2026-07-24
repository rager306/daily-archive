"""ETL unattended fleet dashboard (M266 process glue).

Composes continuity pack + ship matrix + import-hold into one operator view.
Never import. Never graph writes.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

SCHEMA_VERSION = "etl-fleet.v1"


@dataclass(frozen=True, slots=True)
class EtlFleetPackage:
    schema_version: str
    continuity: dict[str, Any]
    ship_matrix: dict[str, Any] | None
    import_hold: dict[str, Any]
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
            "alerts": list(self.alerts),
            "diagnostics": list(self.diagnostics),
            "operator_status": self.operator_status,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Unattended fleet glue: pack + ship matrix + import-hold. "
                "Never expands, never imports. Expand uses default pack refresh."
            ),
        }


def build_etl_fleet_package(
    *,
    continuity: Mapping[str, Any],
    import_hold: Mapping[str, Any],
    ship_matrix: Mapping[str, Any] | None = None,
) -> EtlFleetPackage:
    """Pure compose of already-built operator payloads."""
    cont = dict(continuity or {})
    hold = dict(import_hold or {})
    matrix = dict(ship_matrix) if ship_matrix else None
    alerts: list[str] = []
    for a in cont.get("alerts") or []:
        alerts.append(f"continuity:{a}")
    if hold.get("verdict") not in (None, "pass", "ok", True):
        if hold.get("enablement_hits", 0) not in (0, None):
            alerts.append(f"import_hold_hits:{hold.get('enablement_hits')}")
    if matrix and matrix.get("gepa_justified") is True:
        alerts.append("gepa_justified_true_review_deploy")
    if cont.get("import_eligible") is True or hold.get("import_eligible") is True:
        alerts.append("import_eligible_true_fail_closed")

    dash = cont.get("dashboard") if isinstance(cont.get("dashboard"), Mapping) else cont
    hybrid_found = dash.get("hybrid_found")
    hybrid_fraction = dash.get("hybrid_fraction")
    ship_path = (matrix or {}).get("ship_path")
    diagnostics = (
        f"hybrid_found:{hybrid_found}",
        f"hybrid_fraction:{hybrid_fraction}",
        f"ship_path:{ship_path}",
        f"import_hold_verdict:{hold.get('verdict')}",
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
        alerts=tuple(alerts),
        diagnostics=diagnostics,
        operator_status=status,
    )


__all__ = [
    "SCHEMA_VERSION",
    "EtlFleetPackage",
    "build_etl_fleet_package",
]
