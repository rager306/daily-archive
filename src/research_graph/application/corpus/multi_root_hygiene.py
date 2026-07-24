"""Multi-root hybrid body hygiene plan (M267).

Identical multi-root copies are storage debt, not content corruption.
This module **plans** primary-root retention and optional hardlink/remove
actions. It never deletes by default and never authorizes import.
"""

from __future__ import annotations

import hashlib
import os
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "multi-root-hygiene.v1"

# Prefer expand-live then scholarly then older pilot roots (stable primary).
DEFAULT_PRIMARY_ROOT_ORDER = (
    "artifacts/m213-hybrid-gate/runs-live-expand",
    "artifacts/m213-hybrid-gate/runs-live-20",
    "artifacts/m213-hybrid-gate/runs-live-scholarly-20",
    "artifacts/m213-hybrid-gate/runs-live",
)


@dataclass(frozen=True, slots=True)
class MultiRootHygieneAction:
    paper_id: str
    action: str  # keep_primary | hardlink_duplicate | remove_duplicate | review_divergent
    primary_path: str
    duplicate_path: str | None
    content_sha256: str | None
    note: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "action": self.action,
            "primary_path": self.primary_path,
            "duplicate_path": self.duplicate_path,
            "content_sha256": self.content_sha256,
            "note": self.note,
        }


@dataclass(frozen=True, slots=True)
class MultiRootHygienePlan:
    schema_version: str
    multi_root_paper_id_count: int
    identical_content_count: int
    divergent_content_count: int
    primary_root_order: tuple[str, ...]
    actions: tuple[MultiRootHygieneAction, ...]
    applied_hardlinks: int
    applied_removes: int
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("multi-root hygiene cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "multi_root_paper_id_count": self.multi_root_paper_id_count,
            "identical_content_count": self.identical_content_count,
            "divergent_content_count": self.divergent_content_count,
            "primary_root_order": list(self.primary_root_order),
            "actions": [a.to_dict() for a in self.actions],
            "applied_hardlinks": self.applied_hardlinks,
            "applied_removes": self.applied_removes,
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Plan-only by default. Identical copies may hardlink when "
                "apply_hardlinks=True. Removes require apply_removes=True. "
                "Divergent copies are review-only. Never import."
            ),
        }


def _paper_id_for_body(path: Path) -> str | None:
    name = path.name
    if not name.endswith(".hybrid.body.md"):
        return None
    pid = name[: -len(".hybrid.body.md")]
    if pid == "original" and path.parent.name == "body":
        parent = path.parent.parent.name
        if parent:
            return parent
    return pid or None


def _root_rank(path: Path, primary_order: Sequence[str]) -> int:
    s = str(path).replace("\\", "/")
    for i, pref in enumerate(primary_order):
        if pref in s:
            return i
    return len(primary_order) + 100


def _sha256(path: Path) -> str:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError:
        return f"unreadable:{path}"


def plan_multi_root_hygiene(
    body_roots: Sequence[Path],
    *,
    primary_root_order: Sequence[str] = DEFAULT_PRIMARY_ROOT_ORDER,
    sample_limit: int = 50,
) -> MultiRootHygienePlan:
    """Build a plan: keep highest-priority primary; mark identical dups for hardlink."""
    by_id: dict[str, list[Path]] = {}
    for root in body_roots:
        root_p = Path(root)
        if not root_p.is_dir():
            continue
        for path in root_p.rglob("*.hybrid.body.md"):
            if not path.is_file():
                continue
            pid = _paper_id_for_body(path)
            if not pid:
                continue
            by_id.setdefault(pid, []).append(path)

    multi_ids = sorted(pid for pid, paths in by_id.items() if len(paths) > 1)
    identical = 0
    divergent = 0
    actions: list[MultiRootHygieneAction] = []
    order = tuple(str(x) for x in primary_root_order)

    for pid in multi_ids:
        paths = sorted(
            by_id[pid],
            key=lambda p: (_root_rank(p, order), str(p)),
        )
        digests = {p: _sha256(p) for p in paths}
        uniq = set(digests.values())
        primary = paths[0]
        primary_hash = digests[primary]
        if len(uniq) == 1:
            identical += 1
            actions.append(
                MultiRootHygieneAction(
                    paper_id=pid,
                    action="keep_primary",
                    primary_path=str(primary),
                    duplicate_path=None,
                    content_sha256=primary_hash,
                    note="primary retained by root priority",
                )
            )
            for dup in paths[1:]:
                if len(actions) >= sample_limit * 3:
                    break
                actions.append(
                    MultiRootHygieneAction(
                        paper_id=pid,
                        action="hardlink_duplicate",
                        primary_path=str(primary),
                        duplicate_path=str(dup),
                        content_sha256=primary_hash,
                        note="identical SHA; plan hardlink to primary (optional apply)",
                    )
                )
        else:
            divergent += 1
            actions.append(
                MultiRootHygieneAction(
                    paper_id=pid,
                    action="review_divergent",
                    primary_path=str(primary),
                    duplicate_path=str(paths[1]) if len(paths) > 1 else None,
                    content_sha256=None,
                    note="content hashes differ — human review; no auto action",
                )
            )

    diagnostics = (
        f"multi_root_ids:{len(multi_ids)}",
        f"identical:{identical}",
        f"divergent:{divergent}",
        f"actions:{len(actions)}",
        "plan_only_default",
        "import_write_fail_closed",
    )
    return MultiRootHygienePlan(
        schema_version=SCHEMA_VERSION,
        multi_root_paper_id_count=len(multi_ids),
        identical_content_count=identical,
        divergent_content_count=divergent,
        primary_root_order=order,
        actions=tuple(actions[: sample_limit * 4]),
        applied_hardlinks=0,
        applied_removes=0,
        diagnostics=diagnostics,
    )


