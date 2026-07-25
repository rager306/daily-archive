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

    @property
    def assigned_count(self) -> int:
        return sum(1 for s in self.slots if s.paper_id)

    @property
    def annotation_status(self) -> str:
        n = self.assigned_count
        if n == 0:
            return "design_only"
        if n >= self.target_size:
            return "ids_assigned_labels_pending"
        return "partial_ids_assigned"

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "target_size": self.target_size,
            "strata": list(self.strata),
            "slots": [s.to_dict() for s in self.slots],
            "label_schema": list(self.label_schema),
            "diagnostics": list(self.diagnostics),
            "assigned_count": self.assigned_count,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "annotation_status": self.annotation_status,
            "note": (
                "Canary slots for evidence metrics. paper_ids may be assigned; "
                "labels still pending. Never feed held-out into GEPA/LLM train. "
                "Never import."
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
    status = (
        "design_only"
        if assigned_n == 0
        else (
            "ids_assigned_labels_pending"
            if assigned_n >= n
            else "partial_ids_assigned"
        )
    )
    diagnostics = (
        f"target_size:{n}",
        f"slot_count:{len(slots)}",
        f"strata_count:{len(strata_list)}",
        f"assigned:{assigned_n}",
        f"annotation_status:{status}",
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


def assign_canary_paper_ids(
    design: CanaryCorpusDesign,
    paper_ids: Sequence[str],
    *,
    held_out_count: int = 12,
    held_out_seed: int = 0,
) -> tuple[CanaryCorpusDesign, tuple[str, ...], dict[str, Any]]:
    """Fill unassigned slots from paper_ids in order; freeze held-out split.

    Held-out is a deterministic tail of assigned ids (not used as GEPA train).
    Returns (new_design, held_out_ids, freeze_payload).
    """
    import hashlib

    from research_graph.application.corpus.gt_isolation import freeze_canary_split

    # unique preserve order
    seen: set[str] = set()
    pool: list[str] = []
    for raw in paper_ids:
        pid = str(raw).replace("arxiv:", "").strip()
        if not pid or pid in seen:
            continue
        seen.add(pid)
        pool.append(pid)

    if len(pool) < design.target_size:
        raise ValueError(
            f"need at least {design.target_size} unique paper_ids, got {len(pool)}"
        )

    filled: list[CanarySlot] = []
    pi = 0
    for slot in design.slots:
        if slot.paper_id:
            filled.append(slot)
            continue
        pid = pool[pi]
        pi += 1
        filled.append(
            CanarySlot(
                slot_id=slot.slot_id,
                stratum=slot.stratum,
                priority=slot.priority,
                labels_planned=slot.labels_planned,
                paper_id=pid,
                notes="paper_id assigned; labels pending",
            )
        )

    assigned_ids = [s.paper_id for s in filled if s.paper_id]
    # deterministic held-out: hash-sort assigned, take last held_out_count
    scored = sorted(
        assigned_ids,
        key=lambda x: hashlib.md5(f"{held_out_seed}:{x}".encode()).hexdigest(),
    )
    k = max(0, min(int(held_out_count), len(scored) // 3 if len(scored) >= 3 else 0))
    if k == 0 and held_out_count > 0 and len(scored) >= 2:
        k = 1
    held = tuple(scored[-k:]) if k else ()
    freeze = freeze_canary_split(assigned_ids, held_out_ids=held)

    new_design = build_canary_corpus_design(
        target_size=design.target_size,
        strata=design.strata,
        assigned=[(s.slot_id, s.paper_id) for s in filled if s.paper_id],
    )
    # rebuild preserves notes via assigned path; re-apply label notes
    notes_slots = []
    for s in new_design.slots:
        notes_slots.append(
            CanarySlot(
                slot_id=s.slot_id,
                stratum=s.stratum,
                priority=s.priority,
                labels_planned=s.labels_planned,
                paper_id=s.paper_id,
                notes="paper_id assigned; labels pending" if s.paper_id else s.notes,
            )
        )
    diagnostics = tuple(new_design.diagnostics) + (
        f"held_out_count:{len(held)}",
        f"held_out_seed:{held_out_seed}",
        "gepa_held_out_frozen",
    )
    final = CanaryCorpusDesign(
        schema_version=new_design.schema_version,
        target_size=new_design.target_size,
        strata=new_design.strata,
        slots=tuple(notes_slots),
        label_schema=new_design.label_schema,
        diagnostics=diagnostics,
    )
    return final, held, freeze


__all__ = [
    "SCHEMA_VERSION",
    "DEFAULT_TARGET_SIZE",
    "DEFAULT_STRATA",
    "CanarySlot",
    "CanaryCorpusDesign",
    "build_canary_corpus_design",
    "assign_canary_paper_ids",
]
