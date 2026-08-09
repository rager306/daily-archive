"""Application service: trajectory — builds the full trajectory report."""

import json
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
from governor.ports.gsd_planning_reader import GsdPlanningReader
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
        gsd_planning_reader: GsdPlanningReader,
        git_reader: "GitReader | None" = None,
    ):
        self.root = project_root
        self.audit = audit_service
        self.rule_repo = rule_repo
        self.adr_repo = adr_repo
        self.milestone_reader = milestone_reader
        self.gsd_planning_reader = gsd_planning_reader
        self.git_reader = git_reader

    def build_report(self, phase: str = "preflight") -> TrajectoryReport:
        audit_report = self.audit.run_audit()
        dimensions = self._build_dimensions()
        drift_flags = self._build_drift_flags(audit_report, dimensions)

        # M360+: resource safety check — disk and memory before heavy builds
        resource_flags = self._check_resource_safety()
        drift_flags.extend(resource_flags)

        # M371: reject false-green GSD evidence created after the last commit.
        drift_flags.extend(self._check_gsd_exec_evidence())

        # Gold evaluation must account for every fixture and every gold entity.
        drift_flags.extend(self._check_gold_completeness())

        # Observability is an onion-aware semantic contract, not a log-call count.
        drift_flags.extend(self._check_logging_architecture())

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

    def _check_resource_safety(self) -> list:
        """M360+: disk/memory/target size checks. Returns DriftFlag list."""
        import sys
        sys.path.insert(0, str(self.root / "scripts"))
        try:
            from governor.checks.resource_safety import preflight_check
            result = preflight_check(str(self.root))
        except Exception:
            return []

        flags = []
        from governor.domain.trajectory import DriftFlag

        # Disk check
        disk = result.get("disk", {})
        if disk.get("status") in ("warning", "critical"):
            flags.append(DriftFlag(
                flag="resource_disk_pressure",
                severity="medium" if disk["status"] == "warning" else "high",
                dimension="resource_safety",
                evidence=(
                    f"{disk['message']} Recommendation: run 'cargo clean' or remove "
                    "stale artifacts before heavy builds."
                ),
            ))

        # Memory check
        mem = result.get("memory", {})
        if mem.get("status") in ("warning", "critical"):
            flags.append(DriftFlag(
                flag="resource_memory_pressure",
                severity="medium" if mem["status"] == "warning" else "high",
                dimension="resource_safety",
                evidence=(
                    f"{mem['message']} Recommendation: close other processes and avoid "
                    "cargo test --workspace (RocksDB memory heavy)."
                ),
            ))

        # target/ size check
        target = result.get("target_dir", {})
        if target.get("status") in ("warning", "critical"):
            recs = "; ".join(target.get("recommendations", ["Run 'cargo clean'"]))
            flags.append(DriftFlag(
                flag="resource_target_bloat",
                severity="medium" if target["status"] == "warning" else "high",
                dimension="resource_safety",
                evidence=f"{target['message']} Recommendation: {recs}",
            ))

        return flags

    def _check_logging_architecture(self) -> list[DriftFlag]:
        """Require valuable pipeline logs without leaking transport into core layers."""
        crates = self.root / "crates"
        if not crates.exists():
            return []

        flags: list[DriftFlag] = []
        pure_crates = [
            "da-domain", "da-ports", "kg-ontology", "kg-pipeline", "kg-algorithms"
        ]
        boundary_violations = []
        for crate in pure_crates:
            cargo = crates / crate / "Cargo.toml"
            if cargo.exists() and re.search(
                r"(?m)^\s*(?:tracing|tracing-subscriber|tracing-appender)\s*=",
                cargo.read_text(errors="replace"),
            ):
                boundary_violations.append(str(cargo.relative_to(self.root)))
        if boundary_violations:
            flags.append(
                DriftFlag(
                    flag="logging_transport_crosses_onion_boundary",
                    severity="high",
                    evidence="subscriber/tracing dependency in pure layer: "
                    + ", ".join(boundary_violations),
                    dimension="architecture",
                )
            )

        examples_dir = crates / "da-cli" / "examples"
        missing_roots = []
        for path in sorted(examples_dir.glob("*.rs")):
            if "init_pipeline_logging" not in path.read_text(errors="replace"):
                missing_roots.append(path.name)
        if missing_roots:
            flags.append(
                DriftFlag(
                    flag="pipeline_composition_root_without_file_logging",
                    severity="high",
                    evidence="missing .env-backed rolling JSON init: " + ", ".join(missing_roots),
                    dimension="observability",
                )
            )

        pipeline = crates / "da-application" / "src" / "pipeline.rs"
        if pipeline.exists():
            text = pipeline.read_text(errors="replace")
            required = ["event", "article_id", "pipeline_id", "step", "stage"]
            missing = [field for field in required if field not in text]
            if missing:
                flags.append(
                    DriftFlag(
                        flag="pipeline_stage_logs_lack_semantic_fields",
                        severity="high",
                        evidence="pipeline lifecycle logs miss fields: " + ", ".join(missing),
                        dimension="observability",
                    )
                )

        materializers = {
            "ingest.rs": [
                'event = "parsed_article_materialized"', "paper_id",
                "sections_written", "references_written", "cites_edges_written",
            ],
            "extraction.rs": [
                'event = "extracted_entities_materialized"', "paper_id",
                "entities_created", "mentions_edges_written", "found_in_edges_written",
            ],
        }
        missing_materializer_fields = []
        for filename, required in materializers.items():
            path = crates / "da-application" / "src" / filename
            if not path.exists():
                continue
            text = path.read_text(errors="replace")
            absent = [field for field in required if field not in text]
            if absent:
                missing_materializer_fields.append(f"{filename}: {','.join(absent)}")
        if missing_materializer_fields:
            flags.append(
                DriftFlag(
                    flag="graph_materializer_logs_lack_write_deltas",
                    severity="high",
                    evidence="; ".join(missing_materializer_fields),
                    dimension="observability",
                )
            )
        return flags

    def _check_gold_completeness(self) -> list[DriftFlag]:
        """Validate source and evaluation denominators from product artifacts."""
        base = self.root / "artifacts" / "gold_corpus"
        manifest_path = base / "gold_corpus_manifest.json"
        aggregate_path = base / "eval_aggregate_all_sources.json"
        if not manifest_path.exists() and not aggregate_path.exists():
            return []

        try:
            manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else None
            aggregate = json.loads(aggregate_path.read_text()) if aggregate_path.exists() else None
        except (OSError, json.JSONDecodeError) as error:
            return [
                DriftFlag(
                    flag="gold_completeness_artifact_invalid",
                    severity="high",
                    evidence=f"cannot read gold completeness artifact: {error}",
                    dimension="extraction_coverage",
                )
            ]

        flags: list[DriftFlag] = []
        if manifest is not None:
            fixture_count = int(manifest.get("fixture_count", -1))
            ready_count = int(manifest.get("source_ready_count", -1))
            unavailable_count = int(manifest.get("unavailable_count", -1))
            if (
                fixture_count < 0
                or ready_count < 0
                or unavailable_count < 0
                or ready_count + unavailable_count != fixture_count
            ):
                flags.append(
                    DriftFlag(
                        flag="gold_manifest_counts_inconsistent",
                        severity="high",
                        evidence=(
                            f"fixture={fixture_count}, source_ready={ready_count}, "
                            f"unavailable={unavailable_count}"
                        ),
                        dimension="extraction_coverage",
                    )
                )
            elif unavailable_count > 0:
                flags.append(
                    DriftFlag(
                        flag="gold_unavailable_source_debt",
                        severity="high",
                        evidence=(
                            f"{unavailable_count}/{fixture_count} gold fixtures lack a "
                            "canonical PDF or HTML source"
                        ),
                        dimension="extraction_coverage",
                    )
                )

        if aggregate is not None:
            fixture_count = int(aggregate.get("fixture_count", -1))
            evaluated = int(aggregate.get("evaluated_count", -1))
            unavailable = int(aggregate.get("unavailable_count", -1))
            failed = int(aggregate.get("parse_or_extract_failed_count", -1))
            if min(fixture_count, evaluated, unavailable, failed) < 0 or (
                evaluated + unavailable + failed != fixture_count
            ):
                flags.append(
                    DriftFlag(
                        flag="gold_eval_denominator_incomplete",
                        severity="high",
                        evidence=(
                            f"evaluated={evaluated} + unavailable={unavailable} + "
                            f"failed={failed} != fixture={fixture_count}"
                        ),
                        dimension="extraction_coverage",
                    )
                )
            all_gold = int(aggregate.get("gold_entities_all_fixtures", -1))
            evaluated_gold = int(aggregate.get("gold_entities_evaluated", -1))
            if all_gold < 0 or evaluated_gold < 0 or evaluated_gold != all_gold:
                flags.append(
                    DriftFlag(
                        flag="gold_eval_excludes_fixture_entities",
                        severity="high",
                        evidence=(
                            f"evaluated gold entities={evaluated_gold}, "
                            f"all fixture gold entities={all_gold}"
                        ),
                        dimension="extraction_coverage",
                    )
                )
        return flags

    def _check_gsd_exec_evidence(self) -> list[DriftFlag]:
        """Flag exit-0 evidence whose captured output explicitly reports failure."""
        from datetime import datetime

        from governor.adapters.gsd_exec_reader import GsdExecReader

        since = None
        if self.git_reader and self.git_reader.is_repo():
            value = self.git_reader.get_git_info().latest_commit_date
            try:
                since = datetime.fromisoformat(value) if value else None
            except ValueError:
                since = None
        try:
            findings = GsdExecReader(self.root).false_successes_since(since)
        except OSError:
            return []
        if not findings:
            return []
        ids = [finding.evidence_id for finding in findings]
        markers = sorted({finding.marker for finding in findings})
        return [
            DriftFlag(
                flag="gsd_exec_false_success_evidence",
                severity="high",
                evidence=(
                    f"{len(findings)} exit-0 GSD exec run(s) contain explicit failure "
                    f"markers after the latest commit: ids={ids}, markers={markers}"
                ),
            )
        ]

    def _build_drift_flags(self, audit_report, dimensions) -> list[DriftFlag]:
        flags: list[DriftFlag] = []
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
        flags.extend(self._check_gsd_planning_projection())
        return flags

    def _check_gsd_planning_projection(self) -> list[DriftFlag]:
        """Detect empty active milestones through engine-rendered projections."""
        try:
            projection = self.gsd_planning_reader.read()
        except OSError as error:
            return [
                DriftFlag(
                    flag="gsd_planning_projection_unavailable",
                    severity="high",
                    evidence=f"planning projection read failed: {error}",
                )
            ]
        if not projection.available:
            details = "; ".join(projection.diagnostics) or "no diagnostic"
            return [
                DriftFlag(
                    flag="gsd_planning_projection_unavailable",
                    severity="high",
                    evidence=details,
                )
            ]
        flags: list[DriftFlag] = []
        if projection.diagnostics:
            flags.append(
                DriftFlag(
                    flag="gsd_planning_projection_degraded",
                    severity="medium",
                    evidence="; ".join(projection.diagnostics),
                )
            )
        empty = projection.active_empty_milestone_ids
        if empty:
            flags.append(
                DriftFlag(
                    flag="active_milestone_zero_projected_plan_tasks",
                    severity="medium",
                    evidence=(
                        f"{len(empty)} active milestone(s) with 0 tasks in readable "
                        f"PLAN projections: {list(empty)}; this is projection health, "
                        "not engine database truth"
                    ),
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