def apply_multi_root_hardlinks(
    plan: MultiRootHygienePlan,
    *,
    apply_hardlinks: bool = False,
    apply_removes: bool = False,
) -> MultiRootHygienePlan:
    """Optionally replace identical duplicates with hardlinks to primary.

    Safe defaults: both flags False → no filesystem mutation.
    Never removes unless apply_removes=True (still only after hardlink success).
    """
    if not apply_hardlinks and not apply_removes:
        return plan

    hardlinks = 0
    removes = 0
    new_actions: list[MultiRootHygieneAction] = []
    for act in plan.actions:
        if act.action != "hardlink_duplicate" or not act.duplicate_path:
            new_actions.append(act)
            continue
        primary = Path(act.primary_path)
        dup = Path(act.duplicate_path)
        if not apply_hardlinks:
            new_actions.append(act)
            continue
        if not primary.is_file() or not dup.is_file():
            new_actions.append(
                MultiRootHygieneAction(
                    paper_id=act.paper_id,
                    action="hardlink_duplicate",
                    primary_path=act.primary_path,
                    duplicate_path=act.duplicate_path,
                    content_sha256=act.content_sha256,
                    note="skip: missing file",
                )
            )
            continue
        try:
            # same inode already?
            if primary.stat().st_ino == dup.stat().st_ino:
                hardlinks += 1
                new_actions.append(
                    MultiRootHygieneAction(
                        paper_id=act.paper_id,
                        action="hardlink_duplicate",
                        primary_path=act.primary_path,
                        duplicate_path=act.duplicate_path,
                        content_sha256=act.content_sha256,
                        note="already same inode",
                    )
                )
                continue
            # replace dup with hardlink
            tmp = dup.with_suffix(dup.suffix + ".hygiene-tmp")
            if tmp.exists():
                tmp.unlink()
            os.link(primary, tmp)
            os.replace(tmp, dup)
            hardlinks += 1
            note = "hardlinked to primary"
            if apply_removes:
                # hardlink already is single inode; remove would delete content — refuse
                note += "; remove skipped (hardlink shares inode)"
            new_actions.append(
                MultiRootHygieneAction(
                    paper_id=act.paper_id,
                    action="hardlink_duplicate",
                    primary_path=act.primary_path,
                    duplicate_path=act.duplicate_path,
                    content_sha256=act.content_sha256,
                    note=note,
                )
            )
        except OSError as exc:
            new_actions.append(
                MultiRootHygieneAction(
                    paper_id=act.paper_id,
                    action="hardlink_duplicate",
                    primary_path=act.primary_path,
                    duplicate_path=act.duplicate_path,
                    content_sha256=act.content_sha256,
                    note=f"hardlink_failed:{type(exc).__name__}",
                )
            )

    diagnostics = tuple(plan.diagnostics) + (
        f"applied_hardlinks:{hardlinks}",
        f"applied_removes:{removes}",
    )
    return MultiRootHygienePlan(
        schema_version=plan.schema_version,
        multi_root_paper_id_count=plan.multi_root_paper_id_count,
        identical_content_count=plan.identical_content_count,
        divergent_content_count=plan.divergent_content_count,
        primary_root_order=plan.primary_root_order,
        actions=tuple(new_actions),
        applied_hardlinks=hardlinks,
        applied_removes=removes,
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "DEFAULT_PRIMARY_ROOT_ORDER",
    "MultiRootHygieneAction",
    "MultiRootHygienePlan",
    "plan_multi_root_hygiene",
    "apply_multi_root_hardlinks",
]
