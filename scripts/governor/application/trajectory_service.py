"""Application service: trajectory — builds the full trajectory report."""

import hashlib
import json
import re
from datetime import datetime, timedelta

import yaml
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



# PIR-008: RFC3339 shape guard for the canonical temporal wire contract.
# Mirrors `kg_ontology::temporal::validate_datetime_wire`: a canonical value
# is an RFC3339 date-time with a UTC designator (Z or +00:00), the OPEN
# sentinel, or JSON null. Fractional seconds are allowed; a space separator,
# missing offset, or non-UTC offset is rejected.
_RFC3339_UTC_RE = re.compile(
    r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$"
)


def _wire_value_accepted(value) -> bool:
    """True when `validate_datetime_wire` accepts the JSON value."""
    if value is None:
        return True
    if isinstance(value, str):
        if value == "OPEN":
            return True
        if not _RFC3339_UTC_RE.match(value):
            return False
        try:
            parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
        except ValueError:
            return False
        return parsed.utcoffset() == timedelta(0)
    return False


def _validate_html_projection(projection, required_fields, section_fields) -> list[str]:
    """Validate a positive DocumentContextProjection fixture (PIR-010).

    Mirrors `da_application::DocumentContextStage` semantics: every section
    must carry a non-empty `path_id`, all paths must be unique, each declared
    `parent_section_id` must equal `sec-<dotted parent of path_id>` (dot-free
    `~N` restart suffixes never introduce a hierarchy level), and the
    `previous_path_id` / `next_path_id` chains must follow document order.
    Returns a list of contract problems (empty = honest).
    """
    problems: list[str] = []
    missing = [f for f in required_fields if f not in projection]
    if missing:
        return [f"missing required fields: {missing}"]
    sections = projection.get("sections")
    if not isinstance(sections, list) or not sections:
        return ["sections must be a non-empty array"]
    paths: list[str] = []
    for index, section in enumerate(sections):
        if not isinstance(section, dict):
            problems.append(f"sections[{index}] is not an object")
            continue
        missing_sec = [f for f in section_fields if f not in section]
        if missing_sec:
            problems.append(f"sections[{index}] missing fields: {missing_sec}")
        path_id = section.get("path_id")
        if not isinstance(path_id, str) or not path_id:
            problems.append(f"sections[{index}] has no path_id")
            continue
        paths.append(path_id)
        # Parent identity must be the last-dot prefix (restart suffixes are
        # dot-free and never become dotted parents).
        parent_path = path_id.rpartition(".")[0]
        expected_parent = f"sec-{parent_path}" if parent_path else None
        parent = section.get("parent_section_id")
        if parent is not None and parent != expected_parent:
            problems.append(
                f"sections[{index}] parent_section_id {parent!r} inconsistent "
                f"with path {path_id!r} (expected {expected_parent!r})"
            )
        # prev/next chains follow document order (stage contract).
        prev_expected = sections[index - 1].get("path_id") if index > 0 else None
        if section.get("previous_path_id") != prev_expected:
            problems.append(
                f"sections[{index}] previous_path_id must be {prev_expected!r}, "
                f"got {section.get('previous_path_id')!r}"
            )
        next_expected = (
            sections[index + 1].get("path_id") if index + 1 < len(sections) else None
        )
        if section.get("next_path_id") != next_expected:
            problems.append(
                f"sections[{index}] next_path_id must be {next_expected!r}, "
                f"got {section.get('next_path_id')!r}"
            )
    if len(paths) != len(set(paths)):
        problems.append("path_id values are not unique")
    return problems


def _validate_html_blocked_record(record) -> list[str]:
    """Validate a negative blocked/not_claimed fixture (PIR-010, D127).

    Flat HTML without path identity must stay blocked: `ok=false`,
    `readiness` must be `not_claimed` (or `blocked`), a typed `reason` must
    name the missing identity, and `import_eligible` must stay false (D127
    lock). Returns a list of contract problems (empty = honest).
    """
    problems: list[str] = []
    if record.get("ok") is not False:
        problems.append("blocked record must have ok=false")
    readiness = record.get("readiness")
    if readiness not in ("not_claimed", "blocked"):
        problems.append(
            f"readiness must stay not_claimed/blocked, got {readiness!r}"
        )
    if not record.get("reason"):
        problems.append(
            "blocked record must carry a typed reason (missing_section_path_id)"
        )
    if record.get("import_eligible") is not False:
        problems.append("import_eligible must stay false (D127)")
    return problems


