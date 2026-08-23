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


def _candidate_contract_flags(
    contract: dict,
    root: Path,
    dimension: str,
    prefix: str,
    violations_fn,
) -> list[DriftFlag]:
    """Validate positive/negative fixtures + markers for one candidate contract.

    Shared by the visual-lineage (PIR-018), table-provenance (PIR-019) and
    equation-coverage (PIR-020) checks: the positive fixture must be clean
    (no violations), every negative record must trigger its labelled
    violation (and every required violation must appear somewhere — a future
    edit that removes a negative case is flagged), and the Rust source
    markers plus the snapshot graph task suite must keep the named
    enforcement points.
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

        # M451 W4: per-article manifest content invariants (ADR-068)
        from governor.adapters.article_manifest_checker import check_article_manifests
        drift_flags.extend(check_article_manifests(self.root))

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
