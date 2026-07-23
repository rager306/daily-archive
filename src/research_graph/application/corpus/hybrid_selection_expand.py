"""Wave A hybrid selection expand planner (M245).

Plans the next hybrid-gate selection rung from local PDF inventory rows,
excluding papers already selected or already having hybrid bodies.

Pure application: no FS, no network, no hybrid batch, never authorizes import.
Selection shape matches m213-hybrid-gate-selection.v1 for later batch use.
"""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

SCHEMA_VERSION = "m245-hybrid-selection-expand.v1"
SELECTION_SCHEMA_VERSION = "m213-hybrid-gate-selection.v1"
DEFAULT_MAX_BYTES = 25 * 1024 * 1024
DEFAULT_TARGET_COUNT = 20


@dataclass(frozen=True, slots=True)
class InventoryPdfRow:
    paper_id: str
    category: str
    pdf_path: str
    byte_size: int
    sha256: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "category": self.category,
            "pdf_path": self.pdf_path,
            "byte_size": self.byte_size,
            "sha256": self.sha256,
            "import_eligible": False,
        }


@dataclass(frozen=True, slots=True)
class HybridSelectionExpandPackage:
    schema_version: str
    proposed_count: int
    target_count: int
    available_after_filters: int
    inventory_count: int
    excluded_count: int
    max_bytes: int
    proposed_papers: tuple[InventoryPdfRow, ...]
    diagnostics: tuple[str, ...]
    selection_policy: str
    extends: str
    milestone_id: str
    rung: int
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("hybrid selection expand cannot authorize import/writes")

    def to_selection_dict(self) -> dict[str, Any]:
        """Emit m213-compatible selection JSON (proposal only)."""
        return {
            "schema_version": SELECTION_SCHEMA_VERSION,
            "milestone_id": self.milestone_id,
            "rung": self.rung,
            "count": self.proposed_count,
            "extends": self.extends,
            "selection_policy": self.selection_policy,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "papers": [
                {
                    "paper_id": p.paper_id,
                    "category": p.category,
                    "pdf_path": p.pdf_path,
                    "byte_size": p.byte_size,
                    "sha256": p.sha256,
                }
                for p in self.proposed_papers
            ],
            "note": (
                "Wave A expand proposal only; not hybrid batch run; "
                "not import authorization"
            ),
        }

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "proposed_count": self.proposed_count,
            "target_count": self.target_count,
            "available_after_filters": self.available_after_filters,
            "inventory_count": self.inventory_count,
            "excluded_count": self.excluded_count,
            "max_bytes": self.max_bytes,
            "proposed_papers": [p.to_dict() for p in self.proposed_papers],
            "diagnostics": list(self.diagnostics),
            "selection_policy": self.selection_policy,
            "extends": self.extends,
            "milestone_id": self.milestone_id,
            "rung": self.rung,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "selection_proposal": self.to_selection_dict(),
            "note": (
                "Wave A hybrid selection expand plan only; "
                "does not run hybrid batch or open import"
            ),
        }


def plan_next_hybrid_selection(
    *,
    inventory: Sequence[InventoryPdfRow],
    exclude_paper_ids: frozenset[str],
    target_count: int = DEFAULT_TARGET_COUNT,
    max_bytes: int = DEFAULT_MAX_BYTES,
    rung: int = 40,
    extends: str = "artifacts/m213-hybrid-gate/selection-20.json",
    milestone_id: str = "M245-hybrid-expand",
) -> HybridSelectionExpandPackage:
    """Propose next hybrid selection rung from inventory (pure).

    Policy:
    1. Drop excluded paper_ids (existing selection + existing hybrid bodies).
    2. Drop rows over max_bytes.
    3. Round-robin categories for diversity until target_count.
    4. Within category, smaller PDFs first (stable by paper_id).
    """
    if target_count < 0:
        raise ValueError("target_count must be >= 0")
    if max_bytes <= 0:
        raise ValueError("max_bytes must be > 0")

    size_skipped = 0
    excluded = 0
    by_cat: dict[str, list[InventoryPdfRow]] = defaultdict(list)

    for row in inventory:
        pid = row.paper_id.strip()
        cat = row.category.strip()
        if not pid or not cat:
            continue
        if pid in exclude_paper_ids:
            excluded += 1
            continue
        if row.byte_size > max_bytes:
            size_skipped += 1
            continue
        by_cat[cat].append(row)

    for cat in by_cat:
        by_cat[cat].sort(key=lambda r: (r.byte_size, r.paper_id))

    available = sum(len(v) for v in by_cat.values())
    proposed: list[InventoryPdfRow] = []
    # round-robin over sorted categories
    categories = sorted(by_cat.keys())
    indices = dict.fromkeys(categories, 0)
    while len(proposed) < target_count:
        progressed = False
        for cat in categories:
            if len(proposed) >= target_count:
                break
            i = indices[cat]
            rows = by_cat[cat]
            if i >= len(rows):
                continue
            proposed.append(rows[i])
            indices[cat] = i + 1
            progressed = True
        if not progressed:
            break

    policy = (
        f"local_catalog_pdfs_under_{max_bytes}_bytes_"
        f"exclude_selected_and_bodied_category_round_robin_target_{target_count}"
    )
    diagnostics = (
        f"inventory:{len(inventory)}",
        f"excluded:{excluded}",
        f"size_cap_skipped:{size_skipped}",
        f"available_after_filters:{available}",
        f"proposed:{len(proposed)}",
        f"target:{target_count}",
        f"categories:{len(categories)}",
        "import_write_fail_closed",
        "wave_a_selection_expand_plan_only",
        "no_hybrid_batch",
    )

    return HybridSelectionExpandPackage(
        schema_version=SCHEMA_VERSION,
        proposed_count=len(proposed),
        target_count=target_count,
        available_after_filters=available,
        inventory_count=len(inventory),
        excluded_count=excluded,
        max_bytes=max_bytes,
        proposed_papers=tuple(proposed),
        diagnostics=diagnostics,
        selection_policy=policy,
        extends=extends,
        milestone_id=milestone_id,
        rung=rung,
    )


__all__ = [
    "DEFAULT_MAX_BYTES",
    "DEFAULT_TARGET_COUNT",
    "HybridSelectionExpandPackage",
    "InventoryPdfRow",
    "SCHEMA_VERSION",
    "SELECTION_SCHEMA_VERSION",
    "plan_next_hybrid_selection",
]