def _promotion_decision_violations(decision: dict, contract: dict) -> set[str]:
    """Return violation codes for one PromotionDecision record (PIR-015).

    Mirrors `da_domain::promotion::PromotionDecision::validate` plus
    `PromotionCandidate::validate`: fail-closed decision integrity
    (transition must equal candidate.state, committed_at must be RFC3339),
    mandatory durable evidence references (except on rejected candidates),
    the D127 `import_eligible` lock, per-state actor shape, and actor
    independence (verifier differs from proposer, reviewer differs from
    proposer/verifier, promoter differs from all three). Returns a set of
    violation codes (empty = honest).
    """
    violations: set[str] = set()
    req = set((contract.get("decision_contract") or {}).get("required_fields", []))
    if not req.issubset(decision):
        return {"missing_decision_fields"}
    candidate = decision.get("candidate") or {}
    creq = set((contract.get("candidate_contract") or {}).get("required_fields", []))
    if not creq.issubset(candidate):
        return {"missing_candidate_fields"}

    if not str(decision.get("decision_id") or "").strip():
        violations.add("empty_decision_id")
    if decision.get("transition") != candidate.get("state"):
        violations.add("transition_mismatch")
    committed_at = decision.get("committed_at")
    if not isinstance(committed_at, str) or not _RFC3339_UTC_RE.match(committed_at):
        violations.add("invalid_timestamp")
    else:
        # Mirror `chrono::DateTime::parse_from_rfc3339`: the shape regex
        # alone accepts lexically valid but impossible dates (e.g. month 13).
        try:
            parsed = datetime.fromisoformat(committed_at.replace("Z", "+00:00"))
            if parsed.utcoffset() != timedelta(0):
                violations.add("invalid_timestamp")
        except ValueError:
            violations.add("invalid_timestamp")

    if not str(candidate.get("candidate_id") or "").strip():
        violations.add("empty_candidate_id")
    if candidate.get("import_eligible") is True:
        violations.add("import_leak")

    state = candidate.get("state")
    except_states = set(
        (contract.get("candidate_contract") or {}).get("evidence_required_except", [])
    )
    evidence = candidate.get("evidence_refs") or []
    if state not in except_states:
        if not isinstance(evidence, list) or not evidence:
            violations.add("missing_evidence")
        elif any(
            not isinstance(ref, dict) or not str(ref.get("reference") or "").strip()
            for ref in evidence
        ):
            violations.add("missing_evidence")

    proposer = candidate.get("proposer")
    if not str(proposer or "").strip():
        violations.add("empty_actor")

    shape = (contract.get("candidate_contract") or {}).get("state_shape") or {}
    for field, requirement in (shape.get(state) or {}).items():
        value = candidate.get(field)
        if requirement == "absent" and value not in (None, "", False):
            violations.add("invalid_state_shape")
        if requirement == "present" and value in (None, ""):
            violations.add("invalid_state_shape")

    verifier = candidate.get("verifier")
    reviewer = candidate.get("reviewer")
    promoter = candidate.get("promoter")
    for actor in (verifier, reviewer, promoter):
        if actor is not None and not str(actor).strip():
            violations.add("empty_actor")

    if verifier and verifier == proposer:
        violations.add("self_confirmation")
    if reviewer and (reviewer == proposer or (verifier and reviewer == verifier)):
        violations.add("reviewer_not_independent")
    if promoter and (
        promoter == proposer
        or (verifier and promoter == verifier)
        or (reviewer and promoter == reviewer)
    ):
        violations.add("promoter_not_independent")
    return violations


def _promotion_journal_violations(journal: list, contract: dict) -> set[str]:
    """Return journal-level violation codes for a decision sequence (PIR-015).

    Mirrors the terminal-state guards of the domain FSM: a candidate may be
    promoted at most once (`double_promotion`, AlreadyPromoted) and terminal
    states are immutable (`terminal_mutation`) — any decision that differs
    from the previous one for the same candidate after a terminal state is a
    mutation. Byte-identical idempotent replays remain allowed.
    """
    violations: set[str] = set()
    terminal_seen: set[str] = set()
    promote_count: dict[str, int] = {}
    previous: dict[str, dict] = {}
    for decision in journal:
        candidate = decision.get("candidate") or {}
        candidate_id = candidate.get("candidate_id")
        state = candidate.get("state")
        if state == "promoted":
            promote_count[candidate_id] = promote_count.get(candidate_id, 0) + 1
            if promote_count[candidate_id] > 1:
                violations.add("double_promotion")
        if candidate_id in terminal_seen and decision != previous.get(candidate_id):
            violations.add("terminal_mutation")
        if state in ("promoted", "rejected"):
            terminal_seen.add(candidate_id)
        previous[candidate_id] = decision
    return violations


