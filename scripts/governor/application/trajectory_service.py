"""Application service: trajectory — builds the full trajectory report."""

import re
from pathlib import Path

from governor.application.audit_service import AuditService
from governor.domain.trajectory import (
    Dimension,
    DriftFlag,
    TrajectoryReport,
    TrajectoryVerdict,
)
from governor.ports.adr_repository import ADRRepository
from governor.ports.milestone_reader import MilestoneReader
from governor.ports.rule_repository import RuleRepository


class TrajectoryService:
    """Builds a trajectory report by orchestrating domain + ports."""

    def __init__(
        self,
        project_root: Path,
        audit_service: AuditService,
        rule_repo: RuleRepository,
        adr_repo: ADRRepository,
        milestone_reader: MilestoneReader,
        git_reader: "GitReader | None" = None,
    ):
        self.root = project_root
        self.audit = audit_service
        self.rule_repo = rule_repo
        self.adr_repo = adr_repo
        self.milestone_reader = milestone_reader
        self.git_reader = git_reader

    def build_report(self, phase: str = "preflight") -> TrajectoryReport:
        audit_report = self.audit.run_audit()
        dimensions = self._build_dimensions()
        drift_flags = self._build_drift_flags(audit_report, dimensions)
        verdict = self._derive_verdict(audit_report, drift_flags)
        next_actions = self._build_next_actions(verdict, drift_flags)
        milestone_ctx = self._build_milestone_context()

        return TrajectoryReport(
            schema_version="m286.hexagonal.governor.v1",
            phase=phase,
            verdict=verdict,
            dimensions={d.id: self._dimension_to_dict(d) for d in dimensions},
            drift_flags=[self._flag_to_dict(f) for f in drift_flags],
            reverse_adr_audit_details=audit_report.to_dict(),
            next_actions=next_actions,
            milestone_context=milestone_ctx,
        )

    def _build_dimensions(self) -> list[Dimension]:
        dims_data = self.rule_repo.load_dimensions()
        dimensions = []
        for dim_data in dims_data:
            status = self._evaluate_status(dim_data.get("status_logic", {}))
            dimensions.append(
                Dimension(
                    id=dim_data["id"],
                    description=dim_data.get("description", ""),
                    status=status,
                    evidence=dim_data.get("evidence", []),
                )
            )
        return dimensions

    def _evaluate_status(self, status_logic: dict) -> str:
        for status_name, condition in status_logic.items():
            condition = condition.strip()
            if condition == "always" or condition.startswith("always ("):
                return status_name
            if self._evaluate_condition(condition):
                return status_name
        return "not_started"

    def _evaluate_condition(self, condition: str) -> bool:
        # Handle "X exists AND Y exists" patterns
        parts = [p.strip() for p in condition.split(" AND ")]
        for part in parts:
            if not self._check_single(part):
                return False
        return True

    def _check_single(self, expr: str) -> bool:
        expr = expr.strip()
        if " exists" in expr:
            path_str = expr.replace(" exists", "").strip()
            return (self.root / path_str).exists()
        if " exists AND " in expr:
            return self._evaluate_condition(expr)
        if expr.startswith("artifacts/"):
            return (self.root / expr.split(" exists")[0].strip()).exists()
        return False

    def _build_drift_flags(self, audit_report, dimensions) -> list[DriftFlag]:
        flags = []
        if audit_report.has_blocking:
            flags.append(
                DriftFlag(
                    flag="reverse_adr_audit_violations",
                    severity="high",
                    evidence=f"{audit_report.violations_count} violation(s) found",
                )
            )
        # Check if next gate is documented in README
        readme = self.root / "README.md"
        if readme.exists():
            text = readme.read_text(encoding="utf-8", errors="replace")
            if "Next gate" not in text and "next gate" not in text.lower():
                flags.append(
                    DriftFlag(
                        flag="missing_next_gate",
                        severity="medium",
                        evidence="README.md does not contain 'Next gate' section",
                    )
                )
        # Check for stale milestone (no closeout > 7 days with commits).
        if self.git_reader and self.git_reader.is_repo():
            from datetime import datetime, timedelta

            cutoff = (datetime.now() - timedelta(days=7)).strftime("%Y-%m-%d")
            recent_commits = self.git_reader.commits_since_date(cutoff)
            if recent_commits > 0:
                # Check if any milestone was closed in the last 7 days.
                gsd_dir = self.root / ".gsd" / "phases"
                recent_closeout = False
                if gsd_dir.exists():
                    for ms_dir in gsd_dir.iterdir():
                        summary = ms_dir / f"{ms_dir.name.split('-')[0]}-SUMMARY.md"
                        if summary.exists():
                            stat = summary.stat()
                            import time
                            age_days = (time.time() - stat.st_mtime) / 86400
                            if age_days < 7:
                                recent_closeout = True
                                break
                if not recent_closeout:
                    flags.append(
                        DriftFlag(
                            flag="stale_milestone",
                            severity="medium",
                            evidence=f"{recent_commits} commits in last 7 days but no milestone closeout",
                        )
                    )
        return flags

    def _derive_verdict(
        self, audit_report, drift_flags: list[DriftFlag]
    ) -> str:
        if audit_report.has_blocking:
            return TrajectoryVerdict.BLOCKED
        if any(f.severity == "high" for f in drift_flags):
            return TrajectoryVerdict.DRIFT_RISK
        if drift_flags:
            return TrajectoryVerdict.DRIFT_RISK
        return TrajectoryVerdict.ON_TRACK

    def _build_next_actions(self, verdict: str, flags: list[DriftFlag]) -> list[str]:
        actions = []
        if verdict == "blocked":
            actions.append("Fix blocking reverse-ADR violations before proceeding.")
        for flag in flags:
            if flag.flag == "missing_next_gate":
                actions.append("Add 'Next gate' section to README.md.")
        if not actions:
            actions.append("No drift detected. Continue current milestone.")
        return actions

    def _build_milestone_context(self) -> dict:
        latest = self.milestone_reader.get_latest_milestone()
        if not latest:
            return {}
        return {
            "latest_milestone_id": latest.id,
            "latest_milestone_title": latest.title,
        }

    def _dimension_to_dict(self, dim: Dimension) -> dict:
        return {
            "description": dim.description,
            "status": dim.status,
            "evidence": dim.evidence,
        }

    def _flag_to_dict(self, flag: DriftFlag) -> dict:
        return {
            "flag": flag.flag,
            "severity": flag.severity,
            "evidence": flag.evidence,
        }
