"""Port: milestone reader — reads GSD milestone state."""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from pathlib import Path


@dataclass
class MilestoneInfo:
    """Metadata about a GSD milestone."""

    id: str
    title: str
    phase_dir: Path
    status: str = "unknown"
    slices_total: int = 0
    slices_complete: int = 0


class MilestoneReader(ABC):
    """Reads GSD milestone information from .gsd/ directory."""

    @abstractmethod
    def list_milestones(self, gsd_dir: str = ".gsd/") -> list[MilestoneInfo]:
        """List all milestones from phases/ directory."""
        ...

    @abstractmethod
    def get_latest_milestone(self, gsd_dir: str = ".gsd/") -> MilestoneInfo | None:
        """Get the highest-numbered milestone."""
        ...