def _acap_hard_failures_nonzero(value) -> bool:
    """True when ACAP hard_failures indicates a failure.

    Product reports emit list[str] codes. Legacy governor fixtures used a count.
    Empty list and numeric 0 are both pass; non-empty list or non-zero count fail.
    """
    if value is None:
        return False
    if isinstance(value, list):
        return len(value) > 0
    if isinstance(value, (int, float)):
        return value != 0
    if isinstance(value, str):
        return value.strip() not in ("", "0")
    return bool(value)


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

        # Boundary contracts prevent silent loss of canonical and derived information.
        drift_flags.extend(self._check_information_preservation())

        # ACAP validates multi-layer FSM, metrics, cache/prompt/optimizer and reports.
        drift_flags.extend(self._check_article_context_assurance())

        # PIR-006: section navigation report contract — positive/negative
        # fixtures keep the corrected deep-tree navigation contract enforced.
        drift_flags.extend(self._check_section_navigation_contract())

        # PIR-008: canonical temporal wire format contract — ontology DATA
        # markers plus positive/negative wire fixtures keep the RFC3339 UTC
        # DateTime wire decision enforced (integer writes rejected, legacy
        # reads bounded to the compatibility decoder).
        drift_flags.extend(self._check_canonical_temporal_wire_format())

        # PIR-009/PIR-010: HTML structure honesty contract — positive
        # document-context projection and negative blocked/not_claimed
        # fixtures keep HTML readiness honest: proven paths may claim
        # structure, missing identity stays blocked or not_claimed, and
        # D127 stays locked.
        drift_flags.extend(self._check_html_structure_honesty())

        # PIR-015: promotion evidence contract — positive/negative committed
        # PromotionDecision fixtures keep the authority-neutral candidate
        # promotion protocol enforced (no self-confirmation, no double
        # promotion, mandatory evidence, terminal immutability, D127 lock).
        drift_flags.extend(self._check_promotion_evidence_contract())

        verdict = self._derive_verdict(audit_report, drift_flags, phase)
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

    def _check_article_context_assurance(self) -> list[DriftFlag]:
        """Validate the data-driven Article Context Assurance Plane policy/report."""
        policy_path = self.root / "scripts/governor/checks/article_context_assurance.yaml"
        if not policy_path.exists():
            return []
        try:
            raw = policy_path.read_bytes()
            policy = yaml.safe_load(raw) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="acap_policy_invalid", severity="high",
                evidence=f"cannot load ACAP policy: {error}", dimension="article_context_assurance",
            )]
        errors = []
        metrics = set((policy.get("metric_definitions") or {}).keys())
        layers = policy.get("layers") or []
        layer_ids = [str(layer.get("id")) for layer in layers]
        if len(layer_ids) != len(set(layer_ids)) or not layer_ids:
            errors.append("layer IDs must be non-empty and unique")
        fsm = policy.get("fsm") or {}
        states = [str(state) for state in fsm.get("states", [])]
        state_set = set(states)
        if len(states) != len(state_set) or not states:
            errors.append("FSM states must be non-empty and unique")
        if fsm.get("initial_state") not in state_set:
            errors.append("FSM initial_state is unknown")
        for terminal in fsm.get("terminal_states", []):
            if terminal not in state_set:
                errors.append(f"unknown terminal state {terminal}")
        for transition in fsm.get("transitions", []):
            if not isinstance(transition, list) or len(transition) != 2 or any(state not in state_set for state in transition):
                errors.append(f"illegal FSM transition {transition}")
        for layer in layers:
            layer_id = layer.get("id")
            if layer.get("state") not in state_set:
                errors.append(f"{layer_id}: unknown FSM state")
            if layer.get("enforcement") not in ("required", "bootstrap"):
                errors.append(f"{layer_id}: enforcement must be required/bootstrap")
            for metric in list(layer.get("hard_metrics", [])) + list(layer.get("quality_metrics", [])):
                if metric not in metrics:
                    errors.append(f"{layer_id}: unknown metric {metric}")
        principles = policy.get("principles") or {}
        for lock in ("canonical_truth_is_lossless", "optimizer_outputs_are_candidates_only", "import_eligible_default"):
            expected = False if lock == "import_eligible_default" else True
            if principles.get(lock) is not expected:
                errors.append(f"principle {lock} must be {expected}")
        optimizer = policy.get("optimizer_admission") or {}
        if optimizer.get("candidate_only") is not True or optimizer.get("auto_promote") is not False:
            errors.append("optimizer candidates must be candidate-only with auto_promote=false")
        hard_gates = optimizer.get("hard_gates") or {}
        for gate in ("same_n", "dual_f1_non_regression", "no_hard_invariant_regression", "no_test_contamination", "human_decision_required"):
            if hard_gates.get(gate) is not True:
                errors.append(f"optimizer hard gate {gate} must be true")
        prompt = policy.get("prompt_contract") or {}
        if prompt.get("registry_is_data") is not True or prompt.get("output_must_validate_before_materialization") is not True:
            errors.append("prompt registry and pre-materialization validation are mandatory")
        if errors:
            return [DriftFlag(
                flag="acap_policy_invalid", severity="high",
                evidence="; ".join(errors[:20]), dimension="article_context_assurance",
            )]

        report_path = self.root / str((policy.get("assurance_report_contract") or {}).get("path", ""))
        required_layers = {str(layer["id"]) for layer in layers if layer.get("enforcement") == "required"}
        if not report_path.is_file():
            if required_layers and policy.get("rollout") != "bootstrap":
                return [DriftFlag(
                    flag="acap_report_missing", severity="high",
                    evidence=f"required ACAP report missing: {report_path}", dimension="article_context_assurance",
                )]
            return []
        try:
            report = json.loads(report_path.read_text())
        except (OSError, json.JSONDecodeError) as error:
            return [DriftFlag(
                flag="acap_report_invalid", severity="high", evidence=str(error),
                dimension="article_context_assurance",
            )]
        contract = policy.get("assurance_report_contract") or {}
        failures: list[str] = []
        coverage_gaps: list[str] = []
        required_top = contract.get("required_top_level", [])
        missing_top = [key for key in required_top if key not in report]
        if missing_top:
            failures.append(f"missing report keys {missing_top}")
        expected_schema = contract.get("schema_version")
        if expected_schema and report.get("schema_version") != expected_schema:
            failures.append(
                f"schema_version mismatch: report={report.get('schema_version')!r} "
                f"expected={expected_schema!r}"
            )
        expected_hash = hashlib.sha256(raw).hexdigest()
        report_policy_sha = report.get("policy_sha256")
        # Product ContentDigest may be a plain hex string.
        if isinstance(report_policy_sha, dict):
            report_policy_sha = report_policy_sha.get("sha256") or report_policy_sha.get("hex")
        if report_policy_sha != expected_hash:
            failures.append("policy_sha256 mismatch")
        if report.get("import_eligible") is True:
            failures.append("import_eligible must be false")

        results = {str(item.get("layer_id")): item for item in report.get("layer_results", [])}
        # Product reports include no_claim rows for required layers; absence of a
        # row is still a structural failure. Coverage is judged via status + codes.
        missing_required = sorted(required_layers - set(results))
        if missing_required:
            failures.append(f"missing required layers {missing_required}")
        for layer_id, result in results.items():
            if layer_id not in set(layer_ids):
                failures.append(f"unknown report layer {layer_id}")
            # Product shape is list[str]; legacy fixtures may still use a count.
            if _acap_hard_failures_nonzero(result.get("hard_failures", 0)):
                failures.append(f"{layer_id}: hard_failures={result.get('hard_failures')}")

        aggregate = report.get("aggregate") or {}
        aggregate_required = contract.get("aggregate_required", [])
        missing_agg = [key for key in aggregate_required if key not in aggregate]
        if missing_agg:
            failures.append(f"missing aggregate keys {missing_agg}")
        if aggregate.get("replay_net_writes", 0) != 0:
            failures.append("aggregate replay_net_writes must be zero")
        if aggregate.get("d127_unlock_count", 0) != 0:
            failures.append("aggregate d127_unlock_count must be zero")

        rollout = str(policy.get("rollout") or "bootstrap")
        coverage_prefixes = list(
            contract.get("bootstrap_coverage_hard_failure_prefixes") or []
        )
        agg_hard = aggregate.get("hard_failures", 0)
        # Normalize product list / legacy count into code list for classification.
        if isinstance(agg_hard, list):
            hard_codes = [str(code) for code in agg_hard]
        elif _acap_hard_failures_nonzero(agg_hard):
            hard_codes = [f"legacy_hard_failures_count:{agg_hard}"]
        else:
            hard_codes = []

        for code in hard_codes:
            if rollout == "bootstrap" and any(code.startswith(prefix) for prefix in coverage_prefixes):
                coverage_gaps.append(code)
            else:
                failures.append(f"aggregate hard_failures:{code}")

        # Product verdict is advisory cross-check: Fail with only bootstrap coverage
        # codes should not invent PASS, but also must not block trajectory under
        # bootstrap. Integrity Fail always blocks.
        product_verdict = str(report.get("verdict") or "").lower()
        if product_verdict == "pass" and hard_codes:
            failures.append(
                f"product verdict pass contradicts hard_failures {hard_codes[:5]}"
            )

        flags: list[DriftFlag] = []
        if failures:
            flags.append(DriftFlag(
                flag="acap_report_failed",
                severity="high",
                evidence="; ".join(failures[:20]),
                dimension="article_context_assurance",
            ))
        if coverage_gaps and not failures:
            flags.append(DriftFlag(
                flag="acap_coverage_gap",
                severity="medium",
                evidence=(
                    f"bootstrap coverage gaps (not trajectory-blocking): "
                    f"{'; '.join(coverage_gaps[:20])}"
                ),
                dimension="article_context_assurance",
            ))
        return flags

    def _check_information_preservation(self) -> list[DriftFlag]:
        """Evaluate data-driven source-to-sink fidelity contracts."""
        path = self.root / "scripts/governor/checks/information_preservation.yaml"
        if not path.exists():
            return []
        try:
            payload = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="information_preservation_contract_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="information_preservation",
            )]
        flags = []
        for contract in payload.get("contracts", []):
            contract_id = str(contract.get("id", "unnamed"))
            sink = self.root / str(contract.get("sink", ""))
            if not sink.is_file():
                flags.append(DriftFlag(
                    flag="information_preservation_sink_missing",
                    severity=str(contract.get("severity", "high")),
                    evidence=f"{contract_id}: missing sink {sink}",
                    dimension="information_preservation",
                ))
                continue
            text = sink.read_text(errors="replace")
            missing = [pattern for pattern in contract.get("required_patterns", []) if not re.search(pattern, text, re.MULTILINE | re.DOTALL)]
            forbidden = [pattern for pattern in contract.get("forbidden_patterns", []) if re.search(pattern, text, re.MULTILINE | re.DOTALL)]
            if missing or forbidden:
                flags.append(DriftFlag(
                    flag="information_preservation_contract_broken",
                    severity=str(contract.get("severity", "high")),
                    evidence=(f"{contract_id} [{contract.get('boundary')} mode={contract.get('mode')}]: "
                              f"missing={missing}; forbidden={forbidden}"),
                    dimension="information_preservation",
                ))
        return flags

    def _check_section_navigation_contract(self) -> list[DriftFlag]:
        """Evaluate the data-driven section navigation report contract (PIR-006).

        Reads `scripts/governor/checks/section_navigation_contract.yaml` (DATA)
        and validates the committed NavigationReport fixtures: the positive
        complete-tree fixture must be `ok: true` with zero diagnostics; the
        negative broken-tree fixture must be `ok: false` and report every
        required NAV-* code. The graph-task template expressing the same
        contract must also stay present. The Rust test
        `navigation_report_fixtures_match_committed_governor_data` pins the
        fixtures to the current validator output, so a broken contract cannot
        silently survive a regenerate.
        """
        path = self.root / "scripts/governor/checks/section_navigation_contract.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="section_navigation_contract_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="temporal_ast_navigation",
            )]
        flags: list[DriftFlag] = []
        dimension = str(contract.get("dimension", "temporal_ast_navigation"))
        severity = str(contract.get("severity", "high"))
        required_fields = list((contract.get("report_contract") or {}).get("required_fields", []))

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        for polarity in ("positive", "negative"):
            fixture = (contract.get("fixtures") or {}).get(polarity) or {}
            fixture_path = self.root / str(fixture.get("path", ""))
            if not fixture_path.is_file():
                _flag("section_navigation_contract_fixture_missing",
                      f"{polarity} fixture missing: {fixture_path}")
                continue
            try:
                report = json.loads(fixture_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                _flag("section_navigation_contract_fixture_invalid",
                      f"{polarity} fixture unreadable: {error}")
                continue
            missing = [f for f in required_fields if f not in report]
            if missing:
                _flag("section_navigation_contract_broken",
                      f"{polarity} fixture {fixture.get('id')} missing required fields: {missing}")
                continue
            expect = fixture.get("expect") or {}
            if polarity == "positive":
                if report.get("ok") is not True:
                    _flag("section_navigation_contract_broken",
                          f"positive complete-tree fixture must be ok=true")
                if report.get("diagnostic_count") != 0:
                    _flag("section_navigation_contract_broken",
                          f"positive complete-tree fixture must have zero diagnostics, "
                          f"got diagnostic_count={report.get('diagnostic_count')}")
            else:
                if report.get("ok") is not False:
                    _flag("section_navigation_contract_broken",
                          f"negative broken-tree fixture must be ok=false")
                min_diags = int(expect.get("diagnostic_count_min", 1) or 1)
                if int(report.get("diagnostic_count") or 0) < min_diags:
                    _flag("section_navigation_contract_broken",
                          f"negative broken-tree fixture must report at least {min_diags} "
                          f"diagnostic(s), got {report.get('diagnostic_count')}")
                counts = report.get("counts") or {}
                missing_codes = [
                    code for code in expect.get("require_codes", [])
                    if int(counts.get(code, 0)) <= 0
                ]
                if missing_codes:
                    _flag("section_navigation_contract_broken",
                          f"negative broken-tree fixture missing required diagnostic codes: "
                          f"{missing_codes}")

        suite = contract.get("graph_task_suite") or {}
        suite_path = self.root / str(suite.get("path", ""))
        if not suite_path.is_file():
            _flag("section_navigation_contract_suite_missing",
                  f"navigation graph task suite missing: {suite_path}")
        else:
            text = suite_path.read_text(errors="replace")
            missing = [p for p in suite.get("required_patterns", []) if p not in text]
            if missing:
                _flag("section_navigation_contract_suite_broken",
                      f"navigation graph task suite missing required markers: {missing}")
        return flags

    def _check_canonical_temporal_wire_format(self) -> list[DriftFlag]:
        """Evaluate the data-driven canonical temporal wire contract (PIR-008).

        Reads `scripts/governor/checks/canonical_temporal_wire_format.yaml`
        (DATA). The ontology DATA file (`data/ontology/node_schemas.yaml`)
        must carry the `temporal_wire_contract` markers; the positive wire
        fixture must contain only values the write validator accepts
        (canonical RFC3339 UTC strings, OPEN, null); the negative fixture
        must contain only values the write validator rejects (integer
        timestamps, non-UTC offsets, non-RFC3339 strings, wrong types). The
        Rust test `temporal_wire_fixtures_match_committed_governor_data` in
        `crates/kg-ontology/src/temporal.rs` pins the fixtures to the actual
        validator, so a broken contract cannot silently survive a
        regenerate. The classification predicate `_wire_value_accepted`
        mirrors `kg_ontology::temporal::validate_datetime_wire`.
        """
        path = self.root / "scripts/governor/checks/canonical_temporal_wire_format.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="canonical_temporal_wire_format_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="temporal_ast",
            )]
        flags: list[DriftFlag] = []
        dimension = str(contract.get("dimension", "temporal_ast"))
        severity = str(contract.get("severity", "high"))

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        onto = contract.get("ontology_data") or {}
        onto_path = self.root / str(onto.get("path", ""))
        if not onto_path.is_file():
            _flag("canonical_temporal_wire_format_ontology_data_missing",
                  f"ontology DATA missing: {onto_path}")
        else:
            text = onto_path.read_text(errors="replace")
            missing = [m for m in onto.get("required_markers", []) if m not in text]
            if missing:
                _flag("canonical_temporal_wire_format_ontology_marker_missing",
                      f"ontology DATA missing contract markers: {missing}")

        for polarity, expected in (("positive", True), ("negative", False)):
            fixture = (contract.get("wire_fixtures") or {}).get(polarity) or {}
            fixture_path = self.root / str(fixture.get("path", ""))
            if not fixture_path.is_file():
                _flag("canonical_temporal_wire_format_fixture_missing",
                      f"{polarity} fixture missing: {fixture_path}")
                continue
            try:
                values = json.loads(fixture_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                _flag("canonical_temporal_wire_format_fixture_invalid",
                      f"{polarity} fixture unreadable: {error}")
                continue
            if not isinstance(values, list) or not values:
                _flag("canonical_temporal_wire_format_fixture_invalid",
                      f"{polarity} fixture must be a non-empty JSON array")
                continue
            bad = [v for v in values if _wire_value_accepted(v) != expected]
            if bad:
                _flag("canonical_temporal_wire_format_broken",
                      f"{polarity} fixture {fixture.get('id')} violates the wire contract: "
                      f"{bad}")
        return flags

    def _check_html_structure_honesty(self) -> list[DriftFlag]:
        """Evaluate the data-driven HTML structure honesty contract (PIR-009/PIR-010).

        Reads `scripts/governor/checks/html_structure_honesty.yaml` (DATA).
        The positive fixture is a DocumentContextProjection rendered by
        `da_application::DocumentContextStage` from HTML sections that carry
        the full derived identity: paths must be unique, every section must
        have a `path_id`, each declared parent must be the dotted parent of
        that path (dot-free `~N` restart suffixes never become dotted
        parents), and prev/next chains must follow document order. The
        negative fixture models the fail-closed outcome for flat HTML:
        ok=false, readiness stays `not_claimed`/`blocked`, a typed reason
        names the missing identity, and `import_eligible` stays false (D127).
        The Rust tests
        `document_context_stage_fixtures_match_committed_governor_data` and
        `document_context_stage_negative_fixture_models_blocked_html_readiness`
        pin the fixtures to the current stage behavior, so a broken contract
        cannot silently survive a regenerate. The readiness surface
        (`article_load.rs`) must keep stamping HTML/textbook sources as
        `graph_readiness = "not_claimed"`.
        """
        path = self.root / "scripts/governor/checks/html_structure_honesty.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="html_structure_honesty_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="html_structure_ast",
            )]
        flags: list[DriftFlag] = []
        dimension = str(contract.get("dimension", "html_structure_ast"))
        severity = str(contract.get("severity", "high"))

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        report_contract = contract.get("report_contract") or {}
        required_fields = list(report_contract.get("required_fields", []))
        section_fields = list(report_contract.get("section_required_fields", []))

        for polarity in ("positive", "negative"):
            fixture = (contract.get("fixtures") or {}).get(polarity) or {}
            fixture_path = self.root / str(fixture.get("path", ""))
            if not fixture_path.is_file():
                _flag("html_structure_honesty_fixture_missing",
                      f"{polarity} fixture missing: {fixture_path}")
                continue
            try:
                payload = json.loads(fixture_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                _flag("html_structure_honesty_fixture_invalid",
                      f"{polarity} fixture unreadable: {error}")
                continue
            if polarity == "positive":
                problems = _validate_html_projection(payload, required_fields, section_fields)
            else:
                problems = _validate_html_blocked_record(payload)
            if problems:
                _flag("html_structure_honesty_broken",
                      f"{polarity} fixture {fixture.get('id')} violates the contract: "
                      f"; ".join(problems))

        markers = contract.get("readiness_markers") or {}
        markers_path = self.root / str(markers.get("path", ""))
        if not markers_path.is_file():
            _flag("html_structure_honesty_readiness_marker_missing",
                  f"readiness surface missing: {markers_path}")
        else:
            text = markers_path.read_text(errors="replace")
            missing = [m for m in markers.get("required_patterns", []) if m not in text]
            if missing:
                _flag("html_structure_honesty_readiness_marker_missing",
                      f"readiness surface missing honesty markers: {missing}")
        return flags

    def _check_promotion_evidence_contract(self) -> list[DriftFlag]:
        """Evaluate the data-driven promotion evidence contract (PIR-015).

        Reads `scripts/governor/checks/promotion_evidence_contract.yaml`
        (DATA) and validates the committed PromotionDecision fixtures against
        the authority-neutral promotion protocol implemented in
        `da_domain::promotion`: every positive decision must be clean across
        all five states; every negative record must trigger exactly its
        labelled violation (missing evidence, D127 import leak,
        self-confirmation, non-independent reviewer/promoter, transition
        mismatch, invalid timestamp, double promotion, terminal mutation) and
        every required violation must appear in the fixture; the domain FSM
        and the application commit service must keep the typed guard
        markers. The Rust domain tests and proptests
        (`crates/da-domain/src/promotion.rs`) plus the fail-closed contract
        test (`crates/da-application/tests/promotion_contract_test.rs`)
        implement the same guards, so a broken contract cannot silently
        survive.
        """
        path = self.root / "scripts/governor/checks/promotion_evidence_contract.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="promotion_evidence_contract_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="candidate_promotion",
            )]
        flags: list[DriftFlag] = []
        dimension = str(contract.get("dimension", "candidate_promotion"))
        severity = str(contract.get("severity", "high"))

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        # Positive fixture: every committed decision must be clean.
        positive = (contract.get("fixtures") or {}).get("positive") or {}
        positive_path = self.root / str(positive.get("path", ""))
        if not positive_path.is_file():
            _flag("promotion_evidence_contract_fixture_missing",
                  f"positive fixture missing: {positive_path}")
        else:
            try:
                decisions = json.loads(positive_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                _flag("promotion_evidence_contract_fixture_invalid",
                      f"positive fixture unreadable: {error}")
            else:
                if not isinstance(decisions, list) or not decisions:
                    _flag("promotion_evidence_contract_fixture_invalid",
                          "positive fixture must be a non-empty JSON array")
                else:
                    broken = []
                    for decision in decisions:
                        violations = _promotion_decision_violations(decision, contract)
                        if violations:
                            broken.append((
                                decision.get("decision_id"),
                                sorted(violations),
                            ))
                    if broken:
                        _flag("promotion_evidence_contract_broken",
                              f"positive fixture {positive.get('id')} must be clean, "
                              f"got violations: {broken}")

        # Negative fixture: each record must trigger its labelled violation
        # and every required violation must appear somewhere.
        negative = (contract.get("fixtures") or {}).get("negative") or {}
        negative_path = self.root / str(negative.get("path", ""))
        if not negative_path.is_file():
            _flag("promotion_evidence_contract_fixture_missing",
                  f"negative fixture missing: {negative_path}")
        else:
            try:
                records = json.loads(negative_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                _flag("promotion_evidence_contract_fixture_invalid",
                      f"negative fixture unreadable: {error}")
            else:
                if not isinstance(records, list) or not records:
                    _flag("promotion_evidence_contract_fixture_invalid",
                          "negative fixture must be a non-empty JSON array")
                else:
                    observed: set[str] = set()
                    mismatches = []
                    for record in records:
                        label = str(record.get("violation", "unnamed"))
                        if "decision" in record:
                            violations = _promotion_decision_violations(
                                record.get("decision") or {}, contract)
                        elif "journal" in record:
                            violations = _promotion_journal_violations(
                                record.get("journal") or [], contract)
                        else:
                            violations = {"missing_record_shape"}
                        observed.update(violations)
                        if label not in violations:
                            mismatches.append(
                                f"{label}: expected to trigger {label}, "
                                f"got {sorted(violations)}"
                            )
                    if mismatches:
                        _flag("promotion_evidence_contract_broken",
                              f"negative fixture {negative.get('id')} records do not "
                              f"trigger their labelled violations: {'; '.join(mismatches)}")
                    require = list(
                        (negative.get("expect") or {}).get("require_violations", [])
                    )
                    missing_required = [v for v in require if v not in observed]
                    if missing_required:
                        _flag("promotion_evidence_contract_broken",
                              f"negative fixture {negative.get('id')} missing required "
                              f"violation coverage: {missing_required}")

        # Source markers: the Rust enforcement points must stay present.
        for marker_key, entry in (contract.get("source_markers") or {}).items():
            marker_path = self.root / str(entry.get("path", ""))
            if not marker_path.is_file():
                _flag("promotion_evidence_contract_source_marker_missing",
                      f"{marker_key} source missing: {marker_path}")
                continue
            text = marker_path.read_text(errors="replace")
            missing = [p for p in entry.get("required_patterns", []) if p not in text]
            if missing:
                _flag("promotion_evidence_contract_source_marker_missing",
                      f"{marker_key} source missing enforcement markers: {missing}")
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
            records = manifest.get("records")
            if records is not None:
                invalid_records = []
                if not isinstance(records, list) or len(records) != fixture_count:
                    invalid_records.append(
                        f"records={len(records) if isinstance(records, list) else 'non-list'} "
                        f"fixture={fixture_count}"
                    )
                else:
                    for record in records:
                        paper_id = str(record.get("paper_id", "unknown"))
                        source = record.get("source") or {}
                        kind = source.get("kind")
                        path = source.get("path")
                        sha256 = source.get("sha256")
                        reason = source.get("unavailable_reason")
                        if kind in ("pdf", "html"):
                            candidate = self.root / str(path) if path else None
                            if (
                                not path
                                or not isinstance(sha256, str)
                                or len(sha256) != 64
                                or reason is not None
                                or candidate is None
                                or not candidate.is_file()
                            ):
                                invalid_records.append(f"{paper_id}: invalid ready source")
                        elif kind == "unavailable":
                            if path is not None or sha256 is not None or not reason:
                                invalid_records.append(f"{paper_id}: invalid unavailable source")
                        else:
                            invalid_records.append(f"{paper_id}: unknown source kind {kind}")
                if invalid_records:
                    flags.append(
                        DriftFlag(
                            flag="gold_manifest_record_integrity_invalid",
                            severity="high",
                            evidence="; ".join(invalid_records[:10]),
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
        self, audit_report, drift_flags: list[DriftFlag], phase: str = "preflight"
    ) -> str:
        if audit_report.has_blocking:
            return TrajectoryVerdict.BLOCKED
        severity_path = self.root / "scripts/governor/checks/assurance_severity.yaml"
        if severity_path.is_file():
            try:
                policy = yaml.safe_load(severity_path.read_text()) or {}
                dimensions = set(policy.get("blocking_dimensions", []))
                severity = policy.get("blocking_severity", "high")
                phases = set(policy.get("phases", []))
                if phase in phases and any(
                    flag.severity == severity and flag.dimension in dimensions
                    for flag in drift_flags
                ):
                    return TrajectoryVerdict.BLOCKED
            except (OSError, yaml.YAMLError):
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
            if flag.flag == "resource_target_bloat":
                actions.append(
                    "target/ bloated: prune debug incremental/examples before workspace cargo."
                )
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
