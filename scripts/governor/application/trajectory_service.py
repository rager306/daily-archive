"""Application service: trajectory — builds the full trajectory report."""

import hashlib
import json
import re
from datetime import datetime, timedelta

import yaml
from pathlib import Path

from governor.application.audit_service import AuditService
from governor.domain.trajectory import (
# M449: graph semantics + data integrity
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


def _promotion_qualification_violations(package: dict, contract: dict) -> set[str]:
    """Return disqualifier codes for one measured family promotion package.

    Mirrors `da_application::native_family_promotion_gate`
    `derive_promotion_qualification`: the three disqualifying conditions this
    contract enforces are a shared *measured* producer digest, producer
    independence that is not established by measured identities, and
    contamination that was never assessed. Tri-state contamination is read
    verbatim — an absent field is unknown, never clean. The oracle consumes
    measured values (contamination, producer provenance) and the one
    cardinality requirement declared as DATA in
    `qualification_contract.policy`; it invents no measurement and evaluates no
    validation-quality threshold (those stay in Rust). Returns a set of
    violation codes (empty = qualified measurements).
    """
    violations: set[str] = set()
    policy = (contract.get("qualification_contract") or {}).get("policy") or {}
    try:
        required = int(policy.get("minimum_independent_producers", 0))
    except (TypeError, ValueError):
        required = 0

    contamination = package.get("contamination")
    if contamination == "detected":
        violations.add("contamination_detected")
    elif contamination != "not_detected":
        # Absent, null or any unrecognized value is an unknown measurement —
        # never a clean NotDetected.
        violations.add("contamination_not_assessed")

    producers = package.get("producer_provenance")
    if not isinstance(producers, list):
        producers = []
    complete_digests: list[str] = []
    for producer in producers:
        if not isinstance(producer, dict):
            continue
        digest = producer.get("code_digest")
        method = producer.get("method")
        authority = producer.get("authority")
        if not all(
            isinstance(value, str) and value.strip()
            for value in (digest, method, authority)
        ):
            # An incomplete measured identity does not count as independence.
            continue
        complete_digests.append(digest.strip())

    distinct = set(complete_digests)
    if len(complete_digests) != len(distinct):
        violations.add("shared_producer_digest")
    if len(distinct) < required:
        violations.add("weak_producer_independence")
    return violations


def _acap_admission_violations(record: dict, contract: dict, transitions: set) -> set[str]:
    """Return violation codes for one ACAP runtime admission record (PIR-014).

    Mirrors `ArticleContextState::admit_transition` + `can_transition_to` in
    `crates/da-domain/src/assurance.rs`: an admitted stage must carry a
    durable evidence reference and a declared forward FSM transition; a
    blocked record must carry the typed `admission:<code>` error class; a
    cache hit keeps its durable cache-key sha256; D127 keeps
    `import_eligible` false. An illegal mid-FSM jump fails closed even when a
    durable cached output exists for the target stage.
    """
    violations: set[str] = set()
    req = set((contract.get("record_contract") or {}).get("required_fields", []))
    if not req.issubset(record):
        return {"missing_record_fields"}
    if not str(record.get("stage_id") or "").strip():
        violations.add("empty_stage_id")
    if record.get("import_eligible") is True:
        violations.add("import_leak")

    admitted = record.get("admitted")
    disposition = record.get("disposition")
    allowed = set((contract.get("record_contract") or {}).get("dispositions", []))
    if allowed and disposition not in allowed:
        violations.add("invalid_disposition")
    from_state = str(record.get("from_state") or "")
    to_state = str(record.get("to_state") or "")
    if transitions and (from_state, to_state) not in transitions:
        violations.add("illegal_transition")

    if admitted is True:
        evidence_ref = record.get("evidence_ref")
        if not isinstance(evidence_ref, str) or not evidence_ref.strip():
            violations.add("missing_evidence_ref")
        if record.get("error"):
            violations.add("admitted_with_error")
    elif admitted is False:
        if not record.get("error_class"):
            violations.add("denied_without_reason")
    if disposition == "cache_hit" and not record.get("cache_key_sha256"):
        violations.add("cache_hit_without_key")
    return violations


def _acap_promotion_authority_violations(record: dict, contract: dict) -> set[str]:
    """Return violation codes for one promotion authority record (PIR-015).

    Mirrors the runtime promotion surface of the T01 control-plane proof:
    promotion requires exactly three explicit actor-gated transitions
    (Verify → Review → Promote) — optimizer auto-promotion is never allowed;
    the promoter must be independent of verifier/reviewer (no authority
    collapse); durable evidence refs are mandatory; D127 keeps
    `import_eligible` false. The full PromotionDecision wire contract lives
    in `promotion_evidence_contract.yaml`; this check guards the runtime
    authority shape of the same protocol.
    """
    violations: set[str] = set()
    req = set((contract.get("record_contract") or {}).get("required_fields", []))
    if not req.issubset(record):
        return {"missing_record_fields"}
    if not str(record.get("decision_id") or "").strip():
        violations.add("empty_decision_id")
    if record.get("import_eligible") is True:
        violations.add("import_leak")

    transition = str(record.get("transition") or "")
    required_transitions = int(
        (contract.get("record_contract") or {}).get("required_explicit_transitions", 3)
    )
    explicit = record.get("explicit_transitions")
    if transition == "auto_promoted" or explicit != required_transitions:
        violations.add("auto_promote")

    verifier = record.get("verifier")
    reviewer = record.get("reviewer")
    promoter = record.get("promoter")
    if promoter and (promoter == verifier or promoter == reviewer):
        violations.add("self_authority")
    evidence = record.get("evidence_refs")
    if not isinstance(evidence, list) or not evidence:
        violations.add("missing_evidence")
    return violations


def _acap_scheduler_violations(record: dict, contract: dict) -> set[str]:
    """Return violation codes for one durable scheduler recovery record (PIR-016).

    Mirrors the T01 scheduler plane: a crashed worker's expired lease is
    reclaimed by a successor (the crashed attempt stays Aborted, the fence
    epoch increments), the task completes with exactly one logical result
    published at the current epoch, and after replay the completed task is
    never due again and no new attempts exist. D127 keeps `import_eligible`
    false.
    """
    violations: set[str] = set()
    req = set((contract.get("record_contract") or {}).get("required_fields", []))
    if not req.issubset(record):
        return {"missing_record_fields"}
    if not str(record.get("task_vid") or "").strip():
        violations.add("empty_task_vid")
    if record.get("import_eligible") is True:
        violations.add("import_leak")

    status = str(record.get("status") or "")
    completed = str((contract.get("record_contract") or {}).get("completed_status", "completed"))
    results = record.get("logical_results") or []
    if len(results) > 1:
        violations.add("double_publish")
    outcomes = record.get("attempt_outcomes") or []
    attempts = int(record.get("attempts") or 0)
    if attempts > 1 and "aborted" not in outcomes:
        violations.add("recovery_without_aborted_attempt")
    if status == completed and not results:
        violations.add("completed_without_result")
    if int(record.get("due_after_replay") or 0) > 0:
        violations.add("due_after_completion")
    if results:
        first = results[0] if isinstance(results[0], dict) else {}
        if int(first.get("published_by_epoch") or 0) < len(outcomes):
            violations.add("stale_epoch_publish")
        evidence = first.get("evidence_refs")
        if not isinstance(evidence, list) or not evidence:
            violations.add("missing_evidence")
    return violations


def _acap_replay_violations(record: dict, contract: dict) -> set[str]:
    """Return violation codes for one replay-idempotence record.

    Mirrors the T01 replay section: an identical replay creates no duplicate
    authoritative effects — every stage executor runs exactly once, stage
    records stay 1:1 with executors, every replay disposition is a cache hit,
    the promotion sink does not grow, no new scheduler attempts are minted,
    and the completed task is never due again. D127 stays locked.
    """
    violations: set[str] = set()
    req = set((contract.get("record_contract") or {}).get("required_fields", []))
    if not req.issubset(record):
        return {"missing_record_fields"}
    if record.get("import_eligible") is True:
        violations.add("import_leak")

    calls = record.get("pipeline_executor_calls") or []
    if any(
        isinstance(pair, (list, tuple)) and len(pair) == 2 and int(pair[1]) > 1
        for pair in calls
    ):
        violations.add("executor_rerun")
    stage_records = int(record.get("pipeline_stage_records") or 0)
    if stage_records > len(calls):
        violations.add("duplicate_stage_records")
    dispositions = record.get("pipeline_replay_dispositions") or []
    if any(d != "cache_hit" for d in dispositions):
        violations.add("cache_miss_on_replay")
    if int(record.get("promotion_sink_len_after_replay") or 0) > int(
        record.get("promotion_sink_len") or 0
    ):
        violations.add("promotion_sink_growth")
    if int(record.get("scheduler_attempts_after_replay") or 0) > int(
        record.get("scheduler_attempts") or 0
    ):
        violations.add("scheduler_new_attempts")
    if int(record.get("scheduler_due_after_replay") or 0) > 0:
        violations.add("scheduler_due_again")
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


def _is_sha256_hex(value) -> bool:
    """True when value is a 64-character SHA-256 hex digest (case-insensitive).

    Mirrors `normalize_sha256` in the da-domain candidate modules: a lineage
    field must be exactly 64 ASCII hex digits to be a content-hash reference.
    """
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdefABCDEF" for ch in value)
    )


def _sha256_vid(prefix: str, key: str) -> str:
    """Reproduce `kg_ontology::vid::sha256_vid` (prefix + ":" + trimmed key).

    The Rust helper canonicalizes the key with `trim()` then hashes
    `prefix + ":" + canonical`; the Python mirror must match byte-for-byte so
    the committed fixture VIDs can be recomputed by the Governor.
    """
    return hashlib.sha256(f"{prefix}:{key.strip()}".encode("utf-8")).hexdigest()


def _visual_candidate_violations(record: dict, contract: dict) -> set[str]:
    """Return violation codes for one FigureCandidate fixture record (PIR-018).

    Mirrors `da_domain::figure_candidate::FigureCandidate::validate` plus
    `figure_candidate_vid`: the revision and section VIDs and every SHA-256
    lineage field must be 64-character hex digests, the stored VID must equal
    the recomputed revision-bound lineage VID (revision + artifact label +
    detector fingerprint + rendered image hash + section VID), and authority
    and import locks must stay false (D127 / ADR-053 — candidate evidence is
    never graph truth and never gains import eligibility). Returns a set of
    violation codes (empty = honest).
    """
    violations: set[str] = set()
    candidate_contract = contract.get("candidate_contract") or {}
    req = set(candidate_contract.get("required_fields", []))
    if not req.issubset(record):
        return {"missing_candidate_fields"}

    if not _is_sha256_hex(record.get("revision_vid")):
        violations.add("missing_revision_lineage")
    if not _is_sha256_hex(record.get("section_vid")):
        violations.add("broken_section_lineage")
    for field in candidate_contract.get("sha256_fields", []):
        if not _is_sha256_hex(record.get(field)):
            violations.add("missing_sha256_lineage")
    try:
        recomputed = _sha256_vid(
            "figure_candidate",
            "\n".join([
                str(record.get("revision_vid", "")),
                str(record.get("artifact_id", "")),
                str(record.get("detector_fingerprint", "")),
                str(record.get("image_sha256", "")),
                str(record.get("section_vid", "")),
            ]),
        )
        if recomputed != record.get("vid"):
            violations.add("identity_mismatch")
    except Exception:
        violations.add("identity_mismatch")
    if record.get("authoritative") is True:
        violations.add("authority_unlock")
    if record.get("import_eligible") is True:
        violations.add("import_unlock")
    return violations


def _table_candidate_violations(record: dict, contract: dict) -> set[str]:
    """Return violation codes for one TableCandidate fixture record (PIR-019).

    Mirrors `da_domain::table_candidate` + `da_domain::table_fidelity` (M400
    S02, ADR-051): TEI grounding is mandatory (R087 — no structural TEI
    table, no candidate), every materialized cell must carry grid provenance
    (row, column), a deterministic digest, and a both-or-neither source span
    (typed missingness, never guessed), disagreements between VLM and TEI
    must be typed conflict nodes with allowed kinds (coverage gaps stay
    counters — never conflict nodes), and authority/import stay hard-locked
    false (D127 / ADR-053). Returns a set of violation codes (empty = honest).
    """
    violations: set[str] = set()
    candidate_contract = contract.get("candidate_contract") or {}
    req = set(candidate_contract.get("required_fields", []))
    if not req.issubset(record):
        return {"missing_candidate_fields"}

    if not str(record.get("tei_table_id") or "").strip():
        violations.add("no_tei_grounding")

    cell_contract = contract.get("cell_contract") or {}
    cell_req = set(cell_contract.get("required_fields", []))
    for cell in record.get("cells") or []:
        if not cell_req.issubset(cell):
            violations.add("missing_cell_provenance")
        if cell.get("row") is None or cell.get("column") is None:
            violations.add("missing_cell_provenance")
        if not _is_sha256_hex(cell.get("digest")):
            violations.add("cell_without_digest")
        span_start = cell.get("span_start")
        span_end = cell.get("span_end")
        if (span_start is None) != (span_end is None):
            violations.add("cell_partial_span")
        if cell.get("candidate_vid") != record.get("vid"):
            violations.add("identity_mismatch")
        if cell.get("row") is not None and cell.get("column") is not None:
            try:
                cell_vid = _sha256_vid(
                    "table_cell",
                    f"{cell.get('candidate_vid')}\n{cell.get('row')}\n{cell.get('column')}",
                )
                if cell_vid != cell.get("vid"):
                    violations.add("identity_mismatch")
            except Exception:
                violations.add("identity_mismatch")

    conflict_contract = contract.get("conflict_contract") or {}
    allowed_kinds = set(conflict_contract.get("allowed_kinds", []))
    gap_kinds = set(conflict_contract.get("gap_kinds_forbidden", []))
    for conflict in record.get("conflicts") or []:
        kind = str(conflict.get("kind") or "")
        if kind in gap_kinds:
            violations.add("coverage_gap_as_conflict")
        elif kind and kind not in allowed_kinds:
            violations.add("invalid_conflict_kind")
        if conflict.get("authoritative") is True:
            violations.add("authority_unlock")
        if conflict.get("import_eligible") is True:
            violations.add("import_unlock")

    try:
        recomputed = _sha256_vid(
            "table_candidate",
            "\n".join([
                str(record.get("revision_vid", "")),
                str(record.get("artifact_id", "")),
                str(record.get("image_sha256", "")),
                str(record.get("section_vid", "")),
                str(record.get("tei_table_id", "")),
                str(record.get("tei_source_digest", "")),
            ]),
        )
        if recomputed != record.get("vid"):
            violations.add("identity_mismatch")
    except Exception:
        violations.add("identity_mismatch")
    if record.get("authoritative") is True:
        violations.add("authority_unlock")
    if record.get("import_eligible") is True:
        violations.add("import_unlock")
    return violations


def _equation_candidate_violations(record: dict, contract: dict) -> set[str]:
    """Return violation codes for one EquationCandidate fixture record (PIR-020).

    Mirrors `da_domain::equation_candidate` (M400 S03, ADR-051): every
    equation candidate must retain formula identity (`formula_id`), exact
    revision-scoped section lineage (`revision_vid`, `section_vid`,
    `section_path_id`), a typed representation kind (MathML, TeX, plain text)
    with representation text and digest, and a both-or-neither source span
    (typed missingness — never guessed). Authority and import stay hard-locked
    false (D127 / ADR-053); extraction missingness is typed at conversion and
    never invents a candidate. Returns a set of violation codes (empty =
    honest).
    """
    violations: set[str] = set()
    candidate_contract = contract.get("candidate_contract") or {}
    req = set(candidate_contract.get("required_fields", []))
    if not req.issubset(record):
        return {"missing_candidate_fields"}

    if not str(record.get("formula_id") or "").strip():
        violations.add("missing_formula_identity")
    if not _is_sha256_hex(record.get("revision_vid")):
        violations.add("ungrounded_formula")
    kinds = set(candidate_contract.get("representation_kinds", []))
    if (
        not str(record.get("representation_text") or "").strip()
        or record.get("representation_kind") not in kinds
    ):
        violations.add("missing_representation")
    span_start = record.get("span_start")
    span_end = record.get("span_end")
    if (span_start is None) != (span_end is None):
        violations.add("partial_span")
    if span_start is not None and span_end is not None and span_start > span_end:
        violations.add("invalid_span")
    try:
        recomputed = _sha256_vid(
            "equation_candidate",
            "\n".join([
                str(record.get("revision_vid", "")),
                str(record.get("formula_id", "")),
                str(record.get("section_vid", "")),
                str(record.get("source_digest", "")),
            ]),
        )
        if recomputed != record.get("vid"):
            violations.add("identity_mismatch")
    except Exception:
        violations.add("identity_mismatch")
    if record.get("authoritative") is True:
        violations.add("authority_unlock")
    if record.get("import_eligible") is True:
        violations.add("import_unlock")
    return violations


# GAR-001: canonical boundary coordinate metrics. The oracle recomputes
# positional accuracy independently of the Rust runner (`match_boundaries` in
# `crates/da-cli/examples/document_segmentation_shadow.rs`) so a metric that
# only measures count equality cannot silently pass the control plane.
_BOUNDARY_POSITIONAL_METRICS = (
    "boundary_precision",
    "boundary_recall",
    "boundary_f1",
    "boundary_displacement",
    "boundary_displacement_max",
)
_BOUNDARY_METRIC_TOLERANCE = 1e-6


def _boundary_coordinate(value) -> tuple[int, int, int, int] | None:
    """Parse one canonical `[page, block, line, token]` coordinate.

    Returns None when the value is not exactly four integers (one_based);
    booleans are rejected because `bool` is an `int` subclass in Python.
    """
    if not isinstance(value, (list, tuple)) or len(value) != 4:
        return None
    parts: list[int] = []
    for item in value:
        if isinstance(item, bool) or not isinstance(item, int):
            return None
        parts.append(int(item))
    return (parts[0], parts[1], parts[2], parts[3])


def _boundary_pair(value) -> tuple | None:
    """Parse one `{start: [..], end: [..]}` boundary pair."""
    if not isinstance(value, dict):
        return None
    start = _boundary_coordinate(value.get("start"))
    end = _boundary_coordinate(value.get("end"))
    if start is None or end is None:
        return None
    return (start, end)


def _boundary_pairs(value) -> list | None:
    """Parse a boundary list; None means the list is absent or malformed."""
    if not isinstance(value, list):
        return None
    pairs = []
    for item in value:
        pair = _boundary_pair(item)
        if pair is None:
            return None
        pairs.append(pair)
    return pairs


def _boundary_pair_distance(left, right) -> int:
    """L1 distance over both endpoints of two canonical boundaries."""
    start_distance = sum(abs(a - b) for a, b in zip(left[0], right[0]))
    end_distance = sum(abs(a - b) for a, b in zip(left[1], right[1]))
    return start_distance + end_distance


