"""governor.adapters — Concrete IO implementations of ports."""

from governor.adapters.clippy_scanner import ClippyScanner, ClippyWarning, ClippyReport
from governor.adapters.absorption_progress_reader import (
    FilesystemAbsorptionProgressReader,
    absorption_progress_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)
from governor.adapters.filesystem_adr_repository import FilesystemADRRepository
from governor.adapters.filesystem_artifact_reader import FilesystemArtifactReader
from governor.adapters.filesystem_code_scanner import FilesystemCodeScanner
from governor.adapters.filesystem_gsd_planning_reader import FilesystemGsdPlanningReader
from governor.adapters.filesystem_milestone_reader import FilesystemMilestoneReader
from governor.adapters.git_reader import GitReader
from governor.adapters.yaml_rule_repository import YAMLRuleRepository

__all__ = [
    "ClippyReport",
    "ClippyScanner",
    "ClippyWarning",
    "FilesystemAbsorptionProgressReader",
    "FilesystemADRRepository",
    "FilesystemArtifactReader",
    "FilesystemCodeScanner",
    "FilesystemGsdPlanningReader",
    "FilesystemMilestoneReader",
    "GitReader",
    "YAMLRuleRepository",
    "absorption_progress_violations",
    "effective_progress_contract",
    "missing_progress_vocabulary",
]
