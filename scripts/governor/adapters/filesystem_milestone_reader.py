"""Adapter: filesystem milestone reader — reads GSD milestone state."""

import re
from pathlib import Path

from governor.ports.milestone_reader import MilestoneInfo, MilestoneReader


class FilesystemMilestoneReader(MilestoneReader):
    """Reads GSD milestone information from .gsd/phases/ directory."""

    def __init__(self, project_root: Path):
        self.root = project_root

    def list_milestones(self, gsd_dir: str = ".gsd/") -> list[MilestoneInfo]:
        phases_dir = self.root / gsd_dir / "phases"
        if not phases_dir.exists():
            return []
        milestones = []
        for entry in sorted(phases_dir.iterdir()):
            if not entry.is_dir():
                continue
            match = re.match(r"^(\d+)-", entry.name)
            if not match:
                continue
            ms_id = match.group(1)
            title = entry.name[len(ms_id) + 1 :]  # strip "NNN-"
            milestones.append(
                MilestoneInfo(
                    id=ms_id,
                    title=title,
                    phase_dir=entry,
                    status="unknown",
                )
            )
        return milestones

    def get_latest_milestone(self, gsd_dir: str = ".gsd/") -> MilestoneInfo | None:
        milestones = self.list_milestones(gsd_dir)
        if not milestones:
            return None
        return max(milestones, key=lambda m: int(m.id))
