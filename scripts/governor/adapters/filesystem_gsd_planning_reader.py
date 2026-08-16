"""Filesystem adapter for GSD's readable planning projections."""

import re
from pathlib import Path

from governor.ports.gsd_planning_reader import (
    GsdPlanningProjection,
    GsdPlanningReader,
    MilestonePlanningProjection,
)

_MILESTONE_ROW = re.compile(
    r"^-\s+(?P<marker>🔄|✅|⬜|❌|⏸️?)\s+\*\*"
    r"(?P<id>M\d{3}(?:-[a-z0-9]+)?):\s*(?P<title>.+?)\*\*"
)
_TASK_ROW = re.compile(r"^-\s+\[[ xX]\]\s+\*\*(?P<id>T\d+)\*\*:")
_STATE_ACTIVE = re.compile(r"^\*\*Active Milestone:\*\*\s+(?P<id>M\d{3}(?:-[a-z0-9]+)?)")
_STATUS_BY_MARKER = {
    "🔄": "active",
    "✅": "complete",
    "⬜": "pending",
    "❌": "cancelled",
    "⏸": "paused",
    "⏸️": "paused",
}


class FilesystemGsdPlanningReader(GsdPlanningReader):
    """Reads ROADMAP and phase PLAN projections without engine storage access."""

    def __init__(self, project_root: Path):
        self.root = project_root

    def read(self) -> GsdPlanningProjection:
        roadmap = self.root / ".gsd" / "ROADMAP.md"
        if not roadmap.is_file():
            return GsdPlanningProjection(
                available=False,
                diagnostics=("planning projection missing: .gsd/ROADMAP.md",),
            )

        rows = self._parse_roadmap(roadmap)
        if not rows:
            return GsdPlanningProjection(
                available=False,
                diagnostics=(
                    "planning projection contains no recognized milestone rows: "
                    ".gsd/ROADMAP.md",
                ),
            )

        ids_by_base: dict[str, list[str]] = {}
        for milestone_id, _, _ in rows:
            base = milestone_id.split("-", 1)[0]
            ids_by_base.setdefault(base, []).append(milestone_id)

        diagnostics: list[str] = []
        state_active = self._state_active_milestone()
        # Root ROADMAP.md renders an executing milestone as pending until closeout,
        # while STATE.md identifies the engine-selected active milestone. Promote
        # only that exact pending row to active so task projections are counted
        # without hand-editing engine projections.
        if state_active is not None:
            for index, (milestone_id, title, status) in enumerate(rows):
                if milestone_id == state_active and status == "pending":
                    rows[index] = (milestone_id, title, "active")
                    break
        roadmap_active = {row[0] for row in rows if row[2] == "active"}
        if state_active is not None and state_active not in roadmap_active:
            preferred = self._preferred_active_milestone(roadmap_active)
            diagnostics.append(
                "planning projection divergence: "
                f"STATE active={state_active}, ROADMAP active={sorted(roadmap_active)}; "
                f"preferred_active={preferred}; "
                "do not hand-edit engine database files — re-render STATE via GSD tools / "
                "complete or reopen milestones so Active Milestone matches ROADMAP"
            )
        projections: list[MilestonePlanningProjection] = []
        for milestone_id, title, status in rows:
            base = milestone_id.split("-", 1)[0]
            task_count = 0
            source = ".gsd/ROADMAP.md"
            if status == "active":
                phase_dir = self._phase_dir(milestone_id)
                if phase_dir is None:
                    diagnostics.append(
                        self._missing_phase_diagnostic(milestone_id)
                    )
                elif "-" not in milestone_id and len(ids_by_base[base]) > 1:
                    diagnostics.append(
                        f"ambiguous base milestone {milestone_id}; refusing suffixed phase merge"
                    )
                else:
                    task_ids = self._task_ids(phase_dir)
                    task_count = len(task_ids)
                    source = str(phase_dir.relative_to(self.root))
                    for slice_id in self._unplanned_slices(phase_dir):
                        diagnostics.append(
                            f"active milestone {milestone_id} slice {slice_id} "
                            "has no PLAN tasks — run gsd_plan_slice before execute"
                        )
            projections.append(
                MilestonePlanningProjection(
                    milestone_id=milestone_id,
                    title=title,
                    status=status,
                    task_count=task_count,
                    source=source,
                )
            )

        return GsdPlanningProjection(
            available=True,
            milestones=tuple(projections),
            diagnostics=tuple(diagnostics),
        )

    def _state_active_milestone(self) -> str | None:
        state = self.root / ".gsd" / "STATE.md"
        if not state.is_file():
            return None
        for line in state.read_text(encoding="utf-8", errors="replace").splitlines():
            match = _STATE_ACTIVE.match(line)
            if match:
                return match.group("id")
        return None

    @staticmethod
    def _preferred_active_milestone(roadmap_active: set[str]) -> str:
        """Newest ROADMAP-active id by numeric M###, then prefer suffixed ids."""
        if not roadmap_active:
            return "none"

        def sort_key(mid: str) -> tuple[int, int, str]:
            base = mid.split("-", 1)[0]
            try:
                num = int(base[1:])
            except ValueError:
                num = -1
            # Prefer suffixed (unique) ids over bare M### when same number.
            suffixed = 1 if "-" in mid else 0
            return (num, suffixed, mid)

        return max(roadmap_active, key=sort_key)

    @staticmethod
    def _parse_roadmap(path: Path) -> list[tuple[str, str, str]]:
        rows = []
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            match = _MILESTONE_ROW.match(line)
            if match:
                marker = match.group("marker")
                rows.append(
                    (
                        match.group("id"),
                        match.group("title"),
                        _STATUS_BY_MARKER[marker],
                    )
                )
        return rows

    def _phase_dir(self, milestone_id: str) -> Path | None:
        phases = self.root / ".gsd" / "phases"
        if not phases.is_dir():
            return None
        raw_id = milestone_id.removeprefix("M").lower()
        if "-" not in raw_id:
            matches = [
                entry
                for entry in phases.iterdir()
                if entry.is_dir() and entry.name.lower() == raw_id
            ]
        else:
            matches = [
                entry
                for entry in phases.iterdir()
                if entry.is_dir()
                and (
                    entry.name.lower() == raw_id
                    or entry.name.lower().startswith(raw_id + "-")
                )
            ]
        return matches[0] if len(matches) == 1 else None

    def _missing_phase_diagnostic(self, milestone_id: str) -> str:
        raw_id = milestone_id.removeprefix("M").lower()
        if "-" not in raw_id:
            phases = self.root / ".gsd" / "phases"
            shadowed = (
                sorted(
                    entry.name
                    for entry in phases.iterdir()
                    if entry.is_dir() and entry.name.lower().startswith(raw_id + "-")
                )
                if phases.is_dir()
                else []
            )
            if shadowed:
                return (
                    f"bare milestone {milestone_id} has no exact phase projection; "
                    f"refusing suffixed phase merge: {shadowed}"
                )
        return f"phase projection not found for {milestone_id}"

    @staticmethod
    def _unplanned_slices(phase_dir: Path) -> list[str]:
        """ROADMAP pending slices that still have no phase PLAN file."""
        roadmaps = sorted(phase_dir.glob("*-ROADMAP.md"))
        roadmap = roadmaps[0] if roadmaps else None
        if roadmap is None:
            return []
        pending: list[str] = []
        for line in roadmap.read_text(encoding="utf-8", errors="replace").splitlines():
            match = re.match(r"^-\s+\[ \]\s+\*\*(S\d+)\b", line)
            if not match:
                continue
            slice_id = match.group(1)
            num = slice_id[1:]
            has_plan = any(phase_dir.glob(f"*-{num}-PLAN.md"))
            if not has_plan:
                pending.append(slice_id)
        return pending

    @staticmethod
    def _task_ids(phase_dir: Path) -> set[tuple[str, str]]:
        task_ids: set[tuple[str, str]] = set()
        for plan in sorted(phase_dir.glob("*-PLAN.md")):
            for line in plan.read_text(encoding="utf-8", errors="replace").splitlines():
                match = _TASK_ROW.match(line)
                if match:
                    task_ids.add((plan.name, match.group("id")))
        return task_ids