def _boundary_positional_violations(record: dict, contract: dict) -> set[str]:
    """Return violation codes for one boundary-metric fixture record (GAR-001).

    Independent Python oracle: it recomputes positional boundary accuracy
    from the raw canonical `[page, block, line, token]` coordinates and
    compares the declared metrics against the recomputation (ADR-051 — the
    oracle must not reuse the Rust runner's code). Matching is greedy in
    predicted order: for each predicted boundary the unmatched reviewed
    boundary with the smallest `distance <= tolerance` wins, ties going to
    the earliest reviewed boundary, where the distance is the L1 distance of
    the starts plus the L1 distance of the ends.

    Violation labels (a label with no reprimand is a claim over count
    metrics that the coordinates contradict):
      - missing_coordinate_binding: a positional metric is declared but the
        predicted/reviewed boundary lists are absent or not four-integer
        coordinates (this is the only label returned in that case);
      - count_metric_mismatch: declared count agreement/difference does not
        equal the recomputed boundary counts;
      - permuted_boundary_false_match: equal boundary counts, declared
        precision (or recall) 1.0, but no positionally matching pair;
      - count_metric_claims_position: a declared positional metric differs
        from the recomputed value by more than 1e-6;
      - displacement_from_counts: zero declared displacement while nothing
        matched and both sides are non-empty;
      - unmatched_not_false_positive: declared false positives/negatives do
        not equal unmatched predicted/reviewed counts;
      - tolerance_violation: declared matched exceeds the number of pairs
        within the pinned tolerance.
    `null`/absent declared values mean "not claimed" and emit no label.
    """
    declared = record.get("reported") or {}
    if not isinstance(declared, dict):
        declared = {}
    positional_metrics = list(
        (contract.get("metrics") or {}).get("positional")
        or _BOUNDARY_POSITIONAL_METRICS
    )
    positional_claimed = [m for m in positional_metrics if declared.get(m) is not None]

    predicted = _boundary_pairs(record.get("predicted_boundaries"))
    reviewed = _boundary_pairs(record.get("reviewed_boundaries"))
    if predicted is None or reviewed is None:
        # A positional claim without canonical coordinates is the binding
        # failure itself; without a positional claim the record is silent.
        return {"missing_coordinate_binding"} if positional_claimed else set()

    tolerance_value = (contract.get("coordinate_binding") or {}).get("tolerance", 0)
    tolerance = (
        tolerance_value
        if isinstance(tolerance_value, (int, float)) and not isinstance(tolerance_value, bool)
        else 0
    )

    used = [False] * len(reviewed)
    matched = 0
    displacements: list[int] = []
    for predicted_pair in predicted:
        best_index = None
        best_distance = None
        for index, reviewed_pair in enumerate(reviewed):
            if used[index]:
                continue
            distance = _boundary_pair_distance(predicted_pair, reviewed_pair)
            if distance <= tolerance and (best_distance is None or distance < best_distance):
                best_distance = distance
                best_index = index
        if best_index is not None:
            used[best_index] = True
            matched += 1
            displacements.append(int(best_distance))

    within_tolerance_pairs = sum(
        1
        for predicted_pair in predicted
        for reviewed_pair in reviewed
        if _boundary_pair_distance(predicted_pair, reviewed_pair) <= tolerance
    )

    recomputed_precision = matched / len(predicted) if predicted else None
    recomputed_recall = matched / len(reviewed) if reviewed else None
    if recomputed_precision is None or recomputed_recall is None:
        recomputed_f1 = None
    elif recomputed_precision + recomputed_recall > 0:
        recomputed_f1 = (
            2 * recomputed_precision * recomputed_recall
            / (recomputed_precision + recomputed_recall)
        )
    else:
        recomputed_f1 = 0.0
    if displacements:
        recomputed_displacement = sum(displacements) / len(displacements)
        recomputed_displacement_max = float(max(displacements))
    else:
        recomputed_displacement = None
        recomputed_displacement_max = None

    violations: set[str] = set()

    declared_count_agreement = declared.get("count_agreement")
    if declared_count_agreement is not None and declared_count_agreement != int(
        len(predicted) == len(reviewed)
    ):
        violations.add("count_metric_mismatch")
    declared_count_difference = declared.get("count_difference")
    if declared_count_difference is not None and declared_count_difference != abs(
        len(predicted) - len(reviewed)
    ):
        violations.add("count_metric_mismatch")

    declared_precision = declared.get("boundary_precision")
    declared_recall = declared.get("boundary_recall")
    if (
        len(predicted) == len(reviewed)
        and matched == 0
        and (declared_precision == 1.0 or declared_recall == 1.0)
    ):
        violations.add("permuted_boundary_false_match")

    for name, recomputed in (
        ("boundary_precision", recomputed_precision),
        ("boundary_recall", recomputed_recall),
        ("boundary_f1", recomputed_f1),
        ("boundary_displacement", recomputed_displacement),
        ("boundary_displacement_max", recomputed_displacement_max),
    ):
        claimed = declared.get(name)
        if claimed is None or recomputed is None:
            continue
        try:
            numeric = float(claimed)
        except (TypeError, ValueError):
            violations.add("count_metric_claims_position")
            continue
        if abs(numeric - recomputed) > _BOUNDARY_METRIC_TOLERANCE:
            violations.add("count_metric_claims_position")

    if matched == 0 and predicted and reviewed:
        for name in ("boundary_displacement", "boundary_displacement_max"):
            if declared.get(name) == 0.0:
                violations.add("displacement_from_counts")

    declared_false_positives = declared.get("false_positives")
    if (
        declared_false_positives is not None
        and declared_false_positives != len(predicted) - matched
    ):
        violations.add("unmatched_not_false_positive")
    declared_false_negatives = declared.get("false_negatives")
    if (
        declared_false_negatives is not None
        and declared_false_negatives != len(reviewed) - matched
    ):
        violations.add("unmatched_not_false_positive")

    declared_matched = declared.get("matched")
    if declared_matched is not None and declared_matched > within_tolerance_pairs:
        violations.add("tolerance_violation")

    return violations


def _non_empty_string(value) -> bool:
    """True when value is a string with non-whitespace content."""
    return isinstance(value, str) and bool(value.strip())


