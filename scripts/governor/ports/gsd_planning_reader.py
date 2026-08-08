"""Port for read-only GSD planning projections.

The engine database is deliberately outside this boundary. Consumers read only
human-readable projections rendered by GSD.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass


@dataclass(frozen=True)
class MilestonePlanningProjection:
    """Planning facts visible in supported GSD filesystem projections."""

    milestone_id: str
    title: str
    status: str
    task_count: int
    source: str


@dataclass(frozen=True)
class GsdPlanningProjection:
    """A bounded planning read with explicit availability diagnostics."""

    available: bool
    milestones: tuple[MilestonePlanningProjection, ...] = ()
    diagnostics: tuple[str, ...] = ()

    @property
    def active_empty_milestone_ids(self) -> tuple[str, ...]:
        return tuple(
            milestone.milestone_id
            for milestone in self.milestones
            if milestone.status == "active" and milestone.task_count == 0
        )


class GsdPlanningReader(ABC):
    """Reads supported GSD projections without touching engine-owned storage."""

    @abstractmethod
    def read(self) -> GsdPlanningProjection:
        """Return planning facts or an explicit unavailable result."""
        ...
