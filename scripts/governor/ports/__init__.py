"""governor.ports — Abstract interfaces (ports) for IO operations.

Ports define WHAT the governor needs to read/scan.
Adapters implement HOW it reads the filesystem, JSON, etc.
"""

from governor.ports.adr_repository import ADRRepository
from governor.ports.artifact_reader import ArtifactReader
from governor.ports.code_scanner import CodeScanner
from governor.ports.milestone_reader import MilestoneReader
from governor.ports.rule_repository import RuleRepository

__all__ = [
    "ADRRepository",
    "ArtifactReader",
    "CodeScanner",
    "MilestoneReader",
    "RuleRepository",
]
