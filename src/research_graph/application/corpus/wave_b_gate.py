"""Wave B extraction-quality gate ratchet (M251 / D123).

Default blocked. Opens only when ``human_go=True`` is passed explicitly.
Wave A closeout_pass is context only — never authorization.

Never authorizes import or graph writes.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal

SCHEMA_VERSION = "m251-wave-b-gate.v1"

GateSignal = Literal["blocked", "open"]


@dataclass(frozen=True, slots=True)
class WaveBGatePackage:
    schema_version: str
    gate_signal: GateSignal
    wave_b_gate_open: bool
    human_go: bool
    wave_a_closeout_pass: bool | None
    wave_a_closeout_signal: str | None
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("wave B gate cannot authorize import/writes")
        if self.wave_b_gate_open and not self.human_go:
            raise ValueError("wave_b_gate_open requires human_go=True")
        if self.gate_signal == "open" and not self.human_go:
            raise ValueError("gate_signal=open requires human_go=True")
        if self.wave_b_gate_open != (self.gate_signal == "open"):
            raise ValueError("wave_b_gate_open must match gate_signal")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "gate_signal": self.gate_signal,
            "wave_b_gate_open": self.wave_b_gate_open,
            "human_go": self.human_go,
            "wave_a_closeout_pass": self.wave_a_closeout_pass,
            "wave_a_closeout_signal": self.wave_a_closeout_signal,
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave B gate only; human_go is sole authorizer; "
                "Wave A closeout is context not authorization; "
                "not import; not Falkor"
            ),
        }


def evaluate_wave_b_gate(
    *,
    human_go: bool = False,
    wave_a_closeout_pass: bool | None = None,
    wave_a_closeout_signal: str | None = None,
) -> WaveBGatePackage:
    """Evaluate Wave B gate. Default human_go=False → blocked."""
    diagnostics: list[str] = [
        f"human_go:{human_go}",
        f"wave_a_closeout_pass:{wave_a_closeout_pass}",
        f"wave_a_closeout_signal:{wave_a_closeout_signal}",
        "closeout_not_authorization",
        "import_write_fail_closed",
        "d123_wave_b_requires_human_go",
    ]
    if wave_a_closeout_pass is False:
        diagnostics.append("closeout_pass_false")
    if wave_a_closeout_pass is True and not human_go:
        diagnostics.append("closeout_pass_alone_insufficient")

    if human_go:
        signal: GateSignal = "open"
        open_flag = True
        diagnostics.append("gate_signal:open")
    else:
        signal = "blocked"
        open_flag = False
        diagnostics.append("gate_signal:blocked")

    return WaveBGatePackage(
        schema_version=SCHEMA_VERSION,
        gate_signal=signal,
        wave_b_gate_open=open_flag,
        human_go=human_go,
        wave_a_closeout_pass=wave_a_closeout_pass,
        wave_a_closeout_signal=wave_a_closeout_signal,
        diagnostics=tuple(diagnostics),
    )


__all__ = [
    "SCHEMA_VERSION",
    "GateSignal",
    "WaveBGatePackage",
    "evaluate_wave_b_gate",
]
