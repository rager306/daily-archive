"""governor.ports — Abstract interfaces (ports) for IO operations.

Ports define WHAT the governor needs to read/scan.
Adapters implement HOW it reads the filesystem, JSON, etc.
"""

from governor.ports.adr_repository import ADRRepository
from governor.ports.absorption_progress_reader import AbsorptionProgressReader
from governor.ports.artifact_reader import ArtifactReader
from governor.ports.code_scanner import CodeScanner
from governor.ports.gsd_planning_reader import GsdPlanningReader
from governor.ports.milestone_reader import MilestoneReader
from governor.ports.rule_repository import RuleRepository

__all__ = [
    "ADRRepository",
    "AbsorptionProgressReader",
    "ArtifactReader",
    "CodeScanner",
    "GsdPlanningReader",
    "MilestoneReader",
    "RuleRepository",
]
