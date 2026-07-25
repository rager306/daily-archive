"""Wave A: controlled hybrid expand batch gate (pre-batch policy).

Pure application policy: live expand batch may run only when preflight has
ready papers, sidecars are healthy, limit is positive, and operator enabled live.

Never authorizes import. Never starts batch itself.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal

SCHEMA_VERSION = "hybrid-expand-batch-gate.v1"

ExpandBatchGateSignal = Literal["allow_limited_batch", "blocked", "repair"]


@dataclass(frozen=True, slots=True)
class HybridExpandBatchGatePackage:
    schema_version: str
    gate_signal: ExpandBatchGateSignal
    allow_live_batch: bool
    effective_limit: int
    ready_count: int
    preflight_signal: str
    grobid_available: bool
    odl_available: bool
    reasons: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("expand batch gate cannot authorize import/writes")
        if self.allow_live_batch and self.gate_signal != "allow_limited_batch":
            raise ValueError("allow_live_batch requires gate_signal=allow_limited_batch")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "gate_signal": self.gate_signal,
            "allow_live_batch": self.allow_live_batch,
            "effective_limit": self.effective_limit,
            "ready_count": self.ready_count,
            "preflight_signal": self.preflight_signal,
            "grobid_available": self.grobid_available,
            "odl_available": self.odl_available,
            "reasons": list(self.reasons),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave A expand batch gate only. allow_live_batch is not import. "
                "Requires ready PDF papers + healthy sidecars + positive limit."
            ),
        }


def evaluate_expand_batch_gate(
    *,
    preflight_signal: str,
    ready_count: int,
    limit: int,
    enable_live_hybrid: bool,
    grobid_available: bool,
    odl_available: bool = True,
    max_limit: int = 20,
) -> HybridExpandBatchGatePackage:
    """Decide whether a limited live hybrid expand batch is allowed."""
    reasons: list[str] = []
    if not enable_live_hybrid:
        reasons.append("live_hybrid_not_enabled")
    if int(limit) <= 0:
        reasons.append("limit_not_positive")
    if int(ready_count) <= 0:
        reasons.append("no_ready_papers")
    if preflight_signal == "blocked":
        reasons.append("preflight_blocked")
    if not grobid_available:
        reasons.append("grobid_unavailable")
    if not odl_available:
        reasons.append("opendataloader_unavailable")

    effective = max(0, min(int(limit), int(max_limit), int(ready_count)))
    if reasons:
        signal: ExpandBatchGateSignal = (
            "repair" if ready_count > 0 and enable_live_hybrid else "blocked"
        )
        allow = False
        effective = 0
    else:
        signal = "allow_limited_batch"
        allow = True
        if effective <= 0:
            signal = "blocked"
            allow = False
            reasons.append("effective_limit_zero")

    diagnostics = (
        f"gate_signal:{signal}",
        f"ready_count:{ready_count}",
        f"limit:{limit}",
        f"effective_limit:{effective}",
        f"preflight_signal:{preflight_signal}",
        f"grobid:{grobid_available}",
        f"odl:{odl_available}",
        f"enable_live:{enable_live_hybrid}",
        "import_write_fail_closed",
        "wave_a_expand_batch_gate_only",
    )
    return HybridExpandBatchGatePackage(
        schema_version=SCHEMA_VERSION,
        gate_signal=signal,
        allow_live_batch=allow,
        effective_limit=effective,
        ready_count=int(ready_count),
        preflight_signal=str(preflight_signal),
        grobid_available=bool(grobid_available),
        odl_available=bool(odl_available),
        reasons=tuple(reasons),
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "ExpandBatchGateSignal",
    "HybridExpandBatchGatePackage",
    "evaluate_expand_batch_gate",
]