def _visual_adapter_violations(record: dict, contract: dict) -> set[str]:
    """Return violation codes for one donor-visual-adapter fixture record (GAR-003).

    Independent Python oracle over the durable artifact record shape produced
    by `crates/da-cli/examples/visual_positive_fixture_run.rs`:

        {fixture_id, family, fixture_source_sha256,
         donor: {primitive_count, error},
         native: {primitive_count, lane_absence, head_outcome, head_reason,
                  primitives[], declared_non_deliverable_families}}

    It never calls or copies the Rust converter (ADR-051 — the oracle must not
    share code with the subject under test); it only checks that the declared
    donor content is still present, provenance-complete and honestly labelled
    in the native result.

    Labels:
      - donor_primitives_discarded: the donor produced primitives without an
        error but the native result holds none (or holds fewer than it
        declares) — the pre-fix `primitives: Vec::new()` drop path;
      - malformed_mapped_to_not_observed: a Malformed/InvalidLayout donor
        error is reported as `not_observed` absence instead of a typed
        malformed absence;
      - empty_observation_without_lane_absence: both sides are empty with no
        lane-absence reason, i.e. emptiness is not explained;
      - primitive_provenance_lost: a surviving primitive lost provider id or
        version, document id, source digest, content digest, bbox, a positive
        page ordinal or a valid source ordinal, or the ordinal set no longer
        matches the donor primitive count;
      - terminal_only_parity_without_positive_evidence: a family that must
        reach an observed head keeps donor primitives but reports a terminal
        head;
      - fixture_family_missing: the record's family is outside the contract's
        declared fixture families;
      - non_deliverable_families_undeclared: the declared non-deliverable
        receipt families do not exactly match the contract vocabulary (a
        silently empty declaration is flagged).
    A record that is not a `{donor, native}` dict returns the single label
    `missing_record_shape`.
    """
    if not isinstance(record, dict):
        return {"missing_record_shape"}
    donor = record.get("donor")
    native = record.get("native")
    if not isinstance(donor, dict) or not isinstance(native, dict):
        return {"missing_record_shape"}

    violations: set[str] = set()

    family = record.get("family")
    if family not in list(contract.get("fixture_families") or []):
        violations.add("fixture_family_missing")

    def _count(node: dict) -> int:
        value = node.get("primitive_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else 0

    donor_count = _count(donor)
    native_count = _count(native)
    donor_error = donor.get("error")
    lane_absence = native.get("lane_absence")
    primitives = native.get("primitives")
    primitives = primitives if isinstance(primitives, list) else None

    if donor_error is None and donor_count > 0:
        if native_count == 0:
            violations.add("donor_primitives_discarded")
        elif native_count == donor_count and (
            primitives is None or len(primitives) != donor_count
        ):
            violations.add("donor_primitives_discarded")

    if donor_error in ("Malformed", "InvalidLayout") and lane_absence == "not_observed":
        violations.add("malformed_mapped_to_not_observed")

    if (
        donor_count == 0
        and donor_error is None
        and native_count == 0
        and lane_absence is None
    ):
        violations.add("empty_observation_without_lane_absence")

    if primitives is None:
        if native_count > 0:
            violations.add("primitive_provenance_lost")
    else:
        ordinals: list = []
        for primitive in primitives:
            if not isinstance(primitive, dict):
                violations.add("primitive_provenance_lost")
                continue
            provider = primitive.get("provider")
            if (
                not isinstance(provider, dict)
                or not _non_empty_string(provider.get("id"))
                or not _non_empty_string(provider.get("version"))
            ):
                violations.add("primitive_provenance_lost")
            if any(
                not _non_empty_string(primitive.get(field))
                for field in ("document_id", "source_sha256", "content_digest")
            ):
                violations.add("primitive_provenance_lost")
            bbox = primitive.get("bbox")
            if (
                not isinstance(bbox, list)
                or len(bbox) != 4
                or not all(
                    isinstance(value, (int, float)) and not isinstance(value, bool)
                    for value in bbox
                )
            ):
                violations.add("primitive_provenance_lost")
            page_ordinal = primitive.get("page_ordinal")
            if (
                not isinstance(page_ordinal, int)
                or isinstance(page_ordinal, bool)
                or page_ordinal < 1
            ):
                violations.add("primitive_provenance_lost")
            source_ordinal = primitive.get("source_ordinal")
            if (
                not isinstance(source_ordinal, int)
                or isinstance(source_ordinal, bool)
                or source_ordinal < 0
            ):
                violations.add("primitive_provenance_lost")
            else:
                ordinals.append(source_ordinal)
        if (
            donor_count > 0
            and len(primitives) == donor_count
            and len(ordinals) == len(primitives)
            and sorted(ordinals) != list(range(donor_count))
        ):
            violations.add("primitive_provenance_lost")

    if (
        family in list(contract.get("families_requiring_observed_head") or [])
        and donor_count > 0
        and donor_error is None
        and native_count > 0
        and native.get("head_outcome") == "terminal"
    ):
        violations.add("terminal_only_parity_without_positive_evidence")

    declared = native.get("declared_non_deliverable_families")
    expected_families = sorted(
        str(item) for item in (contract.get("non_deliverable_families") or [])
    )
    observed_families = (
        sorted(str(item) for item in declared) if isinstance(declared, list) else None
    )
    if observed_families is None or observed_families != expected_families:
        violations.add("non_deliverable_families_undeclared")

    return violations


_LICENSE_EVIDENCE_URL_PREFIXES = ("http://", "https://")


def _license_closed_vocabulary(contract: dict, key: str) -> list | None:
    """Return a non-empty closed vocabulary, or None when it is missing/empty.

    A field validated against a closed vocabulary must fail closed when the
    vocabulary itself is absent: an empty or missing vocabulary means "cannot
    decide", which the oracle reports as `missing_closed_vocabulary` instead of
    silently accepting any token.
    """
    value = contract.get(key)
    if isinstance(value, list) and value:
        return value
    return None


def _license_path_resolvable(value, root) -> bool:
    """True when a licence-evidence reference resolves to something real.

    Accepted forms: an `http(s)://` upstream pointer, an existing absolute path
    (vendor licence texts live outside the repository), or an existing
    repository-relative path. `root` is injected by the Governor rule; a direct
    oracle call without a root cannot resolve relative paths and reports the
    reference as unresolved rather than guessing.
    """
    if not _non_empty_string(value):
        return False
    if value.startswith(_LICENSE_EVIDENCE_URL_PREFIXES):
        return True
    if root is None:
        return False
    candidate = Path(value)
    resolved = candidate if candidate.is_absolute() else Path(root) / candidate
    return resolved.exists()


def _license_obligation_resolved(value, obligations: list | None, root) -> bool:
    """True when an artifact obligation is a vocabulary token or a real path.

    Free text is deliberately not accepted: an obligation that is neither a
    closed token nor an existing path is indistinguishable from an unkept
    promise, which is exactly the fail-open shape GAR-004 closes.
    """
    if not _non_empty_string(value):
        return False
    if obligations is not None and value in obligations:
        return True
    if root is None:
        return False
    candidate = Path(value)
    resolved = candidate if candidate.is_absolute() else Path(root) / candidate
    return resolved.exists()


def _license_provenance_violations(record: dict, contract: dict) -> set[str]:
    """Return violation codes for one canonical licence-inventory position (GAR-004).

    Independent Python oracle over `data/parse/license_inventory.yaml`
    `components[]`: it recomputes the rights state from the raw position and
    the contract's mirrored closed vocabularies (ADR-051 — it shares no code
    with the Rust contract in
    `crates/da-parse/tests/component_license_provenance_data_contract.rs`). A
    position is either confirmed or an explicit refusal; an unproven right is
    never assumed compatible.

    Labels:
      - missing_component_identity: no non-empty `component_id`;
      - unverified_status_token: `license_status` outside the closed
        vocabulary (`UNVERIFIED`, `unknown`, empty);
      - missing_blocked_reason: a blocked position without a non-empty reason;
      - blocked_reason_on_verified_position: a verified position that still
        carries a refusal reason;
      - missing_license_evidence / license_evidence_path_missing: absent
        evidence, or a path that does not resolve;
      - boundary_outside_vocabulary / redistribution_outside_vocabulary: token
        outside the closed vocabulary (a missing field is outside it too);
      - blocked_position_claims_redistribution: blocked while claiming
        `allowed`/`allowed_with_notice` redistribution;
      - gpl_linking_permitted: a position that relaxes the GPL lock;
      - redistribution_lock_relaxed: a position that relaxes
        `new_distribution`/`import_eligible`/`graph_write`/`auto_promote`;
      - unresolved_artifact_obligation: an attribution/source-offer obligation
        that is neither a closed token nor an existing path;
      - surface_claim_target_missing: a `surface_claims` entry naming an
        unknown surface or an identifier that surface does not declare;
      - missing_closed_vocabulary: a validated field has no vocabulary in the
        contract (fail-closed, contract-level guard).
    """
    violations: set[str] = set()
    if not isinstance(record, dict):
        return {"missing_component_identity"}

    root = contract.get("_root")
    surface_ids = contract.get("_surface_ids")
    surface_ids = surface_ids if isinstance(surface_ids, dict) else {}
    boundary_vocabulary = _license_closed_vocabulary(contract, "boundary_vocabulary")
    status_vocabulary = _license_closed_vocabulary(
        contract, "license_status_vocabulary"
    )
    redistribution_vocabulary = _license_closed_vocabulary(
        contract, "redistribution_status_vocabulary"
    )
    obligations = _license_closed_vocabulary(contract, "artifact_obligation_vocabulary")
    blocked_statuses = _license_closed_vocabulary(
        contract, "blocked_status_vocabulary"
    ) or ["blocked"]

    if not _non_empty_string(record.get("component_id")):
        violations.add("missing_component_identity")

    status = record.get("license_status")
    if status_vocabulary is None:
        violations.add("missing_closed_vocabulary")
    elif not isinstance(status, str) or status not in status_vocabulary:
        violations.add("unverified_status_token")
    blocked = isinstance(status, str) and status in blocked_statuses

    blocked_reason = record.get("blocked_reason")
    if blocked and not _non_empty_string(blocked_reason):
        violations.add("missing_blocked_reason")
    elif not blocked and _non_empty_string(blocked_reason):
        violations.add("blocked_reason_on_verified_position")

    evidence = record.get("license_evidence")
    if not _non_empty_string(evidence):
        violations.add("missing_license_evidence")
    elif not _license_path_resolvable(evidence, root):
        violations.add("license_evidence_path_missing")

    if boundary_vocabulary is None:
        violations.add("missing_closed_vocabulary")
    elif record.get("boundary") not in boundary_vocabulary:
        violations.add("boundary_outside_vocabulary")

    redistribution = record.get("redistribution")
    if redistribution_vocabulary is None:
        violations.add("missing_closed_vocabulary")
    elif redistribution not in redistribution_vocabulary:
        violations.add("redistribution_outside_vocabulary")
    if blocked and redistribution in ("allowed", "allowed_with_notice"):
        violations.add("blocked_position_claims_redistribution")

    if "gpl_linking" in record and str(record.get("gpl_linking")).lower() != "forbidden":
        violations.add("gpl_linking_permitted")
    if record.get("import_eligible") is True:
        violations.add("redistribution_lock_relaxed")
    for lock_field in ("new_distribution", "graph_write", "auto_promote"):
        if lock_field in record and str(record.get(lock_field)).lower() != "forbidden":
            violations.add("redistribution_lock_relaxed")

    for obligation_field in ("attribution_notice_artifact", "source_offer_artifact"):
        if not _license_obligation_resolved(
            record.get(obligation_field), obligations, root
        ):
            violations.add("unresolved_artifact_obligation")

    claims = record.get("surface_claims")
    if claims is None:
        claims = []
    if not isinstance(claims, list):
        violations.add("surface_claim_target_missing")
    else:
        for claim in claims:
            if not isinstance(claim, dict):
                violations.add("surface_claim_target_missing")
                continue
            surface_key = claim.get("surface")
            claimed_id = claim.get("component_id")
            known_ids = surface_ids.get(surface_key)
            if known_ids is None or claimed_id not in known_ids:
                violations.add("surface_claim_target_missing")

    return violations


def _license_surface_entry_violations(entry: dict, surface: dict, contract: dict) -> set[str]:
    """Structural violations of one read-only licence-surface entry (GAR-004).

    The three surfaces do not share one status dialect, so the entry is read
    through the surface's declared FIELD MAPPING (`id_field`, `status_field`,
    `blocked_reason_field`, `evidence_field`, `redistribution_field`) and
    validated under its own closed vocabulary. The surface node's own
    `boundary_vocabulary`/`redistribution_status_vocabulary`, when it declares
    them, are injected by the rule as private `_*` keys; a surface that
    publishes no vocabulary for a field is not validated on that field (its
    `kind` is an asset kind, not a runtime boundary).

    Evidence is required non-empty but never resolved to a path: these surfaces
    use in-file pointers (`source_contract.source_documents`, `corpus_binding`,
    `engines`) as evidence references.
    """
    violations: set[str] = set()
    if not isinstance(entry, dict):
        return {"missing_component_identity"}

    status_vocabulary = surface.get("status_vocabulary")
    if not isinstance(status_vocabulary, list) or not status_vocabulary:
        violations.add("missing_closed_vocabulary")
        status_vocabulary = None
    blocked_statuses = _license_closed_vocabulary(
        contract, "blocked_status_vocabulary"
    ) or ["blocked"]

    if not _non_empty_string(entry.get(surface.get("id_field"))):
        violations.add("missing_component_identity")

    status = entry.get(surface.get("status_field"))
    if status_vocabulary is not None and status not in status_vocabulary:
        violations.add("unverified_status_token")
    blocked = isinstance(status, str) and status in blocked_statuses

    reason_field = surface.get("blocked_reason_field")
    if blocked and reason_field and not _non_empty_string(entry.get(reason_field)):
        violations.add("missing_blocked_reason")

    evidence_field = surface.get("evidence_field")
    if evidence_field and not _non_empty_string(entry.get(evidence_field)):
        violations.add("missing_license_evidence")

    boundary_vocabulary = surface.get("_boundary_vocabulary")
    boundary_field = surface.get("boundary_field")
    if (
        isinstance(boundary_vocabulary, list)
        and boundary_vocabulary
        and boundary_field
        and entry.get(boundary_field) not in boundary_vocabulary
    ):
        violations.add("boundary_outside_vocabulary")

    redistribution_vocabulary = surface.get("_redistribution_vocabulary")
    redistribution_field = surface.get("redistribution_field")
    if (
        isinstance(redistribution_vocabulary, list)
        and redistribution_vocabulary
        and redistribution_field
        and entry.get(redistribution_field) not in redistribution_vocabulary
    ):
        violations.add("redistribution_outside_vocabulary")

    if blocked and (
        entry.get("redistributable") is True
        or (
            redistribution_field
            and entry.get(redistribution_field) in ("allowed", "allowed_with_notice")
        )
    ):
        violations.add("blocked_position_claims_redistribution")

    return violations


_LICENSE_VIOLATION_FIELDS = {
    "missing_component_identity": "component_id",
    "unverified_status_token": "license_status",
    "missing_blocked_reason": "blocked_reason",
    "blocked_reason_on_verified_position": "blocked_reason",
    "missing_license_evidence": "license_evidence",
    "license_evidence_path_missing": "license_evidence",
    "boundary_outside_vocabulary": "boundary",
    "redistribution_outside_vocabulary": "redistribution",
    "blocked_position_claims_redistribution": "redistribution",
    "gpl_linking_permitted": "gpl_linking",
    "unresolved_artifact_obligation": "attribution_notice_artifact",
    "surface_claim_target_missing": "surface_claims",
}


def _license_violation_detail(record: dict, violation: str) -> str:
    """Name the offending field and value for one canonical-position violation.

    Diagnostics over a 22-position ledger are only usable if a flag names both
    the position and the field that failed, so the label is resolved back to
    the field it was raised from.
    """
    if violation == "redistribution_lock_relaxed":
        relaxed = []
        for field in ("new_distribution", "import_eligible", "graph_write", "auto_promote"):
            value = record.get(field)
            kept = value is False if field == "import_eligible" else str(value).lower() == "forbidden"
            if not kept:
                relaxed.append(f"{field}={value!r}")
        return ", ".join(relaxed)
    if violation == "unresolved_artifact_obligation":
        return (
            f"attribution_notice_artifact={record.get('attribution_notice_artifact')!r}, "
            f"source_offer_artifact={record.get('source_offer_artifact')!r}"
        )
    field = _LICENSE_VIOLATION_FIELDS.get(violation)
    if field is None:
        return ""
    return f"{field}={record.get(field)!r}"


def _license_policy_violations(policy) -> list:
    """Return `(label, detail)` pairs for the canonical redistribution policy (GAR-004).

    Only the five locks named by GAR-004 are checked here; the wider policy
    (`unknown_rights`, `blocked_components_must_not_enter_runtime`) is pinned by
    the independent Rust contract. A missing/absent value fails closed, because
    an absent lock is not a kept lock.
    """
    if not isinstance(policy, dict):
        return [("redistribution_lock_relaxed", "redistribution_policy block missing")]
    violations: list = []
    if str(policy.get("gpl_linking")).lower() != "forbidden":
        violations.append(
            ("gpl_linking_permitted", f"gpl_linking={policy.get('gpl_linking')!r}")
        )
    if policy.get("import_eligible") is not False:
        violations.append(
            (
                "redistribution_lock_relaxed",
                f"import_eligible={policy.get('import_eligible')!r}",
            )
        )
    for lock_field in ("new_distribution", "graph_write", "auto_promote"):
        if str(policy.get(lock_field)).lower() != "forbidden":
            violations.append(
                (
                    "redistribution_lock_relaxed",
                    f"{lock_field}={policy.get(lock_field)!r}",
                )
            )
    return violations


# --- GAR-011: oracle digest recomputation helpers ---------------------------
#
# The segmentation oracle publishes digest-like fields on three surfaces: the
# reviewed fixture, the canonical byte artifact and the declaration. GAR-011
# recorded that those digests were only ever checked for SHAPE (64 lower-hex
# chars), never for provenance, so substituting the canonical bytes while
# leaving the digest string unchanged passed silently. These helpers carry the
# Python half of the closure: closed-vocabulary membership, canonical-input
# presence, ref direction and pointer/traversal resolution. The SHA-256
# mathematics and the byte-level recomputation live in the independent Rust
# reader `crates/da-parse/tests/oracle_digest_recomputation.rs` (ADR-051 / D273
# / MEM885): Python never recomputes a digest and never reads an audited file's
# semantics here.


def _pointer_segments(pointer) -> list:
    """Split a pointer into non-empty segments (`''` and `'/'` yield `[]`)."""
    if not isinstance(pointer, str):
        return []
    return [segment for segment in pointer.split("/") if segment]


def _pointer_text(segments) -> str:
    """Render pointer segments for diagnostics (`('a', '*')` -> `/a/*`)."""
    return "/" + "/".join(segments) if segments else "/"


def _pointer_segment_matches(pattern: str, candidate: str) -> bool:
    """True when a pointer segment matches another; `*` is the only wildcard."""
    return pattern == "*" or candidate == "*" or pattern == candidate


def _pointer_patterns_match(pattern_segments, candidate_segments) -> bool:
    """True when two pointers address the same shape (equal length + segments).

    `*` on either side matches any single segment, which is the relation the
    contract uses to tie a wildcard `ref` to a wildcard declaration. A missing
    segment is never a match, so a shorter pointer cannot silently resolve to a
    broader (or narrower) field.
    """
    if len(pattern_segments) != len(candidate_segments):
        return False
    return all(
        _pointer_segment_matches(pattern, candidate)
        for pattern, candidate in zip(pattern_segments, candidate_segments)
    )


def _resolve_pointer_values(node, segments) -> list:
    """Resolve a pointer whose `*` matches any single list index or mapping key.

    Returns every resolved value because a pointer is a pattern, not a single
    path; an empty list means the pointer does not resolve in this tree.
    """
    if not segments:
        return [node]
    head, rest = segments[0], segments[1:]
    if head == "*":
        if isinstance(node, list):
            children = list(node)
        elif isinstance(node, dict):
            children = list(node.values())
        else:
            children = []
    elif isinstance(node, dict):
        children = [node[head]] if head in node else []
    elif isinstance(node, list):
        try:
            index = int(head)
        except (TypeError, ValueError):
            children = []
        else:
            children = [node[index]] if 0 <= index < len(node) else []
    else:
        children = []
    resolved: list = []
    for child in children:
        resolved.extend(_resolve_pointer_values(child, rest))
    return resolved


def _population_document_count(population, documents_key: str):
    """The integer document count of a declared population, or None.

    `bool` is rejected explicitly because it is an `int` subclass: a report that
    writes `documents: true` is an unresolvable count, not the number 1, so the
    census binding fails closed instead of reading a boolean as a measurement.
    """
    if not isinstance(population, dict):
        return None
    value = population.get(documents_key)
    if isinstance(value, bool) or not isinstance(value, int):
        return None
    return value


def _digest_pattern_matches(pattern, key: str) -> bool:
    """Mini-syntax of `digest_field_patterns`: anchored regex or exact literal.

    The contract keeps DATA literal: `suffix$`, `^prefix`, `^exact$` are the
    anchored forms, anything else is an exact key. A bare literal is never a
    substring match, so `sha256` cannot claim `source_sha256`.
    """
    if not isinstance(pattern, str) or not pattern:
        return False
    if pattern.startswith("^") or pattern.endswith("$"):
        try:
            return re.search(pattern, key) is not None
        except re.error:
            return False
    return pattern == key


def _discover_digest_fields(node, segments, patterns, discovered) -> None:
    """Collect pointer segments of digest-like key/string-value pairs.

    `*` stands for a list index while mapping keys stay literal, so a declared
    wildcard can match a discovered key but a declared literal can never hide
    behind a wildcard of the walk. The caller bounds the scope with the
    contract's `audit_pointers`: a formula string such as
    `canonicalization.digest` lives outside them and is therefore never
    mistaken for a digest field.
    """
    if isinstance(node, dict):
        for key, value in node.items():
            child = list(segments) + [str(key)]
            if isinstance(value, str) and any(
                _digest_pattern_matches(pattern, str(key)) for pattern in patterns
            ):
                discovered.add(tuple(child))
            _discover_digest_fields(value, child, patterns, discovered)
    elif isinstance(node, list):
        for item in node:
            _discover_digest_fields(item, list(segments) + ["*"], patterns, discovered)


def _digest_declared_records(contract: dict) -> list:
    """Declared digest-field classification records (possibly empty)."""
    declared = contract.get("digest_fields")
    return declared if isinstance(declared, list) else []


def _digest_ref_matches(ref, surface, declared: list) -> list:
    """Declared records a canonical `ref` names (surface + pointer shape).

    A ref is `'<surface>:<pointer>'`; the pseudo-surface `self` binds to the
    referring record's own surface, so an artifact record can point inside
    itself without repeating its key. The pointer comparison is
    shape-compatible (equal length, `*` matches any single segment).
    """
    if not isinstance(ref, str) or ":" not in ref:
        return []
    ref_surface, _, pointer = ref.partition(":")
    if ref_surface == "self":
        ref_surface = str(surface)
    ref_segments = _pointer_segments(pointer)
    return [
        record for record in declared
        if isinstance(record, dict)
        and str(record.get("surface")) == ref_surface
        and _pointer_patterns_match(_pointer_segments(record.get("pointer")), ref_segments)
    ]


def _digest_ref_target_is_opaque(record: dict, contract: dict) -> bool:
    """True when a recomputed record's canonical `ref` names an opaque marker.

    An opaque field publishes no canonical bytes, so it can never be the
    provenance source of a recomputed digest: pointing at one would launder an
    unverifiable marker into a recomputed claim.
    """
    return any(
        matched.get("class") == "opaque"
        for matched in _digest_ref_matches(
            record.get("ref"), record.get("surface"), _digest_declared_records(contract)
        )
    )


def _oracle_digest_violations(record: dict, contract: dict) -> set[str]:
    """Return violation codes for one declared oracle-digest field (GAR-011).

    Independent Python oracle over the segmentation oracle's digest-like
    fields. It classifies FORM only — closed-vocabulary membership, presence of
    the canonical input, the shape of the `ref`, and the provenance direction
    of the classification. It reads no file and computes no SHA-256: the
    digest mathematics lives in the independent Rust reader
    `crates/da-parse/tests/oracle_digest_recomputation.rs` (ADR-051 / D273 /
    MEM885), so the reader and the oracle share no code.

    Labels:
      - missing_field_identity: no non-empty `field_id`/`surface`/`pointer`;
      - digest_without_class: no non-empty class token;
      - class_outside_vocabulary: class outside the closed `classes` list;
      - recomputed_without_canonical_ref: a recomputed field with no `ref`;
      - recomputed_without_canonicalization: a recomputed field with no
        `canonicalization`;
      - canonicalization_outside_contract: a canonicalization outside the
        closed contract list;
      - opaque_without_reason: an opaque marker with no `opaque_reason`;
      - opaque_reason_outside_vocabulary: an opaque reason outside the closed
        vocabulary;
      - opaque_marker_as_provenance: an opaque marker offered as a provenance
        source — either the record itself is opaque yet carries a `ref`, or it
        is recomputed and its `ref` resolves to an opaque declaration;
      - alignment_outside_vocabulary: an alignment outside the closed list;
      - missing_closed_vocabulary: a validated field has no non-empty
        vocabulary in the contract (fail-closed, contract-level guard).
    """
    violations: set[str] = set()
    if not isinstance(record, dict):
        return {"missing_field_identity"}

    classes = contract.get("classes")
    classes = classes if isinstance(classes, list) and classes else None
    canonicalizations = contract.get("canonicalizations")
    canonicalizations = (
        canonicalizations
        if isinstance(canonicalizations, list) and canonicalizations
        else None
    )
    opaque_reasons = contract.get("opaque_reasons")
    opaque_reasons = (
        opaque_reasons if isinstance(opaque_reasons, list) and opaque_reasons else None
    )
    alignments = contract.get("alignments")
    alignments = alignments if isinstance(alignments, list) and alignments else None

    identity_fields = ("field_id", "surface", "pointer")
    if not all(
        _non_empty_string(record.get(field)) for field in identity_fields
    ):
        violations.add("missing_field_identity")

    class_value = record.get("class")
    if not _non_empty_string(class_value):
        violations.add("digest_without_class")
        class_value = None
    elif classes is None:
        violations.add("missing_closed_vocabulary")
    elif class_value not in classes:
        violations.add("class_outside_vocabulary")

    ref = record.get("ref")
    if class_value == "recomputed":
        if not _non_empty_string(ref):
            violations.add("recomputed_without_canonical_ref")
        canonicalization = record.get("canonicalization")
        if not _non_empty_string(canonicalization):
            violations.add("recomputed_without_canonicalization")
        elif canonicalizations is None:
            violations.add("missing_closed_vocabulary")
        elif canonicalization not in canonicalizations:
            violations.add("canonicalization_outside_contract")
        if _non_empty_string(ref) and _digest_ref_target_is_opaque(record, contract):
            violations.add("opaque_marker_as_provenance")
    elif class_value == "opaque":
        if _non_empty_string(ref):
            # An opaque marker is the absence of a canonical source: carrying a
            # ref would present it as one.
            violations.add("opaque_marker_as_provenance")
        reason = record.get("opaque_reason")
        if not _non_empty_string(reason):
            violations.add("opaque_without_reason")
        elif opaque_reasons is None:
            violations.add("missing_closed_vocabulary")
        elif reason not in opaque_reasons:
            violations.add("opaque_reason_outside_vocabulary")

    alignment = record.get("alignment")
    if alignment is not None:
        if alignments is None:
            violations.add("missing_closed_vocabulary")
        elif alignment not in alignments:
            violations.add("alignment_outside_vocabulary")

    return violations


def _candidate_contract_flags(
    contract: dict,
    root: Path,
    dimension: str,
    prefix: str,
    violations_fn,
    *,
    graph_task_suite_required: bool = True,
) -> list[DriftFlag]:
    """Validate positive/negative fixtures + markers for one candidate contract.

    Shared by the visual-lineage (PIR-018), table-provenance (PIR-019),
    equation-coverage (PIR-020) and boundary-positional-metrics (GAR-001)
    checks: the positive fixture must be clean (no violations), every
    negative record must trigger its labelled violation (and every required
    violation must appear somewhere — a future edit that removes a negative
    case is flagged), and the Rust source markers plus the snapshot graph
    task suite must keep the named enforcement points.

    `graph_task_suite_required` defaults to True so a contract without a
    `graph_task_suite` key keeps flagging `*_suite_missing` (fail-closed for
    the existing multimodal contracts). A contract that has no durable
    snapshot-level task suite passes False explicitly, which skips only the
    missing-key case and still validates the suite when the key is present.
    """
    flags: list[DriftFlag] = []
    severity = str(contract.get("severity", "high"))

    def _flag(name: str, evidence: str) -> None:
        flags.append(DriftFlag(
            flag=name, severity=severity, evidence=evidence, dimension=dimension,
        ))

    # Positive fixture: every committed candidate must be clean.
    positive = (contract.get("fixtures") or {}).get("positive") or {}
    positive_path = root / str(positive.get("path", ""))
    if not positive_path.is_file():
        _flag(f"{prefix}_fixture_missing", f"positive fixture missing: {positive_path}")
    else:
        try:
            records = json.loads(positive_path.read_text(encoding="utf-8"))
        except (ValueError, OSError) as error:
            _flag(f"{prefix}_fixture_invalid", f"positive fixture unreadable: {error}")
        else:
            if not isinstance(records, list) or not records:
                _flag(f"{prefix}_fixture_invalid",
                      "positive fixture must be a non-empty JSON array")
            else:
                broken = []
                for record in records:
                    violations = violations_fn(record, contract)
                    if violations:
                        broken.append((record.get("vid", "?"), sorted(violations)))
                if broken:
                    _flag(f"{prefix}_broken",
                          f"positive fixture {positive.get('id')} must be clean, "
                          f"got violations: {broken}")

    # Negative fixture: each record must trigger its labelled violation and
    # every required violation must appear somewhere.
    negative = (contract.get("fixtures") or {}).get("negative") or {}
    negative_path = root / str(negative.get("path", ""))
    if not negative_path.is_file():
        _flag(f"{prefix}_fixture_missing", f"negative fixture missing: {negative_path}")
    else:
        try:
            records = json.loads(negative_path.read_text(encoding="utf-8"))
        except (ValueError, OSError) as error:
            _flag(f"{prefix}_fixture_invalid", f"negative fixture unreadable: {error}")
        else:
            if not isinstance(records, list) or not records:
                _flag(f"{prefix}_fixture_invalid",
                      "negative fixture must be a non-empty JSON array")
            else:
                observed: set[str] = set()
                mismatches = []
                for record in records:
                    label = str(record.get("violation", "unnamed"))
                    if "record" in record:
                        violations = violations_fn(record.get("record") or {}, contract)
                    else:
                        violations = {"missing_record_shape"}
                    observed.update(violations)
                    if label not in violations:
                        mismatches.append(
                            f"{label}: expected to trigger {label}, "
                            f"got {sorted(violations)}"
                        )
                if mismatches:
                    _flag(f"{prefix}_broken",
                          f"negative fixture {negative.get('id')} records do not "
                          f"trigger their labelled violations: {'; '.join(mismatches)}")
                require = list(
                    (negative.get("expect") or {}).get("require_violations", [])
                )
                missing_required = [v for v in require if v not in observed]
                if missing_required:
                    _flag(f"{prefix}_broken",
                          f"negative fixture {negative.get('id')} missing required "
                          f"violation coverage: {missing_required}")

    # Source markers: the Rust enforcement points must stay present.
    for marker_key, entry in (contract.get("source_markers") or {}).items():
        marker_path = root / str(entry.get("path", ""))
        if not marker_path.is_file():
            _flag(f"{prefix}_source_marker_missing",
                  f"{marker_key} source missing: {marker_path}")
            continue
        text = marker_path.read_text(errors="replace")
        missing = [p for p in entry.get("required_patterns", []) if p not in text]
        if missing:
            _flag(f"{prefix}_source_marker_missing",
                  f"{marker_key} source missing enforcement markers: {missing}")

    # Graph task suite: the snapshot-level suite must keep its task markers.
    # A contract that explicitly opts out (`graph_task_suite_required=False`)
    # and declares no suite is skipped; every other combination keeps the
    # original fail-closed behaviour.
    if "graph_task_suite" in contract or graph_task_suite_required:
        suite = contract.get("graph_task_suite") or {}
        suite_path = root / str(suite.get("path", ""))
        if not suite_path.is_file():
            _flag(f"{prefix}_suite_missing", f"graph task suite missing: {suite_path}")
        else:
            text = suite_path.read_text(errors="replace")
            missing = [p for p in suite.get("required_patterns", []) if p not in text]
            if missing:
                _flag(f"{prefix}_suite_broken",
                      f"graph task suite missing markers: {missing}")
    return flags

def _normative_marker_ids(text: str, prefix: str) -> list[str]:
    """The obligation ids declared by `<!-- PROMPT-CONTRACT: ... -->` markers.

    Mirrors the Rust helper in `crates/da-domain/tests/prompt_normative_contract.rs`:
    one marker line per prompt, ids separated by commas, an optional trailing
    annotation after the id (`PR-SUM-003 (v2)`) ignored. A prompt that cites an
    id the register does not identify is register/prompt drift, not a warning.
    """
    ids: list[str] = []
    if not prefix:
        return ids
    for line in text.splitlines():
        if not line.startswith(prefix):
            continue
        rest = line[len(prefix):]
        if rest.endswith(" -->"):
            rest = rest[:-4]
        for token in rest.split(","):
            token = token.strip()
            if token:
                ids.append(token.split()[0])
    return ids


def _deep_has_key(node, key: str) -> bool:
    """True if `key` appears anywhere in a nested dict/list structure.

    Mirrors the Rust verifier's string-level D127 guard
    (`!json.contains("import_eligible")` in `da agent-run verify`): the lock
    is about the key being *absent* from the envelope, not about a boolean
    value. Kept recursive because the committed v1 envelope nests identity /
    budget / evidence blocks several levels deep.
    """
    if isinstance(node, dict):
        if key in node:
            return True
        return any(_deep_has_key(value, key) for value in node.values())
    if isinstance(node, list):
        return any(_deep_has_key(item, key) for item in node)
    return False


def _read_only_agent_envelope_violations(
    envelope: dict, contract: dict
) -> set[str]:
    """Structural violations of one read-only agent-run envelope (v1).

    Python validates envelope *structure* only (D275 / D273 / MEM885):
    format + required top-level fields, terminal-state vocabulary
    (verified / failed_visible), snapshot identity block, budget fields,
    identity block (expected/actual), evidence entries carrying query +
    result_ref, result_records 1:1 with evidence (R087 honesty), and the
    D127 lock (`import_eligible` absent). Digest math (plan -> identity ->
    evidence -> claims -> verification -> result_ref) stays in Rust
    `da agent-run verify`: a structurally clean envelope may still FAIL
    digest verification there (the tampered negative fixture).
    """
    violations: set[str] = set()
    if not isinstance(envelope, dict):
        return {"missing_envelope_object"}
    report = contract.get("report_contract") or {}
    if str(envelope.get("format")) != str(report.get("format")):
        violations.add("wrong_format")
    for field in report.get("required_fields", []):
        if field not in envelope:
            violations.add(f"missing_required_field:{field}")
    run = envelope.get("run") or {}
    terminal = report.get("terminal_states") or []
    if str(run.get("terminal_state")) not in terminal:
        violations.add("invalid_terminal_state")
    # expected_snapshot identity block.
    snapshot = envelope.get("expected_snapshot") or {}
    if not isinstance(snapshot, dict):
        violations.add("missing_snapshot_block")
    else:
        for field in report.get("snapshot_required_fields", []):
            if field not in snapshot:
                violations.add(f"missing_snapshot_field:{field}")
    # run.budget fields.
    budget = run.get("budget") or {}
    if not isinstance(budget, dict):
        violations.add("missing_budget_block")
    else:
        for field in report.get("budget_required_fields", []):
            if field not in budget:
                violations.add(f"missing_budget_field:{field}")
    # Identity blocks: run.expected_identity / run.actual_identity and
    # run.bundle.expected_identity must carry semantic identity.
    identity_blocks = [run.get("expected_identity"), run.get("actual_identity")]
    bundle = run.get("bundle") or {}
    if isinstance(bundle, dict):
        identity_blocks.append(bundle.get("expected_identity"))
    for block in identity_blocks:
        if not isinstance(block, dict):
            violations.add("missing_identity_block")
            continue
        for field in report.get("identity_required_fields", []):
            if field not in block:
                violations.add(f"missing_identity_field:{field}")
    # Evidence entries: each must carry query + result_ref.
    evidence = bundle.get("evidence") if isinstance(bundle, dict) else None
    if not isinstance(evidence, list) or not evidence:
        violations.add("missing_evidence_entries")
    else:
        for entry in evidence:
            if not isinstance(entry, dict):
                violations.add("missing_evidence_fields")
                continue
            for field in report.get("evidence_required_fields", []):
                if field not in entry:
                    violations.add(f"missing_evidence_field:{field}")
        # result_records 1:1 with evidence (R087 honesty).
        records = envelope.get("result_records")
        if not isinstance(records, list) or not records:
            violations.add("missing_result_records")
        elif len(records) != len(evidence):
            violations.add("result_evidence_mismatch")
        else:
            for record in records:
                if not isinstance(record, dict) or "query" not in record:
                    violations.add("missing_result_query")
                    break
    # D127 lock: import_eligible must be absent at any nesting depth.
    if _deep_has_key(envelope, "import_eligible"):
        violations.add("import_eligible_present")
    return violations


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

        # Control-plane health: projection contention, liveness wedges, lease
        # conflicts, an unbaselined doctor, and unbounded runtime stores.
        drift_flags.extend(self._check_gsd_control_plane())

        # Gold evaluation must account for every fixture and every gold entity.
        drift_flags.extend(self._check_gold_completeness())

        # Observability is an onion-aware semantic contract, not a log-call count.
        drift_flags.extend(self._check_logging_architecture())

        # Boundary contracts prevent silent loss of canonical and derived information.
        drift_flags.extend(self._check_information_preservation())

        # R133: shadow reports must keep reference, executed, evaluated and outcome
        # populations non-mixable; corpus-sized numbers without execution evidence
        # fail closed rather than becoming synthetic quality claims.
        drift_flags.extend(self._check_shadow_population_contract())

        # GAR-006: the population split is only half the guarantee — a
        # corpus-scale identity number must also be BOUND to execution evidence
        # for the population it claims, and the relations executed ⊆ reference
        # and evaluated ⊆ executed must hold.
        drift_flags.extend(self._check_executed_population_binding())

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

        # PIR-014/015/016: ACAP runtime control plane — positive/negative
        # committed fixtures keep runtime admission, promotion authority,
        # durable scheduler recovery and replay idempotence enforced with
        # D127 and the optimizer auto-promote lock intact (M399 S04 T02).
        drift_flags.extend(self._check_acap_runtime_control_plane())

        # PIR-018: visual candidate graph lineage contract — positive/negative
        # committed FigureCandidate fixtures keep every visual candidate bound
        # to a PaperRevision VID and revision-scoped Section VID with SHA-256
        # lineage digests, deterministic identity and D127 authority/import
        # locks (M400 S05 T01).
        drift_flags.extend(self._check_visual_candidates_require_graph_lineage())

        # PIR-019: table payload cell provenance contract — positive/negative
        # committed TableCandidate fixtures keep TEI grounding (R087),
        # per-cell grid/digest/span provenance and typed conflict kinds with
        # D127 locks (M400 S05 T01).
        drift_flags.extend(self._check_table_payload_requires_cell_provenance())

        # PIR-020: equation context coverage contract — positive/negative
        # committed EquationCandidate fixtures keep formula identity,
        # revision/section grounding, typed representation and source span
        # with D127 locks (M400 S05 T01).
        drift_flags.extend(self._check_equation_context_coverage())

        # PIR-017/PIR-021: read-only agent plane contract — committed
        # genuine positive/negative v1 envelopes keep the verified read-only
        # agent loop enforced: Python validates structure only (D275 / D273 /
        # MEM885), digest math stays in Rust `da agent-run verify` (pinned by
        # read_only_agent_contract_verify_test.rs), and D127 stays locked
        # (no import_eligible key anywhere in the envelope).
        drift_flags.extend(self._check_read_only_agent_contract())

        # M510/S02: prompt normative contract — the prompt/meta-prompt register
        # keeps identifiers, authority, evidence obligations and allowlisted
        # typed verification operations; this rule executes those allowlisted
        # operations against the real tree and fails closed on a dropped,
        # weakened or unidentified obligation (never a recommendation).
        drift_flags.extend(self._check_prompt_normative_contract())

        # M442 S01: feature-matrix gate pin — the architecture gate must keep
        # compiling documented CLI feature combinations (M441 lesson: the
        # pdf-classifier build stayed broken for a month unseen).
        drift_flags.extend(self._check_feature_matrix_gate())

        # M449: graph semantic validation — catches empty graphs, missing
        # enrichment, stub sections (the "пустышка" problem).
        from governor.adapters.graph_query_reader import check_graph_semantics
        drift_flags.extend(check_graph_semantics(self.root))

        # M449: data integrity — hardcode scan, path canonicality,
        # cache binding, artifact GC.
        from governor.adapters.data_integrity_checker import check_data_integrity
        drift_flags.extend(check_data_integrity(self.root))

        # M451: schema/manifest hierarchy (ADR-068)
        from governor.adapters.schema_hierarchy_checker import check_schema_hierarchy
        drift_flags.extend(check_schema_hierarchy(self.root))

        # S19: source-bound staged scholarly parse governance. The adapter
        # validates only declared DATA/artifact identities and typed fixture
        # fields; it runs in both active and closeout reports.
        drift_flags.extend(self._check_scholarly_parse_plane())

        # GAR-001: boundary positional metrics control-plane rule (M508-g9b5gi
        # S01). Independent oracle over canonical coordinates so a count-only
        # agreement cannot hide permuted boundaries.
        drift_flags.extend(self._check_boundary_positional_metrics())

        # GAR-005: conservation verdict provenance control-plane rule
        # (M509-yzblqx S01). A historic `conserved` verdict returned beside
        # `computed_this_run=false` is the recorded failure: the rule accepts a
        # measured verdict only inside the current-run namespace with the
        # honesty flags asserting the computation, and rejects any unscoped
        # token instead of letting a bare `conserved` read as current.
        drift_flags.extend(self._check_conservation_verdict_provenance())

        # GAR-003: donor visual adapter content preservation control-plane rule
        # (M508-g9b5gi S02). Binds the durable positive-fixture artifact to an
        # independent oracle so a `primitives: Vec::new()` drop path cannot
        # pass unnoticed.
        drift_flags.extend(self._check_visual_adapter_preserves_donor_content())

        # GAR-004: component licence provenance control-plane rule (M508-g9b5gi
        # S03). Every rights position is either verified with resolvable
        # evidence or typed blocked with a mandatory reason, and the three
        # read-only licence surfaces stay claimed exactly once — an unproven
        # right can no longer be assumed compatible.
        drift_flags.extend(self._check_component_license_provenance())

        # GAR-011: oracle digest recomputation control-plane rule (M508-g9b5gi
        # S04). Every digest-like field of the segmentation oracle's three
        # audited surfaces carries exactly one class: `recomputed` with the
        # canonical bytes it digests, or `opaque` with a closed-vocabulary
        # reason that is never accepted as a provenance source. Substituting
        # the canonical bytes while leaving the digest string unchanged can no
        # longer pass silently.
        drift_flags.extend(self._check_oracle_digest_recomputation())

        # GAR-014: bounded cross-automata invalidation map (M509-yzblqx S04).
        # The map is DATA; the rule resolves it and fails when an automaton
        # named by the map has no invalidating input recorded, when a rule
        # names an undeclared automaton, when a trigger class is missing, when
        # a bound evidence source loses its pinned symbols, or when the map
        # grows an executive controller it must never have.
        drift_flags.extend(self._check_fsm_interaction_map())

        # GAR-012: quarantine semantic boundary (M509-yzblqx S04). The staged
        # quarantine establishes input isolation and safety containment only:
        # it is not a quality, relevance or admissibility measurement. The rule
        # binds the existing quarantine definitions in the scholarly parse
        # plane DATA (no second copy), fails when one carries a semantic claim
        # key, and requires every non-claim to route to executed/evaluated
        # evidence or the licence/provenance register rather than back to the
        # scanner.
        drift_flags.extend(self._check_quarantine_semantic_boundary())

        # GAR-015: bounded external holdout package (M510-jsg1pj/S03). A
        # complete package is DATA in the acceptance manifest; the rule fails
        # closed when a declared element is missing, when an unexecuted or
        # bytes_absent external row is read as executed evidence, when prose
        # alone authorizes a citation, or when an executed result is claimed
        # without the recorded human release decision. No executed external
        # probe is claimed here.
        drift_flags.extend(self._check_external_holdout_package())

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

    def _check_shadow_population_contract(self) -> list[DriftFlag]:
        """Fail closed when shadow reports mix corpus identity with execution claims."""
        path = self.root / "data/parse/acceptance_manifest.yaml"
        try:
            manifest = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
            contract = manifest["population_contract"]
            paths = contract["report_paths"]
            required = set(contract["required_blocks"])
            forbidden = set(contract["forbidden_legacy_blocks"])
            executed_required = set(contract["executed_population_required_fields"])
        except (OSError, KeyError, TypeError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="shadow_population_contract_invalid",
                severity="high",
                evidence=f"cannot load population contract: {error}",
                dimension="population_honesty",
            )]

        flags: list[DriftFlag] = []
        for relative in paths:
            source_path = self.root / str(relative)
            if not source_path.is_file():
                flags.append(DriftFlag(
                    flag="shadow_population_report_missing",
                    severity="high",
                    evidence=f"missing report source {relative}",
                    dimension="population_honesty",
                ))
                continue
            text = source_path.read_text(encoding="utf-8", errors="replace")
            # Inspect serialized report keys, not Rust type names such as Corpus.
            serialized = set(re.findall(r'"([a-z_]+)"\s*:', text))
            missing = sorted(required - serialized)
            legacy = sorted(name for name in forbidden if re.search(rf'"{re.escape(name)}"\s*:', text))
            missing_executed = sorted(
                name for name in executed_required
                if not re.search(rf'"{re.escape(name)}"\s*:', text)
            )
            quality_claims = re.search(r'"quality_claims"\s*:\s*\[(?!\s*\])', text)
            if missing or legacy or missing_executed or quality_claims:
                details = (
                    f"{relative}: missing={missing}; legacy={legacy}; "
                    f"missing_executed={missing_executed}; quality_claims_present={bool(quality_claims)}"
                )
                flags.append(DriftFlag(
                    flag="shadow_population_contract_broken",
                    severity="high",
                    evidence=details,
                    dimension="population_honesty",
                ))
        return flags

    def _check_executed_population_binding(self) -> list[DriftFlag]:
        """Evaluate the executed population binding contract (GAR-006).

        Reads `scripts/governor/checks/executed_population_binding.yaml` (DATA)
        and validates both its fixtures and the governed shadow reports against
        the independent Python oracle `executed_population_violations` in
        `governor.adapters.checks.executed_population_binding`. The population
        split alone is presence-only: a report could still publish
        `documents: 589`, `corpus_execution: false`, `execution_evidence: "n/a"`
        and pass. This rule is value- and relation-aware instead:

          * a corpus claim (`corpus_execution: true`, a measured evaluated
            status, or a corpus-scale executed count) requires execution
            evidence; a bare token cannot bind a corpus-scale number, and a
            structured receipt must agree with the executed population's
            identity and count (replay and splice are both refused);
          * `executed ⊆ reference` and `evaluated ⊆ executed` are enforced, so
            a large referenced count carried into the evaluated population
            beside a small execution is caught;
          * absence fails closed: a missing population, a population without a
            numeric count and a measured status with `corpus_execution` not
            literally true each carry their own typed label.

        The governed reports are the shared population contract's report set
        (`data/parse/acceptance_manifest.yaml`), so this rule and the
        population-split contract cannot disagree about which shadow reports
        exist; a report on disk that the contract does not declare is flagged
        rather than silently skipped. The vocabulary is validated first because
        an empty population/evidence vocabulary would make every record clean.
        The shadow reports have no durable snapshot graph task suite, so the
        shared helper is called with `graph_task_suite_required=False`
        explicitly.
        """
        from governor.adapters.checks.executed_population_binding import (
            declared_report_paths,
            executed_population_violations,
            missing_binding_vocabulary,
        )

        path = self.root / "scripts/governor/checks/executed_population_binding.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="executed_population_binding_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="population_honesty",
            )]
        prefix = "executed_population_binding"
        dimension = str(contract.get("dimension") or "population_honesty")
        severity = str(contract.get("severity", "high"))
        flags: list[DriftFlag] = []

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=f"{prefix}_{name}", severity=severity,
                evidence=evidence, dimension=dimension,
            ))

        # Fail closed on an incomplete vocabulary: without the population keys,
        # the evidence kinds and the corpus-scale threshold the oracle cannot
        # tell a bound claim from an unbound one and would accept every record.
        missing = missing_binding_vocabulary(contract)
        if missing:
            _flag("vocabulary_missing",
                  "population/evidence vocabulary is absent or empty, so the "
                  f"oracle would fail open: missing {missing}")

        flags.extend(_candidate_contract_flags(
            contract, self.root, dimension,
            prefix,
            executed_population_violations,
            graph_task_suite_required=False,
        ))

        # ── the governed reports ─────────────────────────────────────────
        report_contract = contract.get("report_contract") or {}
        manifest_path = self.root / str(report_contract.get("path", ""))
        if not manifest_path.is_file():
            # No report set to bind against: the fixtures and source markers
            # above are the whole rule, mirroring the missing-contract
            # convention (absence of the DATA is not itself a failure).
            return flags
        try:
            manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            _flag("report_contract_invalid",
                  f"cannot load the population report contract {manifest_path}: {error}")
            return flags
        declared = declared_report_paths(manifest, contract)
        if not declared:
            _flag("report_contract_invalid",
                  f"{manifest_path} declares no population report paths, so no "
                  "shadow report would be bound to its execution evidence")
            return flags

        # Fail closed on a report on disk that the shared contract does not
        # declare: an unenumerated report is never checked, which is exactly
        # how a corpus-scale number could leave the governed set unnoticed.
        glob_pattern = str(report_contract.get("glob") or "")
        if glob_pattern:
            declared_set = set(declared)
            for found in sorted(self.root.glob(glob_pattern)):
                try:
                    relative = str(found.relative_to(self.root))
                except ValueError:  # pragma: no cover - glob is rooted at self.root
                    relative = str(found)
                if relative not in declared_set:
                    _flag("report_undeclared",
                          f"shadow report {relative} is not declared by "
                          f"{manifest_path}, so it is never bound to execution evidence")

        for relative in declared:
            report_path = self.root / relative
            if not report_path.is_file():
                _flag("report_missing", f"missing shadow report {relative}")
                continue
            try:
                document = json.loads(report_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                _flag("report_invalid", f"unreadable shadow report {relative}: {error}")
                continue
            violations = executed_population_violations(document, contract)
            if violations:
                _flag("report_broken",
                      f"{relative}: populations are not bound to execution "
                      f"evidence: {sorted(violations)}")

        # ── the census document count (M512/S04) ─────────────────────────
        # The count is DATA-bound, never a literal: the expected count is read
        # from the shared manifest's declared partition, the observed count from
        # the governed census artifact's own populations. A mismatch names both
        # paths, and an unresolvable binding fails closed rather than skipping
        # the comparison.
        census = contract.get("census_binding")
        if isinstance(census, dict) and census:
            census_report_rel = str(census.get("report") or "")
            lane_path = str(census.get("manifest_lane") or "")
            measurable_key = str(census.get("measurable_key") or "")
            exception_key = str(census.get("exception_key") or "")
            reference_key = str(census.get("reference_population_key") or "")
            executed_key = str(census.get("executed_population_key") or "")
            documents_key = str(census.get("documents_key") or "documents")
            lane = manifest
            for segment in lane_path.split("."):
                lane = lane.get(segment) if isinstance(lane, dict) else None
            census_report_path = self.root / census_report_rel
            if not census_report_rel or not isinstance(lane, dict) or not measurable_key:
                _flag("census_binding_unresolvable",
                      "census binding cannot resolve "
                      f"{manifest_path}#{lane_path} or census artifact "
                      f"{census_report_rel or '<missing>'}")
            else:
                measurable = lane.get(measurable_key)
                exceptions = lane.get(exception_key)
                if not isinstance(measurable, list) or not isinstance(exceptions, list):
                    _flag("census_binding_unresolvable",
                          f"{manifest_path}#{lane_path}.{measurable_key}/"
                          f"{exception_key} must both declare lists")
                elif not census_report_path.is_file():
                    _flag("census_binding_unresolvable",
                          f"missing census artifact {census_report_rel}")
                else:
                    try:
                        census_report = json.loads(
                            census_report_path.read_text(encoding="utf-8"))
                    except (ValueError, OSError) as error:
                        _flag("census_binding_unresolvable",
                              f"unreadable census artifact {census_report_rel}: {error}")
                    else:
                        expected_reference = len(measurable) + len(exceptions)
                        expected_executed = len(measurable)
                        observed_reference = _population_document_count(
                            census_report.get(reference_key), documents_key)
                        observed_executed = _population_document_count(
                            census_report.get(executed_key), documents_key)
                        if observed_reference is None or observed_executed is None:
                            _flag("census_binding_unresolvable",
                                  f"{census_report_rel} must declare integer "
                                  f"{reference_key}.{documents_key} and "
                                  f"{executed_key}.{documents_key}")
                        else:
                            if observed_reference != expected_reference:
                                _flag("census_reference_population_mismatch",
                                      f"{census_report_rel}#{reference_key}.{documents_key}="
                                      f"{observed_reference} != {manifest_path}#{lane_path}."
                                      f"{measurable_key}+{exception_key}={expected_reference}")
                            if observed_executed != expected_executed:
                                _flag("census_executed_population_mismatch",
                                      f"{census_report_rel}#{executed_key}.{documents_key}="
                                      f"{observed_executed} != {manifest_path}#{lane_path}."
                                      f"{measurable_key}={expected_executed}")
        return flags

    def _check_prompt_normative_contract(self) -> list[DriftFlag]:
        """Evaluate the prompt normative register contract (M510/S02/T02).

        Reads `scripts/governor/checks/prompt_normative_contract.yaml` (DATA) and
        validates both its fixtures and the live `data/prompts/normatives.yaml`
        register against the independent Python oracle
        `prompt_normative_violations` in
        `governor.adapters.checks.prompt_normative_contract`. The register is not
        a README: an obligation that is unidentified, weakened (`must` -> `should`),
        renamed into a duplicate, stripped of its authority/preconditions/evidence,
        or checked by a non-allowlisted operation fails closed and every
        obligation in the register loses authority (D418: validity is recomputed
        from bound content, never trusted from a label).

        Beyond the structural contract this rule is dynamic, not declarative:

          * the register's allowlisted typed verification operations
            (`content_digest_matches`, `marker_text_present`, `manifest_binds_prompt`)
            are executed here in Python against the real tree, so a drifted
            prompt digest or a renamed executed prompt is a control-plane
            failure, not only a Rust unit-test failure;
          * the required obligation ids, authority sources and verification ops
            must stay identified, so the register cannot silently shrink;
          * the committed prompts' `PROMPT-CONTRACT` markers must reference
            registered ids and the marked prompts must carry the
            optimizer-immutable boundary marker.

        Verification operations are identifiers, never shell strings: an op
        outside `verification_execution.ops` is not executed at all (the oracle
        already reports it as `unknown_verification_op`), and every path
        parameter is confined to the repository before any read.
        """
        from governor.adapters.checks.prompt_normative_contract import (
            missing_normative_vocabulary,
            prompt_normative_violations,
        )

        path = self.root / "scripts/governor/checks/prompt_normative_contract.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="prompt_normative_contract_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="prompt_authority",
            )]
        prefix = "prompt_normative_contract"
        dimension = str(contract.get("dimension") or "prompt_authority")
        severity = str(contract.get("severity", "high"))
        flags: list[DriftFlag] = []

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=f"{prefix}_{name}", severity=severity,
                evidence=evidence, dimension=dimension,
            ))

        # A contract with no `must_strength`, required obligation set or marker
        # vocabulary cannot tell a weakened obligation from an enforced one, so
        # it would accept every register. Refuse it instead of failing open.
        missing = missing_normative_vocabulary(contract)
        if missing:
            _flag("vocabulary_missing",
                  "the prompt normative contract is absent or empty, so the "
                  f"oracle would fail open: missing {missing}")

        flags.extend(_candidate_contract_flags(
            contract, self.root, dimension, prefix,
            prompt_normative_violations,
            graph_task_suite_required=False,
        ))

        # ── the live register ────────────────────────────────────────────
        register_contract = contract.get("register") or {}
        register_rel = str(register_contract.get("path") or "")
        if not register_rel:
            _flag("register_missing", "the contract declares no register path")
            return flags
        register_path = self.root / register_rel
        if not register_path.is_file():
            _flag("register_missing", f"missing normative register {register_rel}")
            return flags
        try:
            register = yaml.safe_load(register_path.read_text(encoding="utf-8"))
        except (OSError, yaml.YAMLError) as error:
            _flag("register_invalid",
                  f"unreadable normative register {register_rel}: {error}")
            return flags
        if not isinstance(register, dict):
            _flag("register_invalid",
                  f"normative register {register_rel} must be a mapping")
            return flags

        register_violations = prompt_normative_violations(register, contract)
        if register_violations:
            _flag("register_broken",
                  f"{register_rel} does not satisfy the prompt normative "
                  f"contract: {sorted(register_violations)}")

        entries = [e for e in (register.get("normatives") or []) if isinstance(e, dict)]
        registered_ids = {e.get("id") for e in entries if isinstance(e.get("id"), str)}
        obligations = {e.get("id") for e in entries if e.get("kind") == "obligation"}
        for required in register_contract.get("required_obligations") or []:
            if required not in obligations:
                _flag("obligation_missing",
                      f"{register_rel} no longer identifies required obligation {required}")
        source_ids = {
            source.get("id") for source in (register.get("authority_sources") or [])
            if isinstance(source, dict) and isinstance(source.get("id"), str)
        }
        for required in register_contract.get("required_authority_sources") or []:
            if required not in source_ids:
                _flag("authority_source_missing",
                      f"{register_rel} no longer declares required authority source {required}")
        op_ids = {
            op.get("id") for op in (register.get("verification_ops") or [])
            if isinstance(op, dict) and isinstance(op.get("id"), str)
        }
        for required in register_contract.get("required_verification_ops") or []:
            if required not in op_ids:
                _flag("verification_op_missing",
                      f"{register_rel} no longer allowlists required verification op {required}")

        # ── register/prompt drift ────────────────────────────────────────
        markers = contract.get("markers") or {}
        marker_prefix = str(markers.get("prompt_contract_prefix") or "")
        for rel in markers.get("prompt_marker_paths") or []:
            prompt_path = self.root / str(rel)
            if not prompt_path.is_file():
                _flag("prompt_missing", f"missing governed prompt {rel}")
                continue
            text = prompt_path.read_text(errors="replace")
            for obligation_id in _normative_marker_ids(text, marker_prefix):
                if obligation_id not in registered_ids:
                    _flag("prompt_marker_unknown_obligation",
                          f"{rel} declares contract id {obligation_id} that "
                          f"{register_rel} does not identify")
        immutable_marker = str(markers.get("optimizer_immutable_marker") or "")
        for rel in markers.get("optimizer_immutable_marker_paths") or []:
            prompt_path = self.root / str(rel)
            if not prompt_path.is_file():
                _flag("prompt_missing", f"missing governed prompt {rel}")
                continue
            if immutable_marker and immutable_marker not in prompt_path.read_text(errors="replace"):
                _flag("optimizer_immutable_marker_missing",
                      f"{rel} must carry the optimizer-immutable boundary "
                      f"marker {immutable_marker!r}")

        # ── execute the allowlisted typed operations ─────────────────────
        flags.extend(self._execute_normative_verifications(register, contract, register_rel))
        return flags

    def _execute_normative_verifications(
        self, register: dict, contract: dict, register_rel: str
    ) -> list[DriftFlag]:
        """Execute the register's allowlisted typed operations against the tree.

        This is what makes the register's verification dynamic rather than
        declarative: the operation is an identifier from
        `verification_execution.ops` and its parameters are typed. A shell
        string is never executed: an op outside the allowlist is skipped here
        because the oracle already reports it as `unknown_verification_op`.
        """
        execution = contract.get("verification_execution") or {}
        if not execution.get("enabled", False):
            return []
        allowed = {str(op) for op in execution.get("ops") or []}
        manifest_key = str(execution.get("manifest_prompt_key") or "system_file")
        dimension = str(contract.get("dimension") or "prompt_authority")
        severity = str(contract.get("severity", "high"))
        flags: list[DriftFlag] = []
        for entry in register.get("normatives") or []:
            if not isinstance(entry, dict) or entry.get("kind") != "obligation":
                continue
            entry_id = entry.get("id", "<unnamed>")
            for verification in entry.get("verification") or []:
                if not isinstance(verification, dict):
                    continue
                op = str(verification.get("op") or "")
                if op not in allowed:
                    continue
                params = verification.get("params")
                if not isinstance(params, dict):
                    params = {}
                error = self._run_normative_verification(op, params, manifest_key)
                if error:
                    flags.append(DriftFlag(
                        flag="prompt_normative_contract_verification_failed",
                        severity=severity,
                        evidence=f"{register_rel}:{entry_id}: {error}",
                        dimension=dimension,
                    ))
        return flags

    def _run_normative_verification(
        self, op: str, params: dict, manifest_key: str
    ) -> str | None:
        """Run one allowlisted typed verification operation.

        Returns a human-readable error string, or None when the operation
        holds. Path parameters are confined to the repository before any read,
        so a verification operation is never a generic file-read primitive.
        """
        def _repo_path(value) -> str | None:
            if not isinstance(value, str) or not value:
                return None
            if value.startswith("/") or "://" in value or "\0" in value:
                return None
            if ".." in value.replace("\\", "/").split("/"):
                return None
            return value

        if op == "content_digest_matches":
            rel = _repo_path(params.get("path"))
            algorithm = params.get("algorithm")
            digest = params.get("digest")
            if rel is None:
                return f"content_digest_matches: unsafe path {params.get('path')!r}"
            if algorithm != "sha256":
                return f"content_digest_matches: unsupported algorithm {algorithm!r}"
            target = self.root / rel
            if not target.is_file():
                return f"content_digest_matches: missing file {rel}"
            actual = hashlib.sha256(target.read_bytes()).hexdigest()
            if actual != digest:
                return f"content_digest_matches: {rel} digest {actual} != {digest}"
            return None

        if op == "marker_text_present":
            rel = _repo_path(params.get("path"))
            marker = params.get("marker")
            if rel is None:
                return f"marker_text_present: unsafe path {params.get('path')!r}"
            if not isinstance(marker, str) or not marker:
                return "marker_text_present: marker must be a non-empty string"
            target = self.root / rel
            if not target.is_file():
                return f"marker_text_present: missing file {rel}"
            if marker not in target.read_text(errors="replace"):
                return f"marker_text_present: {rel} does not contain marker {marker!r}"
            return None

        if op == "manifest_binds_prompt":
            rel = _repo_path(params.get("manifest"))
            expected = _repo_path(params.get("expected_prompt"))
            if rel is None:
                return f"manifest_binds_prompt: unsafe manifest path {params.get('manifest')!r}"
            if expected is None:
                return ("manifest_binds_prompt: unsafe prompt path "
                        f"{params.get('expected_prompt')!r}")
            target = self.root / rel
            if not target.is_file():
                return f"manifest_binds_prompt: missing manifest {rel}"
            try:
                manifest = yaml.safe_load(target.read_text(encoding="utf-8")) or {}
            except (OSError, yaml.YAMLError) as error:
                return f"manifest_binds_prompt: unreadable manifest {rel}: {error}"
            bound = ((manifest.get("prompt") or {}).get(manifest_key)
                     if isinstance(manifest, dict) else None)
            if bound != expected:
                return f"manifest_binds_prompt: {rel} binds {bound!r} not {expected!r}"
            return None

        return f"unexecutable verification op {op!r}"

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

    def _check_scholarly_parse_plane(self) -> list[DriftFlag]:
        """Run the S19 source-bound parse-plane adapter."""
        from governor.adapters.checks.scholarly_parse_plane import _check_scholarly_parse_plane
        return _check_scholarly_parse_plane(self.root)

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

        # Qualification fixtures: the measured-value disqualifiers this slice
        # establishes (shared producer digest, weak measured independence,
        # unknown contamination) must fail the contract rather than pass as
        # clean. This consumes measured values; it re-derives no policy.
        qualification = contract.get("qualification_contract") or {}
        q_fixtures = qualification.get("fixtures") or {}
        q_positive = q_fixtures.get("positive") or {}
        q_positive_path = self.root / str(q_positive.get("path", ""))
        if not q_positive_path.is_file():
            _flag("promotion_evidence_contract_fixture_missing",
                  f"qualification positive fixture missing: {q_positive_path}")
        else:
            try:
                q_packages = json.loads(q_positive_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                _flag("promotion_evidence_contract_fixture_invalid",
                      f"qualification positive fixture unreadable: {error}")
            else:
                if not isinstance(q_packages, list) or not q_packages:
                    _flag("promotion_evidence_contract_fixture_invalid",
                          "qualification positive fixture must be a non-empty JSON array")
                else:
                    broken = []
                    for index, package in enumerate(q_packages):
                        if not isinstance(package, dict):
                            broken.append((index, ["malformed_package"]))
                            continue
                        violations = _promotion_qualification_violations(
                            package, contract)
                        if violations:
                            broken.append((index, sorted(violations)))
                    if broken:
                        _flag("promotion_evidence_contract_broken",
                              f"qualification positive fixture {q_positive.get('id')} "
                              f"must be measured-clean, got violations: {broken}")

        q_negative = q_fixtures.get("negative") or {}
        q_negative_path = self.root / str(q_negative.get("path", ""))
        if not q_negative_path.is_file():
            _flag("promotion_evidence_contract_fixture_missing",
                  f"qualification negative fixture missing: {q_negative_path}")
        else:
            try:
                q_records = json.loads(q_negative_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                _flag("promotion_evidence_contract_fixture_invalid",
                      f"qualification negative fixture unreadable: {error}")
            else:
                if not isinstance(q_records, list) or not q_records:
                    _flag("promotion_evidence_contract_fixture_invalid",
                          "qualification negative fixture must be a non-empty JSON array")
                else:
                    observed: set[str] = set()
                    mismatches = []
                    for record in q_records:
                        if not isinstance(record, dict):
                            observed.add("malformed_record")
                            continue
                        label = str(record.get("violation", "unnamed"))
                        package = record.get("package")
                        violations = (
                            _promotion_qualification_violations(package, contract)
                            if isinstance(package, dict)
                            else {"missing_record_shape"}
                        )
                        observed.update(violations)
                        if label not in violations:
                            mismatches.append(
                                f"{label}: expected to trigger {label}, "
                                f"got {sorted(violations)}"
                            )
                    if mismatches:
                        _flag("promotion_evidence_contract_broken",
                              f"qualification negative fixture {q_negative.get('id')} "
                              f"records do not trigger their labelled disqualifiers: "
                              f"{'; '.join(mismatches)}")
                    require = list(
                        (q_negative.get("expect") or {}).get("require_violations", [])
                    )
                    missing_required = [v for v in require if v not in observed]
                    if missing_required:
                        _flag("promotion_evidence_contract_broken",
                              f"qualification negative fixture {q_negative.get('id')} "
                              f"missing required disqualifier coverage: {missing_required}")

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

    def _check_acap_runtime_control_plane(self) -> list[DriftFlag]:
        """Evaluate the data-driven ACAP runtime control-plane contract.

        Reads `scripts/governor/checks/acap_runtime_control_plane.yaml`
        (DATA) and validates the committed positive/negative fixtures for
        runtime admission (PIR-014), promotion authority (PIR-015), durable
        scheduler recovery (PIR-016) and replay idempotence, mirroring the
        same-process proof in `acap_control_plane_test.rs` (M399/S04 T01):
        every positive record must be clean across all four planes; every
        negative record must trigger exactly its labelled violation and every
        required violation must appear in the fixture (a future edit that
        removes a negative case is flagged); the Rust enforcement points must
        keep the named typed guards. The admission FSM transition table is
        read from `article_context_assurance.yaml` (the canonical ACAP
        policy), so the runtime gate mirrors `ArticleContextState::
        can_transition_to` by construction.
        """
        path = self.root / "scripts/governor/checks/acap_runtime_control_plane.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="acap_runtime_control_plane_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="acap_runtime_control_plane",
            )]
        flags: list[DriftFlag] = []
        dimension = str(contract.get("dimension", "acap_runtime_control_plane"))
        severity = str(contract.get("severity", "high"))

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        # Admission legality depends on the canonical ACAP FSM transition
        # table. Fail closed: an unreadable policy means no transition is
        # declared, so every from/to pair is illegal.
        transitions: set = set()
        fsm_policy = str((contract.get("admission") or {}).get("fsm_policy", ""))
        if fsm_policy:
            fsm_path = self.root / fsm_policy
            if fsm_path.is_file():
                try:
                    policy = yaml.safe_load(fsm_path.read_text(encoding="utf-8")) or {}
                    for transition in ((policy.get("fsm") or {}).get("transitions") or []):
                        if isinstance(transition, list) and len(transition) == 2:
                            transitions.add((str(transition[0]), str(transition[1])))
                except (OSError, yaml.YAMLError):
                    pass

        planes = [
            ("admission", _acap_admission_violations, transitions),
            ("promotion", _acap_promotion_authority_violations, None),
            ("scheduler", _acap_scheduler_violations, None),
            ("replay", _acap_replay_violations, None),
        ]
        for plane_key, fn, extra in planes:
            section = contract.get(plane_key) or {}
            fixtures = section.get("fixtures") or {}
            for kind in ("positive", "negative"):
                spec = fixtures.get(kind) or {}
                fixture_path = self.root / str(spec.get("path", ""))
                if not fixture_path.is_file():
                    _flag("acap_runtime_control_plane_fixture_missing",
                          f"{plane_key}/{kind} fixture missing: {fixture_path}")
                    continue
                try:
                    records = json.loads(fixture_path.read_text(encoding="utf-8"))
                except (ValueError, OSError) as error:
                    _flag("acap_runtime_control_plane_fixture_invalid",
                          f"{plane_key}/{kind} fixture unreadable: {error}")
                    continue
                if not isinstance(records, list) or not records:
                    _flag("acap_runtime_control_plane_fixture_invalid",
                          f"{plane_key}/{kind} fixture must be a non-empty JSON array")
                    continue
                if kind == "positive":
                    broken = []
                    for record in records:
                        if not isinstance(record, dict):
                            broken.append(("<non-object>", ["missing_record_fields"]))
                            continue
                        violations = fn(record, contract) if extra is None else fn(record, contract, extra)
                        if violations:
                            identity = record.get(
                                "stage_id",
                                record.get("decision_id", record.get("task_vid", record.get("scenario", "?"))),
                            )
                            broken.append((identity, sorted(violations)))
                    if broken:
                        _flag("acap_runtime_control_plane_broken",
                              f"{plane_key} positive fixture {spec.get('id')} must be clean, "
                              f"got violations: {broken}")
                else:
                    observed: set[str] = set()
                    mismatches = []
                    for record in records:
                        if not isinstance(record, dict):
                            mismatches.append("<non-object>: not a labelled record")
                            continue
                        label = str(record.get("violation", "unnamed"))
                        payload = record.get("record") or {}
                        violations = fn(payload, contract) if extra is None else fn(payload, contract, extra)
                        observed.update(violations)
                        if label not in violations:
                            mismatches.append(
                                f"{label}: expected to trigger {label}, "
                                f"got {sorted(violations)}"
                            )
                    if mismatches:
                        _flag("acap_runtime_control_plane_broken",
                              f"{plane_key} negative fixture {spec.get('id')} records do not "
                              f"trigger their labelled violations: {'; '.join(mismatches)}")
                    require = list(
                        (spec.get("expect") or {}).get("require_violations", [])
                    )
                    missing_required = [v for v in require if v not in observed]
                    if missing_required:
                        _flag("acap_runtime_control_plane_broken",
                              f"{plane_key} negative fixture {spec.get('id')} missing required "
                              f"violation coverage: {missing_required}")

        # Source markers: the Rust enforcement points must stay present.
        for marker_key, entry in (contract.get("source_markers") or {}).items():
            marker_path = self.root / str(entry.get("path", ""))
            if not marker_path.is_file():
                _flag("acap_runtime_control_plane_source_marker_missing",
                      f"{marker_key} source missing: {marker_path}")
                continue
            text = marker_path.read_text(errors="replace")
            missing = [p for p in entry.get("required_patterns", []) if p not in text]
            if missing:
                _flag("acap_runtime_control_plane_source_marker_missing",
                      f"{marker_key} source missing enforcement markers: {missing}")
            forbidden_hits = [p for p in entry.get("forbidden_patterns", []) if p in text]
            if forbidden_hits:
                _flag("acap_runtime_control_plane_source_marker_missing",
                      f"{marker_key} source still contains forbidden markers: {forbidden_hits}")

        # Optional DATA map: present only on the production contract. Isolated
        # fixture tests omit it so they do not need a full pipeline ledger.
        ledger_layers = contract.get("ledger_layers") or {}
        if ledger_layers:
            ledger_path = self.root / str(ledger_layers.get("path", ""))
            if not ledger_path.is_file():
                _flag("acap_runtime_control_plane_ledger_layers_missing",
                      f"pipeline ledger missing: {ledger_path}")
            else:
                try:
                    ledger = yaml.safe_load(ledger_path.read_text(encoding="utf-8")) or {}
                except (OSError, yaml.YAMLError) as error:
                    _flag("acap_runtime_control_plane_ledger_layers_missing",
                          f"cannot load pipeline ledger {ledger_path}: {error}")
                else:
                    stages = {
                        str(stage.get("id")): stage
                        for stage in (ledger.get("stages") or [])
                        if isinstance(stage, dict) and stage.get("id")
                    }
                    for stage_id, expected in (ledger_layers.get("required") or {}).items():
                        actual = (stages.get(str(stage_id)) or {}).get("acap_layer")
                        if actual != expected:
                            _flag(
                                "acap_runtime_control_plane_ledger_layers_missing",
                                f"stage {stage_id} acap_layer={actual!r} expected {expected!r}",
                            )
                    for stage_id in ledger_layers.get("forbidden") or []:
                        if (stages.get(str(stage_id)) or {}).get("acap_layer") is not None:
                            _flag(
                                "acap_runtime_control_plane_ledger_layers_missing",
                                f"residual stage {stage_id} must not declare acap_layer",
                            )
        return flags

    def _check_visual_candidates_require_graph_lineage(self) -> list[DriftFlag]:
        """Evaluate the data-driven visual candidate lineage contract (PIR-018).

        Reads `scripts/governor/checks/visual_candidates_require_graph_lineage.yaml`
        (DATA) and validates the committed FigureCandidate fixtures against
        the revision-grounded candidate contract in `da_domain::figure_candidate`
        (M400 S01, ADR-051): every positive candidate must be clean — full
        revision/section lineage, valid SHA-256 digests, deterministic
        revision-bound identity (VID matches the recomputed lineage), and
        authority/import locks false (D127 / ADR-053); every negative record
        must trigger exactly its labelled violation and every required
        violation must appear in the fixture (a future edit that removes a
        negative case is flagged); the Rust domain and the snapshot graph
        task suite must keep the named enforcement markers.
        """
        path = self.root / "scripts/governor/checks/visual_candidates_require_graph_lineage.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="visual_candidates_require_graph_lineage_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="visual_candidate_lineage",
            )]
        return _candidate_contract_flags(
            contract, self.root, "visual_candidate_lineage",
            "visual_candidates_require_graph_lineage",
            _visual_candidate_violations,
        )

    def _check_boundary_positional_metrics(self) -> list[DriftFlag]:
        """Evaluate the data-driven boundary positional metrics contract (GAR-001).

        Reads `scripts/governor/checks/boundary_positional_metrics.yaml`
        (DATA) and validates the committed segmentation boundary fixtures
        against the shipped Rust runner
        (`crates/da-cli/examples/document_segmentation_shadow.rs`): every
        positive record must be clean — declared count, pairing and
        positional metrics agree with the independent Python oracle that
        recomputes accuracy from the raw canonical
        `[page, block, line, token]` coordinates; every negative record must
        trigger its labelled violation and every required violation must
        appear (so a removed permutation case is flagged); the Rust
        enforcement markers must stay present. Unlike the three multimodal
        contracts this rule has no durable snapshot graph task suite, so the
        shared helper is called with `graph_task_suite_required=False`
        explicitly. The bound source manifest is verified here because
        `_candidate_contract_flags` does not inspect `bindings`.
        """
        path = self.root / "scripts/governor/checks/boundary_positional_metrics.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="boundary_positional_metrics_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="scholarly_parse_plane",
            )]
        flags: list[DriftFlag] = []
        for binding in (contract.get("bindings") or {}).get("source_data", []):
            bound_path = self.root / str(binding.get("path", ""))
            if not bound_path.is_file():
                flags.append(DriftFlag(
                    flag="boundary_positional_metrics_binding_missing",
                    severity="high",
                    evidence=f"source binding missing: {bound_path}",
                    dimension="scholarly_parse_plane",
                ))
                continue
            expected = binding.get("sha256")
            observed = hashlib.sha256(bound_path.read_bytes()).hexdigest()
            # A non-string or non-hex digest is unverifiable and must fail
            # closed: a 64-zero YAML scalar parses as int 0, which a truthy
            # guard would silently skip.
            if not _is_sha256_hex(expected) or str(expected).lower() != observed:
                flags.append(DriftFlag(
                    flag="boundary_positional_metrics_binding_mismatch",
                    severity="high",
                    evidence=(
                        f"{binding.get('path')}: expected {expected!r}, "
                        f"observed {observed}"
                    ),
                    dimension="scholarly_parse_plane",
                ))
        flags.extend(_candidate_contract_flags(
            contract, self.root, "scholarly_parse_plane",
            "boundary_positional_metrics",
            _boundary_positional_violations,
            graph_task_suite_required=False,
        ))
        return flags

    def _check_conservation_verdict_provenance(self) -> list[DriftFlag]:
        """Evaluate the data-driven conservation verdict provenance contract (GAR-005).

        Reads `scripts/governor/checks/conservation_verdict_provenance.yaml`
        (DATA) and validates report observations against the independent Python
        oracle `conservation_provenance_violations` in
        `governor.adapters.checks.conservation_verdict_provenance`: a measured
        conservation verdict is accepted only when the same run declares the
        measurement — the verdict sits under the current-run namespace key,
        that mapping declares the current-run namespace, both honesty flags are
        literally true and `evaluation_state` is not the shipped `unmeasured`
        value. A measured token anywhere else is the pre-fix GAR-005 shape and
        is rejected as `historic_verdict_presented_as_current`. Absence and
        unknown tokens fail closed: a report with no observation at all is
        `provenance_absent`, never compliance.

        The provenance vocabulary is validated first because an empty measured
        vocabulary would make every record clean; flagging it is the difference
        between failing closed and silently passing the whole rule. The P9
        observation has no durable snapshot graph task suite, so the shared
        helper is called with `graph_task_suite_required=False` explicitly.
        """
        from governor.adapters.checks.conservation_verdict_provenance import (
            conservation_provenance_violations,
            missing_provenance_vocabulary,
        )

        path = self.root / "scripts/governor/checks/conservation_verdict_provenance.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="conservation_verdict_provenance_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="scholarly_parse_plane",
            )]
        dimension = str(contract.get("dimension") or "scholarly_parse_plane")
        severity = str(contract.get("severity", "high"))
        flags: list[DriftFlag] = []

        # Fail closed on an incomplete vocabulary: without the measured set the
        # oracle cannot tell a measured verdict from an unknown one and would
        # accept every record.
        missing = missing_provenance_vocabulary(contract)
        if missing:
            flags.append(DriftFlag(
                flag="conservation_verdict_provenance_vocabulary_missing",
                severity=severity,
                evidence=(
                    "provenance vocabulary is absent or empty, so the oracle "
                    f"would fail open: missing {missing}"
                ),
                dimension=dimension,
            ))

        flags.extend(_candidate_contract_flags(
            contract, self.root, dimension,
            "conservation_verdict_provenance",
            conservation_provenance_violations,
            graph_task_suite_required=False,
        ))
        return flags

    def _check_visual_adapter_preserves_donor_content(self) -> list[DriftFlag]:
        """Evaluate the data-driven donor visual adapter preservation contract (GAR-003).

        Reads `scripts/governor/checks/visual_adapter_preserves_donor_content.yaml`
        (DATA) and validates the durable runtime artifact
        `artifacts/parse_compat/visual_positive_fixture_run.json` against it:
        every positive artifact record must be clean under the independent
        Python oracle `_visual_adapter_violations` — donor primitives survive
        into the native result with complete provenance, malformed donor
        errors never collapse into `not_observed` absence, the families that
        must reach an observed head do so, and the non-deliverable receipt
        families stay explicitly declared. Every negative record must trigger
        its labelled violation and every required violation must appear, so a
        future edit that removes the pre-fix drop-path case is flagged. The
        bound fixture sources and the converter source are verified here
        because `_candidate_contract_flags` does not inspect `bindings` or the
        artifact: `..._artifact_stale` catches a converter or fixture edit
        that was not followed by artifact regeneration, and
        `..._source_binding_mismatch` fails closed on a non-hex digest (a
        64-zero YAML scalar parses as int 0, which a truthy guard would skip).
        The adapter has no durable snapshot graph task suite, so the shared
        helper is called with `graph_task_suite_required=False` explicitly.
        """
        path = self.root / "scripts/governor/checks/visual_adapter_preserves_donor_content.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="visual_adapter_preserves_donor_content_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="scholarly_parse_plane",
            )]
        prefix = "visual_adapter_preserves_donor_content"
        dimension = str(contract.get("dimension") or "scholarly_parse_plane")
        severity = str(contract.get("severity", "high"))
        flags: list[DriftFlag] = []

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        # 1. Bound fixture sources: existence plus exact SHA-256.
        bound: dict[str, Path] = {}
        for binding in (contract.get("bindings") or {}).get("fixture_sources", []):
            bound_path = self.root / str(binding.get("path", ""))
            bound[bound_path.stem] = bound_path
            if not bound_path.is_file():
                _flag(f"{prefix}_source_binding_missing",
                      f"fixture source binding missing: {bound_path}")
                continue
            expected = binding.get("sha256")
            observed = hashlib.sha256(bound_path.read_bytes()).hexdigest()
            if not _is_sha256_hex(expected) or str(expected).lower() != observed:
                _flag(f"{prefix}_source_binding_mismatch",
                      f"{binding.get('path')}: expected {expected!r}, observed {observed}")

        # 2. Artifact identity: schema version and a non-empty record list.
        artifact_config = contract.get("artifact") or {}
        artifact_path = self.root / str(artifact_config.get("path", ""))
        artifact = None
        if not artifact_path.is_file():
            _flag(f"{prefix}_artifact_missing", f"artifact missing: {artifact_path}")
        else:
            try:
                artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                artifact = None
                _flag(f"{prefix}_artifact_invalid", f"artifact unreadable: {error}")
            if artifact is not None and not isinstance(artifact, dict):
                artifact = None
                _flag(f"{prefix}_artifact_invalid", "artifact must be a JSON object")
            if isinstance(artifact, dict):
                if artifact.get("schema_version") != artifact_config.get("schema_version"):
                    artifact = None
                    _flag(f"{prefix}_artifact_invalid",
                          "artifact schema_version "
                          f"{artifact.get('schema_version')!r} != "
                          f"{artifact_config.get('schema_version')!r}")
                elif (
                    not isinstance(artifact.get("records"), list)
                    or not artifact["records"]
                ):
                    artifact = None
                    _flag(f"{prefix}_artifact_invalid",
                          "artifact records must be a non-empty JSON array")

        # 3. Coverage, head outcomes, converter and per-record fixture
        #    staleness. Every digest is recomputed at verification time (no
        #    caching between runs), so a stale artifact cannot pass silently.
        if isinstance(artifact, dict):
            records = artifact["records"]
            fixture_families = list(contract.get("fixture_families") or [])
            families_requiring_head = list(
                contract.get("families_requiring_observed_head") or []
            )
            observed_families: set = set()
            for record in records:
                if not isinstance(record, dict):
                    continue
                family = record.get("family")
                observed_families.add(family)
                native = record.get("native")
                if (
                    family in families_requiring_head
                    and (
                        not isinstance(native, dict)
                        or native.get("head_outcome") != "observed"
                    )
                ):
                    declared_head = (
                        native.get("head_outcome") if isinstance(native, dict) else native
                    )
                    _flag(f"{prefix}_artifact_incomplete",
                          f"family {family!r} must reach an observed head, "
                          f"got {declared_head!r}")
            missing_families = [
                family for family in fixture_families
                if family not in observed_families
            ]
            if missing_families:
                _flag(f"{prefix}_artifact_incomplete",
                      f"artifact does not cover fixture families: {missing_families}")

            converter_path = self.root / str(artifact_config.get("converter_source", ""))
            if not converter_path.is_file():
                _flag(f"{prefix}_artifact_stale",
                      f"converter source missing: {converter_path}")
            else:
                observed_converter = hashlib.sha256(converter_path.read_bytes()).hexdigest()
                declared_converter = artifact.get("converter_source_sha256")
                if (
                    not _is_sha256_hex(declared_converter)
                    or str(declared_converter).lower() != observed_converter
                ):
                    _flag(f"{prefix}_artifact_stale",
                          "artifact converter_source_sha256 "
                          f"{declared_converter!r} != converter {observed_converter}")

            for record in records:
                if not isinstance(record, dict):
                    continue
                fixture_id = record.get("fixture_id")
                bound_path = bound.get(str(fixture_id))
                if bound_path is None or not bound_path.is_file():
                    _flag(f"{prefix}_artifact_stale",
                          f"record fixture_id {fixture_id!r} has no bound fixture source")
                    continue
                observed_fixture = hashlib.sha256(bound_path.read_bytes()).hexdigest()
                declared_fixture = record.get("fixture_source_sha256")
                if (
                    not _is_sha256_hex(declared_fixture)
                    or str(declared_fixture).lower() != observed_fixture
                ):
                    _flag(f"{prefix}_artifact_stale",
                          f"record {fixture_id!r} fixture_source_sha256 "
                          f"{declared_fixture!r} != {observed_fixture}")

        flags.extend(_candidate_contract_flags(
            contract, self.root, dimension, prefix,
            _visual_adapter_violations,
            graph_task_suite_required=False,
        ))
        return flags

    def _check_component_license_provenance(self) -> list[DriftFlag]:
        """Evaluate the data-driven component licence provenance contract (GAR-004).

        Reads `scripts/governor/checks/component_license_provenance.yaml` (DATA)
        and binds the canonical `data/parse/license_inventory.yaml` to the
        independent oracle `_license_provenance_violations`:

          1. `bindings.source_data` pins the canonical artifact by SHA-256 and is
             verified here (the shared fixture helper does not inspect
             `bindings`); a non-hex/absent digest fails closed.
          2. The canonical ledger must declare all three closed vocabularies and
             they must match the contract mirrors exactly
             (`..._vocabulary_missing` / `..._vocabulary_drift`), and the
             document-level `redistribution_policy` must keep the five GAR-004
             locks (`..._gpl_linking_permitted` / `..._redistribution_lock_relaxed`).
          3. Every canonical position is re-checked by the oracle (strict mode:
             evidence paths resolve, `blocked_reason` exactly on blocked).
          4. The three read-only surfaces are read through their declared field
             mappings under their own dialect; a surface token outside the
             canonical closed vocabulary is reported as drift.
          5. Relational completeness: every identifier of every surface must be
             claimed exactly once by the canonical `surface_claims`
             (`..._cross_surface_component_unclaimed` /
             `..._cross_surface_claim_duplicated`), and the positive fixture must
             mirror the canonical identity set (`..._fixture_inventory_drift`).

        The rule has no durable snapshot graph task suite, so the shared helper
        is called with `graph_task_suite_required=False` explicitly. Nothing is
        written into any artifact: the oracle receives the repository root and
        the surface identifier sets through private in-memory `_*` keys on a
        copy of the contract.
        """
        path = self.root / "scripts/governor/checks/component_license_provenance.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="component_license_provenance_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="scholarly_parse_plane",
            )]
        prefix = "component_license_provenance"
        dimension = str(contract.get("dimension") or "scholarly_parse_plane")
        severity = str(contract.get("severity", "high"))
        flags: list[DriftFlag] = []

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        # 1. Bound canonical artifact: existence plus exact SHA-256.
        for binding in (contract.get("bindings") or {}).get("source_data", []):
            bound_path = self.root / str(binding.get("path", ""))
            if not bound_path.is_file():
                _flag(f"{prefix}_binding_missing",
                      f"source binding missing: {bound_path}")
                continue
            expected = binding.get("sha256")
            observed = hashlib.sha256(bound_path.read_bytes()).hexdigest()
            # A non-string or non-hex digest is unverifiable and must fail
            # closed: a 64-zero YAML scalar parses as int 0, which a truthy
            # guard would silently skip.
            if not _is_sha256_hex(expected) or str(expected).lower() != observed:
                _flag(f"{prefix}_binding_mismatch",
                      f"{binding.get('path')}: expected {expected!r}, observed {observed}")

        # 2. Canonical inventory identity.
        canonical = contract.get("canonical_inventory") or {}
        inventory_path = self.root / str(canonical.get("path", ""))
        inventory = None
        if not inventory_path.is_file():
            _flag(f"{prefix}_inventory_missing",
                  f"canonical inventory missing: {inventory_path}")
        else:
            try:
                inventory = yaml.safe_load(inventory_path.read_text(encoding="utf-8"))
            except (OSError, yaml.YAMLError) as error:
                inventory = None
                _flag(f"{prefix}_inventory_invalid",
                      f"canonical inventory unreadable: {error}")
            if inventory is not None and not isinstance(inventory, dict):
                inventory = None
                _flag(f"{prefix}_inventory_invalid",
                      "canonical inventory must be a YAML mapping")

        for vocabulary_key in (
            "boundary_vocabulary",
            "license_status_vocabulary",
            "redistribution_status_vocabulary",
        ):
            declared = inventory.get(vocabulary_key) if isinstance(inventory, dict) else None
            mirror = contract.get(vocabulary_key)
            if not isinstance(declared, list) or not declared:
                _flag(f"{prefix}_vocabulary_missing",
                      f"canonical inventory does not declare {vocabulary_key}")
            elif not isinstance(mirror, list) or not mirror or set(declared) != set(mirror):
                _flag(f"{prefix}_vocabulary_drift",
                      f"{vocabulary_key}: inventory {declared!r} != contract {mirror!r}")

        # 3. Document-level redistribution locks (GAR-004).
        if isinstance(inventory, dict):
            for label, detail in _license_policy_violations(
                inventory.get("redistribution_policy")
            ):
                _flag(f"{prefix}_{label}", f"canonical redistribution_policy: {detail}")

        # 4. Read-only surfaces: field-mapped structural validation.
        surface_ids: dict = {}
        for surface in contract.get("surfaces") or []:
            key = str(surface.get("key", ""))
            surface_path = self.root / str(surface.get("path", ""))
            if not surface_path.is_file():
                _flag(f"{prefix}_surface_missing",
                      f"read-only surface missing: {surface_path}")
                continue
            try:
                surface_doc = yaml.safe_load(surface_path.read_text(encoding="utf-8"))
            except (OSError, yaml.YAMLError) as error:
                surface_doc = None
                _flag(f"{prefix}_surface_invalid", f"{key}: unreadable: {error}")
            node = surface_doc
            pointer = surface.get("pointer")
            if pointer:
                node = surface_doc.get(pointer) if isinstance(surface_doc, dict) else None
            entries = None
            if isinstance(node, dict):
                entries = node.get(str(surface.get("entries_key", "")))
            elif isinstance(node, list):
                entries = node
            if not isinstance(entries, list) or not entries:
                _flag(f"{prefix}_surface_invalid",
                      f"{key}: entry list not found")
                continue
            # Surface-local vocabularies are closed dialects: they may use a
            # subset of the canonical vocabularies, never a new token.
            if isinstance(node, dict):
                for vocabulary_key in (
                    "boundary_vocabulary",
                    "license_status_vocabulary",
                    "redistribution_status_vocabulary",
                ):
                    declared = node.get(vocabulary_key)
                    mirror = contract.get(vocabulary_key)
                    if isinstance(declared, list) and declared and isinstance(mirror, list):
                        drifted = [token for token in declared if token not in mirror]
                        if drifted:
                            _flag(
                                f"{prefix}_vocabulary_drift",
                                f"{key}: {vocabulary_key} introduces tokens outside "
                                f"the canonical vocabulary: {drifted}",
                            )
            mapped = dict(surface)
            if isinstance(node, dict):
                mapped["_boundary_vocabulary"] = node.get("boundary_vocabulary")
                mapped["_redistribution_vocabulary"] = node.get(
                    "redistribution_status_vocabulary"
                )
            ids = set()
            for entry in entries:
                entry_id = entry.get(surface.get("id_field")) if isinstance(entry, dict) else None
                if _non_empty_string(entry_id):
                    ids.add(str(entry_id))
                for violation in sorted(
                    _license_surface_entry_violations(entry, mapped, contract)
                ):
                    _flag(f"{prefix}_{violation}", f"{key}#{entry_id}: {violation}")
            surface_ids[key] = ids

        # 5. Canonical positions: independent entry-level oracle.
        components = inventory.get("components") if isinstance(inventory, dict) else None
        if inventory is not None and not isinstance(components, list):
            _flag(f"{prefix}_inventory_invalid",
                  "canonical inventory components must be a list")
            components = []
        components = components if isinstance(components, list) else []
        oracle_contract = dict(contract)
        oracle_contract["_root"] = str(self.root)
        oracle_contract["_surface_ids"] = surface_ids
        for record in components:
            label = record.get("component_id") if isinstance(record, dict) else "?"
            if not isinstance(record, dict):
                record = {}
            for violation in sorted(_license_provenance_violations(record, oracle_contract)):
                detail = _license_violation_detail(record, violation)
                _flag(f"{prefix}_{violation}",
                      f"canonical inventory {label!r}: {violation}"
                      + (f" ({detail})" if detail else ""))

        # 6. Cross-surface relational completeness: every surface identifier is
        #    claimed exactly once by the canonical ledger.
        claim_counts: dict = {}
        for record in components:
            if not isinstance(record, dict):
                continue
            claims = record.get("surface_claims")
            if not isinstance(claims, list):
                continue
            for claim in claims:
                if not isinstance(claim, dict):
                    continue
                surface_key = claim.get("surface")
                claimed_id = claim.get("component_id")
                if surface_key in surface_ids and claimed_id in surface_ids[surface_key]:
                    pair = (surface_key, str(claimed_id))
                    claim_counts[pair] = claim_counts.get(pair, 0) + 1
        for surface_key, ids in surface_ids.items():
            for component_id in sorted(ids):
                count = claim_counts.get((surface_key, component_id), 0)
                if count == 0:
                    _flag(f"{prefix}_cross_surface_component_unclaimed",
                          f"{surface_key}#{component_id} is not claimed by the "
                          "canonical inventory")
                elif count > 1:
                    _flag(f"{prefix}_cross_surface_claim_duplicated",
                          f"{surface_key}#{component_id} is claimed {count} times")

        # 7. The positive fixture must mirror the canonical identity set.
        positive = (contract.get("fixtures") or {}).get("positive") or {}
        positive_path = self.root / str(positive.get("path", ""))
        if positive_path.is_file() and components:
            try:
                fixture_records = json.loads(positive_path.read_text(encoding="utf-8"))
            except (ValueError, OSError):
                fixture_records = None
            if isinstance(fixture_records, list):
                fixture_ids = {
                    record.get("component_id")
                    for record in fixture_records
                    if isinstance(record, dict)
                }
                canonical_ids = {
                    record.get("component_id")
                    for record in components
                    if isinstance(record, dict)
                }
                if fixture_ids != canonical_ids:
                    _flag(
                        f"{prefix}_fixture_inventory_drift",
                        "positive fixture drifts from the canonical inventory: "
                        f"missing={sorted(map(str, canonical_ids - fixture_ids))}, "
                        f"extra={sorted(map(str, fixture_ids - canonical_ids))}",
                    )

        flags.extend(_candidate_contract_flags(
            oracle_contract, self.root, dimension, prefix,
            _license_provenance_violations,
            graph_task_suite_required=False,
        ))
        return flags

    def _check_external_holdout_package(self) -> list[DriftFlag]:
        """Evaluate the bounded external holdout package contract (GAR-015).

        Reads `scripts/governor/checks/external_holdout_package.yaml` (DATA) and
        binds the canonical `data/parse/acceptance_manifest.yaml` to the
        independent oracle `external_holdout_package_violations` in
        `governor.adapters.checks.external_holdout_package`:

          1. `bindings.source_data` pins the canonical manifest by SHA-256 and is
             verified here (the shared fixture helper does not inspect
             `bindings`); a non-hex/absent digest fails closed.
          2. The contract's declared vocabulary is validated first: without it
             the oracle cannot decide package completeness and would accept
             every package, so an absent declaration is flagged rather than
             skipped.
          3. The live manifest's `external_holdout_contract`,
             `release_decision_package` and `classes` sections are assembled into
             the envelope the oracle consumes, and every failure class it
             re-derives is surfaced as `external_holdout_package_<label>`. An
             executed external probe whose recorded human release decision is
             still pending, an unexecuted/bytes-absent row read as evidence, a
             missing declared element or a broken citation lock therefore fail
             closed. Any label the oracle emits that the contract does not
             declare is surfaced as `..._label_undeclared` so the vocabulary
             cannot silently drift.
          4. The human release decision must exist as a real repository artifact:
             the oracle is pure, so the `decision_record` existence check lives
             here (`..._release_decision_artifact_missing`).
          5. The positive fixture must mirror the live declared element set
             (`..._fixture_drift`), so a manifest that renames/re-points an
             element cannot pass beside a stale fixture.

        The package has no durable snapshot graph task suite, so the shared
        helper is called with `graph_task_suite_required=False` explicitly.
        Nothing is written into any artifact.
        """
        from governor.adapters.checks.external_holdout_package import (
            declared_package_elements,
            external_holdout_package_violations,
            missing_external_holdout_vocabulary,
        )

        path = self.root / "scripts/governor/checks/external_holdout_package.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="external_holdout_package_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="scholarly_parse_plane",
            )]
        prefix = "external_holdout_package"
        dimension = str(contract.get("dimension") or "scholarly_parse_plane")
        severity = str(contract.get("severity", "high"))
        flags: list[DriftFlag] = []

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        # 1. Bound canonical manifest: existence plus exact SHA-256.
        for binding in (contract.get("bindings") or {}).get("source_data", []):
            bound_path = self.root / str(binding.get("path", ""))
            if not bound_path.is_file():
                _flag(f"{prefix}_binding_missing",
                      f"source binding missing: {bound_path}")
                continue
            expected = binding.get("sha256")
            observed = hashlib.sha256(bound_path.read_bytes()).hexdigest()
            # A non-string or non-hex digest is unverifiable and must fail
            # closed: a 64-zero YAML scalar parses as int 0, which a truthy
            # guard would silently skip.
            if not _is_sha256_hex(expected) or str(expected).lower() != observed:
                _flag(f"{prefix}_binding_mismatch",
                      f"{binding.get('path')}: expected {expected!r}, observed {observed}")

        # 2. Fail closed on an incomplete vocabulary: without the declared
        #    element set the oracle cannot decide completeness.
        missing = missing_external_holdout_vocabulary(contract)
        if missing:
            _flag(f"{prefix}_vocabulary_missing",
                  f"package vocabulary is absent or empty, so the oracle would "
                  f"fail open: missing {missing}")

        # 3. The live manifest assembled into the oracle envelope.
        canonical = contract.get("canonical_manifest") or {}
        manifest_path = self.root / str(canonical.get("path", ""))
        live_envelope = None
        if not manifest_path.is_file():
            _flag(f"{prefix}_manifest_missing",
                  f"canonical acceptance manifest missing: {manifest_path}")
        else:
            try:
                manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8"))
            except (OSError, yaml.YAMLError) as error:
                manifest = None
                _flag(f"{prefix}_manifest_invalid",
                      f"acceptance manifest unreadable: {error}")
            if isinstance(manifest, dict):
                live_envelope = {
                    "external_holdout_contract": manifest.get("external_holdout_contract"),
                    "release_decision_package": manifest.get("release_decision_package"),
                    "classes": manifest.get("classes"),
                }
                live_violations = external_holdout_package_violations(
                    live_envelope, contract
                )
                declared_labels = set(contract.get("violations") or [])
                for label in sorted(live_violations):
                    _flag(f"{prefix}_{label}",
                          f"live acceptance_manifest.yaml: {label}")
                    if label not in declared_labels:
                        _flag(f"{prefix}_label_undeclared",
                              f"oracle emitted undeclared label {label!r}")
            elif manifest is not None:
                _flag(f"{prefix}_manifest_invalid",
                      "acceptance manifest must be a YAML mapping")

        # 4. The recorded human release decision must be a real artifact.
        if isinstance(live_envelope, dict):
            release = live_envelope.get("release_decision_package")
            human = release.get("human_release_review") if isinstance(release, dict) else None
            decision_record = human.get("decision_record") if isinstance(human, dict) else None
            if _non_empty_string(decision_record) and not (
                self.root / str(decision_record)
            ).is_file():
                _flag(f"{prefix}_release_decision_artifact_missing",
                      f"recorded human release decision artifact missing: {decision_record}")

        # 5. The positive fixture must mirror the live declared element set.
        positive = (contract.get("fixtures") or {}).get("positive") or {}
        positive_path = self.root / str(positive.get("path", ""))
        if isinstance(live_envelope, dict) and positive_path.is_file():
            try:
                fixture_records = json.loads(positive_path.read_text(encoding="utf-8"))
            except (ValueError, OSError):
                fixture_records = None
            if isinstance(fixture_records, list) and fixture_records:
                first = fixture_records[0] if isinstance(fixture_records[0], dict) else {}
                live_ids = declared_package_elements(live_envelope)
                fixture_ids = declared_package_elements(first)
                if not live_ids or live_ids != fixture_ids:
                    _flag(
                        f"{prefix}_fixture_drift",
                        "positive fixture drifts from the live declared element set: "
                        f"live={live_ids}, fixture={fixture_ids}",
                    )

        flags.extend(_candidate_contract_flags(
            contract, self.root, dimension, prefix,
            external_holdout_package_violations,
            graph_task_suite_required=False,
        ))
        return flags

    def _check_oracle_digest_recomputation(self) -> list[DriftFlag]:
        """Evaluate the data-driven oracle digest recomputation contract (GAR-011).

        Reads `scripts/governor/checks/oracle_digest_recomputation.yaml` (DATA)
        and binds the three audited surfaces of the segmentation oracle (the
        reviewed fixture, the canonical byte artifact and the declaration) to
        the independent oracle `_oracle_digest_violations`:

          1. `bindings.source_data` pins the canonical artifact by SHA-256 and is
             verified here (the shared fixture helper does not inspect
             `bindings`); a non-hex/absent digest fails closed.
          2. The contract's closed vocabularies (`classes`,
             `canonicalizations`, `opaque_reasons`, `alignments`,
             `digest_field_patterns`) must be non-empty lists — an absent
             vocabulary means "cannot decide" and is reported as
             `..._missing_closed_vocabulary`.
          3. Every declared digest field is classified by the oracle: it carries
             exactly one class, a recomputed field names its canonical `ref` and
             `canonicalization`, an opaque field names its reason from the closed
             vocabulary and never carries a `ref`.
          4. Every `(surface, pointer)` pair is declared exactly once
             (`..._duplicate_field_binding`).
          5. Discovery closure — every digest-like field reachable under the
             declared `audit_pointers` is claimed by exactly one declaration
             (`..._unclassified_digest_field`), and every declared pointer
             resolves in its surface (`..._declared_field_not_found`). The walk
             is bounded by `audit_pointers`, so a formula string such as
             `canonicalization.digest` can never be mistaken for a digest.
          6. Both-or-neither — a published canonical bytes object carries its
             sibling `sha256`, and a published `sha256` carries the bytes it
             digests (`..._bytes_without_sha256_pair` /
             `..._sha256_without_bytes_pair`). A whole-file `ref` is skipped:
             its bytes and digest are bound by `bindings.source_data`.
          7. Opaque markers are never a canonical source for a recomputed record
             (`..._opaque_used_as_canonical_source`).
          8. Adjudications — a pin adjudicated as `removed_unreferenced_dangling_pin`
             must stay absent (`..._adjudicated_field_reintroduced`); a
             `repinned_to_recomputed_bytes` adjudication is informational
             because the repinned field is already classified.
          9. The positive fixture must mirror the declared identity set
             (`..._fixture_field_drift`).
         10. The shared fixture/marker helper is called with
             `graph_task_suite_required=False` explicitly: this rule has no
             durable snapshot graph task suite.

        The rule has no durable snapshot graph task suite and never recomputes
        a digest in Python — the SHA-256 mathematics lives in the independent
        Rust reader (D273 / MEM885). Nothing is written into the report: the
        oracle receives no private `_*` keys, so the contract travels unchanged.
        """
        path = self.root / "scripts/governor/checks/oracle_digest_recomputation.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="oracle_digest_recomputation_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="scholarly_parse_plane",
            )]
        prefix = "oracle_digest_recomputation"
        dimension = str(contract.get("dimension") or "scholarly_parse_plane")
        severity = str(contract.get("severity", "high"))
        flags: list[DriftFlag] = []

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        # 1. Bound canonical artifact: existence plus exact SHA-256.
        for binding in (contract.get("bindings") or {}).get("source_data", []):
            bound_path = self.root / str(binding.get("path", ""))
            if not bound_path.is_file():
                _flag(f"{prefix}_binding_missing",
                      f"source binding missing: {bound_path}")
                continue
            expected = binding.get("sha256")
            observed = hashlib.sha256(bound_path.read_bytes()).hexdigest()
            # A non-string or non-hex digest is unverifiable and must fail
            # closed: a 64-zero YAML scalar parses as int 0, which a truthy
            # guard would silently skip.
            if not _is_sha256_hex(expected) or str(expected).lower() != observed:
                _flag(f"{prefix}_binding_mismatch",
                      f"{binding.get('path')}: expected {expected!r}, observed {observed}")

        # 2. Contract-level closed vocabularies (fail-closed).
        for vocabulary_key in (
            "classes",
            "canonicalizations",
            "opaque_reasons",
            "alignments",
            "digest_field_patterns",
        ):
            vocabulary = contract.get(vocabulary_key)
            if not isinstance(vocabulary, list) or not vocabulary:
                _flag(f"{prefix}_missing_closed_vocabulary",
                      f"contract does not declare a non-empty {vocabulary_key} list")

        # 3. Audited surfaces: existence, readability and audit-pointer scope.
        surfaces: dict = {}
        for surface in contract.get("audited_surfaces") or []:
            if not isinstance(surface, dict):
                continue
            surface_key = str(surface.get("key", ""))
            surface_path = self.root / str(surface.get("path", ""))
            if not surface_path.is_file():
                _flag(f"{prefix}_surface_missing",
                      f"audited surface missing: {surface_path}")
                continue
            try:
                text = surface_path.read_text(encoding="utf-8")
                document = (
                    json.loads(text)
                    if str(surface.get("format", "yaml")) == "json"
                    else yaml.safe_load(text)
                )
            except (OSError, ValueError, yaml.YAMLError) as error:
                _flag(f"{prefix}_surface_invalid", f"{surface_key}: unreadable: {error}")
                continue
            if document is None:
                _flag(f"{prefix}_surface_invalid", f"{surface_key}: empty document")
                continue
            surfaces[surface_key] = (document, surface)
            for pointer in surface.get("audit_pointers") or []:
                if not _resolve_pointer_values(document, _pointer_segments(pointer)):
                    _flag(f"{prefix}_surface_invalid",
                          f"{surface_key}: audit pointer does not resolve: {pointer!r}")

        declared = _digest_declared_records(contract)

        # 4. Per-field classification by the independent oracle.
        for record in declared:
            current = record if isinstance(record, dict) else {}
            for violation in sorted(_oracle_digest_violations(current, contract)):
                _flag(f"{prefix}_{violation}",
                      "digest_fields "
                      f"field_id={current.get('field_id')!r} "
                      f"surface={current.get('surface')!r} "
                      f"pointer={current.get('pointer')!r}: {violation}")

        # 5. Every (surface, pointer) pair is declared exactly once.
        declared_keys: dict = {}
        for record in declared:
            if not isinstance(record, dict):
                continue
            pair = (str(record.get("surface")), str(record.get("pointer")))
            declared_keys[pair] = declared_keys.get(pair, 0) + 1
        for (surface_key, pointer), count in sorted(declared_keys.items()):
            if count > 1:
                _flag(f"{prefix}_duplicate_field_binding",
                      f"{surface_key}:{pointer} is declared {count} times")

        # 6. Discovery closure: no unclassified digest-like field, no dangling
        #    declaration. The walk is scoped by the audit pointers, never by an
        #    exclusion list.
        patterns = contract.get("digest_field_patterns")
        patterns = patterns if isinstance(patterns, list) else []
        for surface_key, (document, surface) in sorted(surfaces.items()):
            discovered: set = set()
            for pointer in surface.get("audit_pointers") or []:
                for node in _resolve_pointer_values(document, _pointer_segments(pointer)):
                    _discover_digest_fields(
                        node, _pointer_segments(pointer), patterns, discovered
                    )
            for segments in sorted(discovered):
                matches = [
                    record for record in declared
                    if isinstance(record, dict)
                    and str(record.get("surface")) == surface_key
                    and _pointer_patterns_match(
                        _pointer_segments(record.get("pointer")), list(segments)
                    )
                ]
                if len(matches) != 1:
                    _flag(f"{prefix}_unclassified_digest_field",
                          f"{surface_key}:{_pointer_text(segments)} is claimed by "
                          f"{len(matches)} declared records")
            for record in declared:
                if not isinstance(record, dict) or str(record.get("surface")) != surface_key:
                    continue
                if not _resolve_pointer_values(
                    document, _pointer_segments(record.get("pointer"))
                ):
                    _flag(f"{prefix}_declared_field_not_found",
                          f"{surface_key}:{record.get('pointer')!r} "
                          f"(field_id={record.get('field_id')!r}) does not resolve")

        # 7. Both-or-neither on a published canonical bytes object. The object
        #    is the parent of the referenced `bytes`/`sha256` leaf; a whole-file
        #    ref carries no sibling pair and is bound by `bindings.source_data`.
        for record in declared:
            if not isinstance(record, dict) or record.get("class") != "recomputed":
                continue
            ref = record.get("ref")
            if not isinstance(ref, str) or ":" not in ref:
                continue
            ref_surface, _, ref_pointer = ref.partition(":")
            if ref_surface == "self":
                ref_surface = str(record.get("surface"))
            ref_segments = _pointer_segments(ref_pointer)
            if not ref_segments or ref_segments[-1] not in ("bytes", "sha256"):
                continue
            entry = surfaces.get(ref_surface)
            if entry is None:
                continue
            for object_value in _resolve_pointer_values(entry[0], ref_segments[:-1]):
                if not isinstance(object_value, dict):
                    continue
                has_bytes = "bytes" in object_value
                has_sha256 = "sha256" in object_value
                if has_bytes and not has_sha256:
                    _flag(f"{prefix}_bytes_without_sha256_pair",
                          f"field_id={record.get('field_id')!r} ref={ref!r}: canonical "
                          "bytes object publishes no sibling sha256")
                elif has_sha256 and not has_bytes:
                    _flag(f"{prefix}_sha256_without_bytes_pair",
                          f"field_id={record.get('field_id')!r} ref={ref!r}: sha256 "
                          "without the canonical bytes it digests")

        # 8. An opaque marker is never the provenance source of a recomputed
        #    digest: the per-field oracle already reports the record it reads
        #    on, this reports the resolved declaration. Both are fail-closed.
        for record in declared:
            if not isinstance(record, dict) or record.get("class") != "recomputed":
                continue
            matched = _digest_ref_matches(
                record.get("ref"), record.get("surface"), declared
            )
            opaque_hits = [item for item in matched if item.get("class") == "opaque"]
            if opaque_hits:
                _flag(f"{prefix}_opaque_used_as_canonical_source",
                      f"field_id={record.get('field_id')!r} ref={record.get('ref')!r} "
                      "resolves to opaque marker(s) "
                      f"{[item.get('field_id') for item in opaque_hits]}")

        # 9. Adjudicated dangling pins must stay removed.
        for adjudication in contract.get("adjudications") or []:
            if not isinstance(adjudication, dict):
                continue
            if str(adjudication.get("action")) != "removed_unreferenced_dangling_pin":
                continue
            adjudicated_surface = str(adjudication.get("surface"))
            entry = surfaces.get(adjudicated_surface)
            if entry is None:
                continue
            if _resolve_pointer_values(
                entry[0], _pointer_segments(adjudication.get("pointer"))
            ):
                _flag(f"{prefix}_adjudicated_field_reintroduced",
                      f"field_id={adjudication.get('field_id')!r} "
                      f"{adjudicated_surface}:{adjudication.get('pointer')!r} was "
                      "adjudicated as removed; reinstatement must fail until it is "
                      "explicitly classified")

        # 10. The positive fixture must mirror the declared identity set.
        positive = (contract.get("fixtures") or {}).get("positive") or {}
        positive_path = self.root / str(positive.get("path", ""))
        if positive_path.is_file():
            try:
                fixture_records = json.loads(positive_path.read_text(encoding="utf-8"))
            except (ValueError, OSError):
                fixture_records = None
            if isinstance(fixture_records, list):
                fixture_ids = {
                    record.get("field_id")
                    for record in fixture_records
                    if isinstance(record, dict)
                }
                declared_ids = {
                    record.get("field_id")
                    for record in declared
                    if isinstance(record, dict)
                }
                if fixture_ids != declared_ids:
                    _flag(
                        f"{prefix}_fixture_field_drift",
                        "positive fixture drifts from the declared classification: "
                        f"missing={sorted(map(str, declared_ids - fixture_ids))}, "
                        f"extra={sorted(map(str, fixture_ids - declared_ids))}",
                    )

        flags.extend(_candidate_contract_flags(
            contract, self.root, dimension, prefix,
            _oracle_digest_violations,
            graph_task_suite_required=False,
        ))
        return flags

    def _check_table_payload_requires_cell_provenance(self) -> list[DriftFlag]:
        """Evaluate the data-driven table cell provenance contract (PIR-019).

        Reads `scripts/governor/checks/table_payload_requires_cell_provenance.yaml`
        (DATA) and validates the committed TableCandidate fixtures against
        the revision-grounded table contract in `da_domain::table_candidate`
        + `da_domain::table_fidelity` (M400 S02, ADR-051): every positive
        candidate must be clean — TEI grounding (R087), per-cell
        row/column/digest/span provenance, deterministic cell identities,
        typed conflict kinds only, authority/import locks false (D127 /
        ADR-053); every negative record must trigger exactly its labelled
        violation and every required violation must appear in the fixture;
        the Rust domain/fidelity and the snapshot table fidelity suite must
        keep the named enforcement markers.
        """
        path = self.root / "scripts/governor/checks/table_payload_requires_cell_provenance.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="table_payload_requires_cell_provenance_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="table_cell_provenance",
            )]
        return _candidate_contract_flags(
            contract, self.root, "table_cell_provenance",
            "table_payload_requires_cell_provenance",
            _table_candidate_violations,
        )

    def _check_equation_context_coverage(self) -> list[DriftFlag]:
        """Evaluate the data-driven equation context coverage contract (PIR-020).

        Reads `scripts/governor/checks/equation_context_coverage.yaml` (DATA)
        and validates the committed EquationCandidate fixtures against the
        revision-grounded equation contract in `da_domain::equation_candidate`
        (M400 S03, ADR-051): every positive candidate must be clean — formula
        identity, revision/section grounding, typed representation, source
        span, deterministic identity, locks false (D127 / ADR-053); every
        negative record must trigger exactly its labelled violation and every
        required violation must appear in the fixture; the Rust domain and
        the snapshot equation candidate suite must keep the named enforcement
        markers.
        """
        path = self.root / "scripts/governor/checks/equation_context_coverage.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="equation_context_coverage_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="equation_context_coverage",
            )]
        return _candidate_contract_flags(
            contract, self.root, "equation_context_coverage",
            "equation_context_coverage",
            _equation_candidate_violations,
        )

    def _check_read_only_agent_contract(self) -> list[DriftFlag]:
        """Evaluate the data-driven read-only agent plane contract (PIR-017/021).

        Reads `scripts/governor/checks/read_only_agent_contract.yaml` (DATA)
        and validates the committed genuine positive/negative v1 envelopes
        produced by `da agent-run verify` (M401 S05 T01): Python checks
        envelope *structure* only (D275 / D273 / MEM885) — format, required
        fields, terminal-state vocabulary, snapshot/identity/budget/evidence
        blocks, result_records 1:1 with evidence, and the D127 lock
        (`import_eligible` absent) — while digest math (plan -> identity ->
        evidence -> claims -> verification -> result_ref) stays in Rust and
        is pinned by
        `crates/da-cli/tests/read_only_agent_contract_verify_test.rs` (exit
        0 PASS / exit 3 FAIL). The negative fixture is a *structurally
        valid* v1 envelope with a tampered raw row, so Python must stay
        clean and only the Rust verifier FAILs (exit 3).
        `_candidate_contract_flags` is deliberately NOT reused: it expects a
        JSON *array* of labelled candidate records, while each fixture here
        is a single envelope *object*. The Rust enforcement points must keep
        the named verify / evidence-chain / read-plane / serve-graph / FSM
        markers.
        """
        path = self.root / "scripts/governor/checks/read_only_agent_contract.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="read_only_agent_contract_invalid",
                severity="high",
                evidence=f"cannot load {path}: {error}",
                dimension="agent_read_plane",
            )]
        flags: list[DriftFlag] = []
        dimension = str(contract.get("dimension", "agent_read_plane"))
        severity = str(contract.get("severity", "high"))

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        # Fixtures: BOTH the positive (genuine, Rust PASS exit 0) and the
        # negative (tampered raw row, Rust FAIL exit 3) envelope must be
        # structurally clean v1 objects. `_candidate_contract_flags` is NOT
        # reused — it expects a JSON *array* of labelled candidate records,
        # while each fixture here is a single envelope *object*.
        for kind, spec in ((contract.get("fixtures") or {}).items()):
            fixture_path = self.root / str(spec.get("path", ""))
            if not fixture_path.is_file():
                _flag("read_only_agent_contract_fixture_missing",
                      f"{kind} fixture missing: {fixture_path}")
                continue
            try:
                envelope = json.loads(fixture_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                _flag("read_only_agent_contract_fixture_invalid",
                      f"{kind} fixture unreadable: {error}")
                continue
            if not isinstance(envelope, dict):
                _flag("read_only_agent_contract_fixture_invalid",
                      f"{kind} fixture must be a single envelope object "
                      f"(not an array): {type(envelope).__name__}")
                continue
            violations = _read_only_agent_envelope_violations(envelope, contract)
            # D127 string-level lock, mirroring the Rust verifier guard
            # `!json.contains("import_eligible")` (da-cli/src/agent_run.rs).
            if "import_eligible" in fixture_path.read_text(encoding="utf-8"):
                violations.add("import_eligible_present")
            if violations:
                _flag("read_only_agent_contract_broken",
                      f"{kind} fixture {spec.get('id')} must be structurally clean, "
                      f"got violations: {sorted(violations)}")

        # Source markers: the Rust enforcement points must stay present.
        for marker_key, entry in (contract.get("source_markers") or {}).items():
            marker_path = self.root / str(entry.get("path", ""))
            if not marker_path.is_file():
                _flag("read_only_agent_contract_source_marker_missing",
                      f"{marker_key} source missing: {marker_path}")
                continue
            text = marker_path.read_text(errors="replace")
            missing = [p for p in entry.get("required_patterns", []) if p not in text]
            if missing:
                _flag("read_only_agent_contract_source_marker_missing",
                      f"{marker_key} source missing enforcement markers: {missing}")
        return flags

    def _check_feature_matrix_gate(self) -> list[DriftFlag]:
        """Feature-matrix gate pin (M442 S01, M441 lesson).

        Reads `scripts/governor/checks/feature_matrix_gate.yaml` (DATA) and
        verifies that scripts/verify_rust_architecture.sh keeps its feature
        matrix step: documented CLI feature combinations must compile in the
        full gate. The pdf-classifier combination stayed broken from M422 to
        M441 because no verification layer built it; this check makes a
        silent removal of the pin a trajectory flag. Adding new CLI features
        means extending both the gate script block and this YAML.
        """
        path = self.root / "scripts/governor/checks/feature_matrix_gate.yaml"
        if not path.exists():
            return []
        try:
            contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as error:
            return [DriftFlag(
                flag="feature_matrix_gate_invalid",
                severity="medium",
                evidence=f"cannot load {path}: {error}",
                dimension="test_strategy",
            )]
        flags: list[DriftFlag] = []
        dimension = str(contract.get("dimension", "test_strategy"))
        severity = str(contract.get("severity", "medium"))

        def _flag(name: str, evidence: str) -> None:
            flags.append(DriftFlag(
                flag=name, severity=severity, evidence=evidence, dimension=dimension,
            ))

        for marker_key, entry in (contract.get("source_markers") or {}).items():
            marker_path = self.root / str(entry.get("path", ""))
            if not marker_path.is_file():
                _flag("feature_matrix_gate_source_missing",
                      f"{marker_key} source missing: {marker_path}")
                continue
            text = marker_path.read_text(errors="replace")
            missing = [p for p in entry.get("required_patterns", []) if p not in text]
            if missing:
                _flag("feature_matrix_gate_pin_missing",
                      f"{marker_key} gate script lost the feature-matrix markers: "
                      f"{missing} — documented CLI features are no longer compiled by the gate")
        return flags

    def _check_fsm_interaction_map(self) -> list[DriftFlag]:
        """GAR-014: bounded cross-automata invalidation map (M509-yzblqx S04).

        Reads `scripts/governor/checks/fsm_interaction_map.yaml` (DATA) through
        the adapter and returns its violation flags unchanged. The map names
        the invalidating inputs, local transitions, retry preconditions and
        non-authorizing states of the parse, maturity, assurance and
        learning/evaluation automata; the rule fails closed when an automaton
        has no invalidating input recorded or when the map drifts from the
        evidence sources it binds.
        """
        from governor.adapters.checks.fsm_interaction_map import (
            check_fsm_interaction_map,
        )

        return check_fsm_interaction_map(self.root)

    def _check_quarantine_semantic_boundary(self) -> list[DriftFlag]:
        """GAR-012: quarantine stays isolation, not a quality statement.

        Reads `scripts/governor/checks/quarantine_semantic_boundary.yaml` (DATA)
        through the adapter and returns its violation flags unchanged. The rule
        resolves the existing quarantine definitions in the scholarly parse
        plane DATA, fails when a reusable scope lost its `isolation_only`
        annotation or gained a semantic claim key, and fails when a declared
        semantic non-claim is not routed to executed/evaluated evidence or to
        the component licence/provenance register.
        """
        from governor.adapters.checks.quarantine_semantic_boundary import (
            check_quarantine_semantic_boundary,
        )

        return check_quarantine_semantic_boundary(self.root)

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
        flags: list[DriftFlag] = []
        if findings:
            ids = [finding.evidence_id for finding in findings]
            markers = sorted({finding.marker for finding in findings})
            flags.append(
                DriftFlag(
                    flag="gsd_exec_false_success_evidence",
                    severity="high",
                    evidence=(
                        f"{len(findings)} exit-0 GSD exec run(s) contain explicit failure "
                        f"markers after the latest commit: ids={ids}, markers={markers}"
                    ),
                )
            )

        # A timeout is never a success. The 2026-09-20..27 review found captures
        # that reported `exit_code == 0` while `timed_out` was true, which an
        # exit-code-only reader counts as clean. Reported separately from false
        # success because the remedy differs: bound the command, not the wording.
        try:
            timeouts = GsdExecReader(self.root).timeouts_since(since)
        except OSError:
            timeouts = []
        if timeouts:
            zero_exit = [item.evidence_id for item in timeouts if item.exit_code == 0]
            flags.append(
                DriftFlag(
                    flag="gsd_exec_timeout_evidence",
                    severity="high",
                    evidence=(
                        f"{len(timeouts)} GSD exec run(s) hit the timeout cap after the "
                        f"latest commit; exit-0 among them={zero_exit}, "
                        f"ids={[item.evidence_id for item in timeouts]}"
                    ),
                )
            )
        return flags

    def _check_gsd_control_plane(self) -> list[DriftFlag]:
        """Flag the GSD control-plane failure class discovered by the log review.

        Reads `scripts/governor/checks/gsd_control_plane.yaml` (DATA) for the window
        and thresholds, collects observations through `GsdControlPlaneReader`, and
        lets the pure oracle decide. A missing or malformed contract is a `high`
        flag rather than a silent pass, because an empty finding list must mean
        "measured and clean", never "could not measure".
        """
        contract_path = self.root / "scripts/governor/checks/gsd_control_plane.yaml"
        try:
            contract = yaml.safe_load(contract_path.read_text(encoding="utf-8"))
        except (OSError, yaml.YAMLError) as error:
            return [
                DriftFlag(
                    flag="gsd_control_plane_contract_invalid",
                    severity="high",
                    evidence=f"cannot read {contract_path}: {error}",
                    dimension="gsd_control_plane",
                )
            ]

        from governor.adapters.gsd_control_plane_reader import GsdControlPlaneReader
        from governor.checks.gsd_control_plane import evaluate
        from governor.domain.retention import RetentionPolicy

        window_days = contract.get("window_days") if isinstance(contract, dict) else None
        markers = contract.get("journal_markers") if isinstance(contract, dict) else None
        baseline_path = contract.get("doctor_baseline_path") if isinstance(contract, dict) else None
        policy = (
            RetentionPolicy.from_mapping(contract.get("retention"))
            if isinstance(contract, dict)
            else RetentionPolicy()
        )
        # Same commit-boundary rule as the exec-evidence check: a later commit is the
        # boundary that resolves earlier failures, so both signals self-clear.
        commit_since = None
        if self.git_reader and self.git_reader.is_repo():
            from datetime import datetime

            value = self.git_reader.get_git_info().latest_commit_date
            try:
                commit_since = datetime.fromisoformat(value) if value else None
            except ValueError:
                commit_since = None
        try:
            observations = GsdControlPlaneReader(self.root).observe(
                window_days if isinstance(window_days, int) else 7,
                markers if isinstance(markers, dict) else {},
                baseline_path if isinstance(baseline_path, str) else ".gsd/doctor-baseline.json",
                policy,
                commit_since,
            )
        except OSError as error:
            return [
                DriftFlag(
                    flag="gsd_control_plane_observations_incomplete",
                    severity="high",
                    evidence=f"control-plane stores unreadable: {error}",
                    dimension="gsd_control_plane",
                )
            ]

        return [
            DriftFlag(
                flag=f"gsd_control_plane_{finding.signal}",
                severity=finding.severity,
                evidence=finding.evidence,
                dimension="gsd_control_plane",
            )
            for finding in evaluate(contract, observations)
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
            # Control-plane signals each name their own remedy, so the report is
            # actionable rather than merely descriptive.
            if flag.flag.startswith("gsd_control_plane_unretained") or flag.flag == "gsd_control_plane_tmp_scratch_bloat":
                actions.append("Prune GSD runtime stores: python3 -m governor hygiene --apply")
            if flag.flag == "gsd_control_plane_orphan_projection_dir":
                actions.append(
                    "Move artifacts out of the nested .gsd/gsd tree into .gsd/, then remove it."
                )
            if flag.flag == "gsd_control_plane_liveness_wedge":
                actions.append(
                    "Read the wedge's printed recoveryActionId before restarting auto; a "
                    "task-recovery-abort is not cleared by a restart (/gsd recover <id>)."
                )
            if flag.flag == "gsd_control_plane_doctor_new_divergence":
                actions.append(
                    "Review the new doctor divergence; if it is accepted historical debt, "
                    "record it in scripts/governor/checks/gsd_doctor_baseline.json."
                )
            if flag.flag == "gsd_control_plane_commit_hook_failures":
                actions.append(
                    "A commit hook refused a commit: run `cargo fmt` (or the named hook's "
                    "fix), `git add`, then verify with the hook's exact command and commit. "
                    "Switching the hook to auto-fix does not help: pre-commit still aborts "
                    "that commit once, and the engine cannot retry it."
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
