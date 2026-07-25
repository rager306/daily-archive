"""Ground-truth isolation ratchet for GEPA/LLM (M278 E3.5).

Held-out canary / gold labels must not appear in train or prompt context.
Fail-closed: violations block the run path (still never import).
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any, Literal

IsolationRole = Literal["train", "prompt", "eval", "held_out"]


@dataclass(frozen=True, slots=True)
class IsolationVerdict:
    ok: bool
    violations: tuple[str, ...]
    held_out_count: int
    train_count: int
    import_eligible: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "ok": self.ok,
            "violations": list(self.violations),
            "held_out_count": self.held_out_count,
            "train_count": self.train_count,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "constraint_code": "gold_leakage",
        }


def freeze_canary_split(
    paper_ids: Sequence[str],
    *,
    held_out_ids: Sequence[str],
) -> dict[str, Any]:
    """Mark frozen canary held-out set. Design/eval only."""
    held = {str(x) for x in held_out_ids}
    all_ids = [str(x) for x in paper_ids]
    overlap_missing = sorted(held - set(all_ids))
    return {
        "schema_version": "canary-split.v1",
        "all_count": len(all_ids),
        "held_out_ids": sorted(held),
        "held_out_count": len(held),
        "held_out_not_in_pool": overlap_missing,
        "frozen": True,
        "import_eligible": False,
        "note": "held_out must not appear in GEPA/LLM train or prompt context",
    }


def check_gt_isolation(
    *,
    context_paper_ids: Sequence[str],
    held_out_ids: Sequence[str],
    role: IsolationRole,
    context_blob: str | None = None,
    gold_markers: Sequence[str] | None = None,
) -> IsolationVerdict:
    """Fail if held-out ids leak into train/prompt context.

    role=eval/held_out may include held-out ids.
    role=train/prompt must not.
    Optional gold_markers: substrings that must not appear in context_blob for train/prompt.
    """
    held = {str(x) for x in held_out_ids}
    ctx = [str(x) for x in context_paper_ids]
    violations: list[str] = []

    if role in {"train", "prompt"}:
        leaked = sorted(set(ctx) & held)
        for pid in leaked:
            violations.append(f"held_out_in_{role}:{pid}")
        if context_blob and gold_markers:
            blob = context_blob
            for m in gold_markers:
                if m and m in blob:
                    violations.append(f"gold_marker_in_{role}:{m[:64]}")
    elif role in {"eval", "held_out"}:
        pass  # allowed
    else:
        violations.append(f"unknown_role:{role}")

    return IsolationVerdict(
        ok=not violations,
        violations=tuple(violations),
        held_out_count=len(held),
        train_count=len(ctx) if role == "train" else 0,
    )


def assert_context_isolated(
    context: Mapping[str, Any],
    *,
    held_out_ids: Sequence[str],
    role: IsolationRole,
) -> IsolationVerdict:
    """Convenience over dict context with paper_ids + optional prompt_text."""
    ids = context.get("paper_ids") or context.get("train_ids") or []
    if not isinstance(ids, (list, tuple)):
        ids = []
    blob = context.get("prompt_text") or context.get("context_text")
    markers = context.get("forbidden_gold_markers")
    if markers is not None and not isinstance(markers, (list, tuple)):
        markers = None
    return check_gt_isolation(
        context_paper_ids=list(ids),  # type: ignore[arg-type]
        held_out_ids=held_out_ids,
        role=role,
        context_blob=str(blob) if blob is not None else None,
        gold_markers=list(markers) if markers else None,  # type: ignore[arg-type]
    )


__all__ = [
    "IsolationRole",
    "IsolationVerdict",
    "freeze_canary_split",
    "check_gt_isolation",
    "assert_context_isolated",
]
