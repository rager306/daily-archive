"""Wave A hybrid expand preflight (M246).

Validates a proposed hybrid selection before any live batch: PDF presence and
already-bodied exclusions. Pure application when given check rows; never
authorizes import or runs hybrid batch.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any, Literal

SCHEMA_VERSION = "m246-hybrid-expand-preflight.v1"

PreflightSignal = Literal["blocked", "repair", "ready_to_batch"]


@dataclass(frozen=True, slots=True)
class ProposedPaperCheck:
    paper_id: str
    pdf_path: str
    pdf_exists: bool
    already_bodied: bool
    byte_size: int = 0

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "pdf_path": self.pdf_path,
            "pdf_exists": self.pdf_exists,
            "already_bodied": self.already_bodied,
            "byte_size": self.byte_size,
            "ready": bool(self.pdf_exists and not self.already_bodied),
            "import_eligible": False,
        }


def derive_preflight_signal(
    *,
    proposed_count: int,
    missing_pdf_count: int,
    already_bodied_count: int,
    ready_count: int,
) -> PreflightSignal:
    """Derive non-authorizing preflight signal before hybrid batch."""
    if proposed_count <= 0 or ready_count <= 0:
        return "blocked"
    if missing_pdf_count > 0 or already_bodied_count > 0:
        # partial readiness → repair (operator should trim or fix paths)
        if ready_count > 0:
            return "repair"
        return "blocked"
    if ready_count == proposed_count:
        return "ready_to_batch"
    return "repair"


@dataclass(frozen=True, slots=True)
class HybridExpandPreflightPackage:
    schema_version: str
    preflight_signal: PreflightSignal
    proposed_count: int
    missing_pdf_count: int
    already_bodied_count: int
    ready_count: int
    ready_paper_ids: tuple[str, ...]
    missing_paper_ids: tuple[str, ...]
    already_bodied_paper_ids: tuple[str, ...]
    selection_path: str
    target_count: int
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("hybrid expand preflight cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "preflight_signal": self.preflight_signal,
            "proposed_count": self.proposed_count,
            "missing_pdf_count": self.missing_pdf_count,
            "already_bodied_count": self.already_bodied_count,
            "ready_count": self.ready_count,
            "ready_paper_ids": list(self.ready_paper_ids),
            "missing_paper_ids": list(self.missing_paper_ids),
            "already_bodied_paper_ids": list(self.already_bodied_paper_ids),
            "selection_path": self.selection_path,
            "target_count": self.target_count,
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave A preflight only; ready_to_batch is not import authorization "
                "and does not start hybrid batch by itself"
            ),
        }


def preflight_hybrid_expand(
    *,
    checks: Sequence[ProposedPaperCheck],
    selection_path: str,
    target_count: int,
) -> HybridExpandPreflightPackage:
    """Aggregate proposed paper checks into fail-closed preflight package."""
    missing: list[str] = []
    bodied: list[str] = []
    ready: list[str] = []
    for c in checks:
        pid = c.paper_id.strip()
        if not pid:
            continue
        if c.already_bodied:
            bodied.append(pid)
            continue
        if not c.pdf_exists:
            missing.append(pid)
            continue
        ready.append(pid)

    proposed_count = len(checks)
    signal = derive_preflight_signal(
        proposed_count=proposed_count,
        missing_pdf_count=len(missing),
        already_bodied_count=len(bodied),
        ready_count=len(ready),
    )
    diagnostics = (
        f"proposed:{proposed_count}",
        f"ready:{len(ready)}",
        f"missing_pdf:{len(missing)}",
        f"already_bodied:{len(bodied)}",
        f"preflight_signal:{signal}",
        f"target_count:{target_count}",
        "import_write_fail_closed",
        "wave_a_preflight_only",
        "no_batch_started",
    )
    return HybridExpandPreflightPackage(
        schema_version=SCHEMA_VERSION,
        preflight_signal=signal,
        proposed_count=proposed_count,
        missing_pdf_count=len(missing),
        already_bodied_count=len(bodied),
        ready_count=len(ready),
        ready_paper_ids=tuple(ready),
        missing_paper_ids=tuple(missing),
        already_bodied_paper_ids=tuple(bodied),
        selection_path=selection_path,
        target_count=target_count,
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "HybridExpandPreflightPackage",
    "PreflightSignal",
    "ProposedPaperCheck",
    "derive_preflight_signal",
    "preflight_hybrid_expand",
]
