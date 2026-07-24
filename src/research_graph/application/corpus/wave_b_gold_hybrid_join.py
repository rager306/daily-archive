"""Wave B reviewed-gold ↔ hybrid body join inventory.

Metadata join only: matches reviewed gold paper_ids to hybrid body paths.
No LLM, no DSPy, never import.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.application.corpus.etl_preprocess_fleet_audit import (
    discover_unique_hybrid_bodies,
)

SCHEMA_VERSION = "wave-b-reviewed-gold-hybrid-join.v1"


def normalize_paper_id(paper_id: str) -> str:
    """Normalize arxiv-like paper ids to bare catalog form (no arxiv: / version)."""
    s = str(paper_id).strip()
    for prefix in (
        "arxiv:",
        "http://arxiv.org/abs/",
        "https://arxiv.org/abs/",
        "http://ar5iv.labs.arxiv.org/html/",
        "https://ar5iv.labs.arxiv.org/html/",
    ):
        if s.lower().startswith(prefix):
            s = s[len(prefix) :]
            break
    if "/" in s:
        s = s.rsplit("/", 1)[-1]
    # strip version suffix 1206.6423v2
    if "v" in s:
        head, tail = s.rsplit("v", 1)
        if tail.isdigit():
            s = head
    return s


@dataclass(frozen=True, slots=True)
class GoldHybridJoinPackage:
    schema_version: str
    gold_case_count: int
    hybrid_unique_count: int
    joined_count: int
    missing_hybrid_count: int
    joined: tuple[dict[str, Any], ...]
    missing_paper_ids: tuple[str, ...]
    diagnostics: tuple[str, ...]
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("gold hybrid join cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("gold hybrid join cannot enable DSPy")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "gold_case_count": self.gold_case_count,
            "hybrid_unique_count": self.hybrid_unique_count,
            "joined_count": self.joined_count,
            "missing_hybrid_count": self.missing_hybrid_count,
            "joined": list(self.joined),
            "missing_paper_ids": list(self.missing_paper_ids),
            "diagnostics": list(self.diagnostics),
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Reviewed gold to hybrid body join only; "
                "not LLM; not DSPy; not import; not metrics"
            ),
        }


def inventory_reviewed_gold_hybrid_join(
    *,
    gold_records: Sequence[Mapping[str, Any]],
    body_roots: Sequence[Path],
) -> GoldHybridJoinPackage:
    """Join gold records to unique hybrid bodies by normalized paper_id."""
    hybrid_refs = discover_unique_hybrid_bodies(body_roots)
    hybrid_by_id = {ref.paper_id: ref for ref in hybrid_refs}

    joined: list[dict[str, Any]] = []
    missing: list[str] = []
    seen_cases: set[str] = set()

    for rec in gold_records:
        case_id = str(rec.get("case_id") or "")
        paper_raw = str(rec.get("paper_id") or "")
        bare = normalize_paper_id(paper_raw)
        if not bare:
            continue
        # de-dupe by case_id when present
        key = case_id or bare
        if key in seen_cases:
            continue
        seen_cases.add(key)
        href = hybrid_by_id.get(bare)
        if href is None:
            missing.append(bare)
            continue
        joined.append(
            {
                "case_id": case_id or f"case:unknown:{bare}",
                "paper_id": bare,
                "paper_id_raw": paper_raw,
                "body_path": str(href.path),
                "body_root": href.body_root,
                "split": "train"
                if "train" in case_id
                else ("validation" if "validation" in case_id else "unknown"),
                "import_eligible": False,
            }
        )

    diagnostics = (
        f"gold_cases:{len(seen_cases)}",
        f"hybrid_unique:{len(hybrid_by_id)}",
        f"joined:{len(joined)}",
        f"missing_hybrid:{len(missing)}",
        "dspy:false",
        "import_write_fail_closed",
        "join_only_no_metrics",
    )
    return GoldHybridJoinPackage(
        schema_version=SCHEMA_VERSION,
        gold_case_count=len(seen_cases),
        hybrid_unique_count=len(hybrid_by_id),
        joined_count=len(joined),
        missing_hybrid_count=len(missing),
        joined=tuple(joined),
        missing_paper_ids=tuple(sorted(set(missing))),
        diagnostics=diagnostics,
        dspy_optimizer_enabled=False,
        import_eligible=False,
        graph_writes_allowed=False,
    )


__all__ = [
    "SCHEMA_VERSION",
    "GoldHybridJoinPackage",
    "inventory_reviewed_gold_hybrid_join",
    "normalize_paper_id",
]
