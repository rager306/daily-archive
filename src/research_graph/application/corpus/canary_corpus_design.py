"""Canary corpus design plan (M277 E2.2) — design only, not full annotation.

Defines stratified slots for ≥60 papers so evidence-resolvability and structure
metrics can be measured without gold leakage into GEPA/LLM. Never import.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, field
from typing import Any

SCHEMA_VERSION = "canary-corpus-design.v1"
DEFAULT_TARGET_SIZE = 60

# Strata for layout/parse stress (design labels, not scored yet)
DEFAULT_STRATA: tuple[str, ...] = (
    "clean_single_column",
    "two_column",
    "multi_column_complex",
    "heavy_tables",
    "formulas_equations",
    "figures_captions",
    "ocr_or_scanned",
    "appendices_supplements",
    "references_dense",
    "mixed_language_or_script",
)


@dataclass(frozen=True, slots=True)
class CanarySlot:
    slot_id: str
    stratum: str
    priority: int
    labels_planned: tuple[str, ...]
    paper_id: str | None = None
    notes: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "slot_id": self.slot_id,
            "stratum": self.stratum,
            "priority": self.priority,
            "labels_planned": list(self.labels_planned),
            "paper_id": self.paper_id,
            "notes": self.notes,
            "status": "unassigned" if not self.paper_id else "assigned",
        }


@dataclass(frozen=True, slots=True)
class CanaryCorpusDesign:
    schema_version: str
    target_size: int
    strata: tuple[str, ...]
    slots: tuple[CanarySlot, ...]
    label_schema: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("canary design cannot authorize import/writes")
        if self.target_size < 60:
            raise ValueError("canary target_size must be >= 60")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "target_size": self.target_size,
            "strata": list(self.strata),
            "slots": [s.to_dict() for s in self.slots],
            "label_schema": list(self.label_schema),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "annotation_status": "design_only",
            "note": (
                "E2.2 design plan: stratified canary slots for evidence metrics. "
                "Not a gold set yet; do not feed into GEPA/LLM. Never import."
            ),
        }


def build_canary_corpus_design(
    *,
    target_size: int = DEFAULT_TARGET_SIZE,
    strata: Sequence[str] | None = None,
    assigned: Sequence[tuple[str, str]] | None = None,
) -> CanaryCorpusDesign:
    """Build design with ≥60 slots balanced across strata.

    assigned: optional (slot_id, paper_id) pairs for partial fill.
    """
    n = int(target_size)
    if n < 60:
        raise ValueError("canary target_size must be >= 60")
    strata_list = tuple(strata) if strata else DEFAULT_STRATA
    if not strata_list:
        raise ValueError("strata required")

    label_schema = (
        "section_tree_ok",
        "table_present",
        "formula_present",
        "figure_caption_link",
        "span_page_or_bbox",
        "ocr_degraded",
        "two_column",
    )

    # balance slots across strata
    base = n // len(strata_list)
    rem = n % len(strata_list)
    slots: list[CanarySlot] = []
    idx = 0
    for si, stratum in enumerate(strata_list):
        count = base + (1 if si < rem else 0)
        for j in range(count):
            idx += 1
            priority = 1 if stratum in {
                "ocr_or_scanned",
                "heavy_tables",
                "formulas_equations",
                "two_column",
            } else 2
            slots.append(
                CanarySlot(
                    slot_id=f"c{idx:03d}",
                    stratum=stratum,
                    priority=priority,
                    labels_planned=label_schema,
                    notes="design placeholder; assign paper_id later",
                )
            )

    assign_map = {sid: pid for sid, pid in (assigned or ())}
    if assign_map:
        filled: list[CanarySlot] = []
        for s in slots:
            pid = assign_map.get(s.slot_id)
            if pid:
                filled.append(
                    CanarySlot(
                        slot_id=s.slot_id,
                        stratum=s.stratum,
                        priority=s.priority,
                        labels_planned=s.labels_planned,
                        paper_id=pid,
                        notes="assigned in design plan",
                    )
                )
            else:
                filled.append(s)
        slots = filled

    assigned_n = sum(1 for s in slots if s.paper_id)
    diagnostics = (
        f"target_size:{n}",
        f"slot_count:{len(slots)}",
        f"strata_count:{len(strata_list)}",
        f"assigned:{assigned_n}",
        "annotation_status:design_only",
        "gepa_llm_isolation:do_not_use_as_train",
        "import_write_fail_closed",
    )
    return CanaryCorpusDesign(
        schema_version=SCHEMA_VERSION,
        target_size=n,
        strata=strata_list,
        slots=tuple(slots),
        label_schema=label_schema,
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "DEFAULT_TARGET_SIZE",
    "DEFAULT_STRATA",
    "CanarySlot",
    "CanaryCorpusDesign",
    "build_canary_corpus_design",
]
