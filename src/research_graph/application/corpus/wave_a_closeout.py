"""Wave A data-readiness closeout.

Evaluates whether Wave A metrics meet freeze thresholds for data readiness
without authorizing import, graph writes, or Wave B auto-start.

wave_a_closed means operator may discuss Wave B — not that extraction quality
or graph import is ready.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any, Literal

SCHEMA_VERSION = "wave-a-closeout.v1"
DEFAULT_MIN_HYBRID_FOUND = 40

CloseoutSignal = Literal["wave_a_closed", "blocked"]

DEFAULT_OPERATOR_COMMANDS: tuple[str, ...] = (
    "uv run python scripts/verify_onion_layering.py",
    "uv run python scripts/verify_import_hold_inventory.py",
    "uv run python scripts/verify_etl_body_coverage.py",
    "uv run python scripts/verify_etl_preprocess_fleet.py",
    "uv run python scripts/verify_etl_continuity_readiness.py",
    "uv run python scripts/verify_hybrid_expand_batch.py",
    "uv run python scripts/verify_wave_a_closeout.py",
)


@dataclass(frozen=True, slots=True)
class WaveACloseoutPackage:
    schema_version: str
    closeout_signal: CloseoutSignal
    closeout_pass: bool
    hybrid_found: int
    min_hybrid_found: int
    readiness_signal: str
    import_hold_hits: int
    preprocess_errors: int
    preprocess_body_count: int
    article_count: int
    diagnostics: tuple[str, ...]
    operator_commands: tuple[str, ...]
    wave_b_gate_open: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("wave A closeout cannot authorize import/writes")
        if self.wave_b_gate_open:
            raise ValueError(
                "wave A closeout cannot auto-open Wave B; set wave_b_gate_open=False"
            )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "closeout_signal": self.closeout_signal,
            "closeout_pass": self.closeout_pass,
            "hybrid_found": self.hybrid_found,
            "min_hybrid_found": self.min_hybrid_found,
            "readiness_signal": self.readiness_signal,
            "import_hold_hits": self.import_hold_hits,
            "preprocess_errors": self.preprocess_errors,
            "preprocess_body_count": self.preprocess_body_count,
            "article_count": self.article_count,
            "diagnostics": list(self.diagnostics),
            "operator_commands": list(self.operator_commands),
            "wave_b_gate_open": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave A data-readiness closeout only; not import authorization; "
                "not extraction quality; Wave B requires explicit human go"
            ),
        }


def evaluate_wave_a_closeout(
    *,
    hybrid_found: int,
    readiness_signal: str,
    import_hold_hits: int,
    preprocess_errors: int,
    preprocess_body_count: int,
    article_count: int,
    min_hybrid_found: int = DEFAULT_MIN_HYBRID_FOUND,
    operator_commands: Sequence[str] | None = None,
) -> WaveACloseoutPackage:
    """Derive Wave A closeout from freeze metrics (pure)."""
    reasons: list[str] = []
    if hybrid_found < min_hybrid_found:
        reasons.append(f"hybrid_found_below_min:{hybrid_found}<{min_hybrid_found}")
    if readiness_signal != "ready_for_review":
        reasons.append(f"readiness_not_ready_for_review:{readiness_signal}")
    if import_hold_hits != 0:
        reasons.append(f"import_hold_hits:{import_hold_hits}")
    if preprocess_errors != 0:
        reasons.append(f"preprocess_errors:{preprocess_errors}")
    if preprocess_body_count <= 0:
        reasons.append("preprocess_body_count_zero")

    closed = not reasons
    signal: CloseoutSignal = "wave_a_closed" if closed else "blocked"
    diagnostics = (
        f"closeout_signal:{signal}",
        f"hybrid_found:{hybrid_found}",
        f"min_hybrid_found:{min_hybrid_found}",
        f"readiness_signal:{readiness_signal}",
        f"import_hold_hits:{import_hold_hits}",
        f"preprocess_errors:{preprocess_errors}",
        f"preprocess_body_count:{preprocess_body_count}",
        f"article_count:{article_count}",
        *(f"block:{r}" for r in reasons),
        "import_write_fail_closed",
        "wave_b_not_auto_open",
        "wave_a_data_readiness_only",
    )
    cmds = tuple(operator_commands) if operator_commands is not None else DEFAULT_OPERATOR_COMMANDS
    return WaveACloseoutPackage(
        schema_version=SCHEMA_VERSION,
        closeout_signal=signal,
        closeout_pass=closed,
        hybrid_found=hybrid_found,
        min_hybrid_found=min_hybrid_found,
        readiness_signal=readiness_signal,
        import_hold_hits=import_hold_hits,
        preprocess_errors=preprocess_errors,
        preprocess_body_count=preprocess_body_count,
        article_count=article_count,
        diagnostics=diagnostics,
        operator_commands=cmds,
    )


__all__ = [
    "DEFAULT_MIN_HYBRID_FOUND",
    "DEFAULT_OPERATOR_COMMANDS",
    "SCHEMA_VERSION",
    "CloseoutSignal",
    "WaveACloseoutPackage",
    "evaluate_wave_a_closeout",
]
