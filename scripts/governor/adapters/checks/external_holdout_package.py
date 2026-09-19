"""Independent oracle for the bounded external holdout package DATA rule (GAR-015).

GAR-015 recorded that an optional frozen external probe could be cited as
evidence without the bounded package the finding demands: no declared acceptance
margins, no document-level uncertainty method, no transfer latency/cost
accounting, no probe identity frozen apart from the production corpora, no
invalidation rules, and no machine-recorded human release decision. The finding's
core failure mode is a *citation* failure: an unexecuted external representation
(or an external row whose bytes are absent) read as executed evidence.

M510-jsg1pj/S03/T01 declared the complete package in
``data/parse/acceptance_manifest.yaml`` as DATA (``package_elements`` names the six
canonical elements; the citation rule is explicitly fail-closed). This module is
the CONTROL-PLANE half of that declaration: the Governor rule
``external_holdout_package`` re-derives package completeness and citation
admissibility from the document a package declares about itself.

It is an INDEPENDENT oracle (ADR-051): it shares no code with the Rust data
contract ``validate_external_holdout_contract`` in
``crates/da-parse/tests/data_contracts.rs`` and does not import it. It re-derives
the failure modes from the fields the package declares, which is what makes it
falsifiable against the DATA fixtures rather than a restatement of the Rust
validator.

Semantics
---------
The oracle receives an *envelope*: the manifest sections a package consists of,
keyed as the manifest keys them (``external_holdout_contract``,
``release_decision_package`` and ``classes``). A package is accepted only when
every one of the following holds; each failure class is a typed token:

  * completeness (GAR-015's first demand): the contract's declared six canonical
    element ids are present exactly once, each ``ref`` equals the canonical ref
    and resolves inside the envelope, and the completeness lock
    (``required_before_citation`` + ``citation_blocked_until_complete``) is held.
    Anything missing is ``package_element_missing``; an undeclared id is
    ``package_element_unexpected``; a re-pointed ref is
    ``package_element_ref_drift`` and a dangling one
    ``package_element_ref_unresolved``.
  * acceptance margins declared before execution: ``declared_before_execution``
    true and ``measured`` false (``margin_measured_before_execution``), the
    declared metric set and per-row structure present (``margin_missing``),
    provisional (``margin_not_provisional``), and every row scoped to the local
    proxy rather than a pooled corpus (``pooled_corpus_claim``).
  * a document-level uncertainty method: a non-empty method
    (``uncertainty_method_missing``), the document unit and the declared fields
    (``uncertainty_not_document_level`` / ``uncertainty_fields_missing``).
  * transfer latency and cost accounting: a present block
    (``transfer_accounting_missing``), not measured
    (``transfer_measured_before_execution``), an unknown value never read as zero
    (``unknown_transfer_read_as_zero``), no pooled latency/cost claim
    (``pooled_transfer_claim``) and no transfer claim
    (``transfer_claim_allowed``).
  * a probe identity frozen apart from production corpora
    (``probe_identity_not_frozen`` / ``probe_identity_overlaps_production``).
  * result invalidation rules (``invalidation_rule_missing``) that make a retired
    result non-citable (``retired_result_citable``).
  * the fail-closed citation rule. This is the finding's headline property: while
    the probe is not executed — and for any external row whose state is
    ``bytes_absent`` — the representation is never evidence. Reading it as
    evidence, dropping ``requires_executed_state``, or re-labelling an unexecuted
    row is ``unexecuted_rows_read_as_evidence`` /
    ``bytes_absent_rows_read_as_evidence``; prose alone is
    ``prose_alone_authorizes_citation``; and citation without a recorded human
    release decision is ``citation_without_release_decision``. An executed probe
    whose human decision is still pending is
    ``execution_claimed_without_release``.
  * the recorded human release gate: a required, resolvable ``decision_record``
    (``release_decision_not_recorded``), prose that never authorizes
    (``release_decision_prose_only``), and a claimed recorded decision that names
    its reviewer (``release_decision_claimed_without_record``).
  * the byte policy: external bytes are never loaded
    (``external_bytes_load_permitted``).

Absence is never compliance: a missing vocabulary returns the single guard token
``missing_closed_vocabulary`` (the oracle cannot decide and refuses instead of
accepting every record), and a missing package/release root returns
``package_root_missing``.

The oracle is pure: it resolves ``ref`` strings inside the envelope and never
touches the filesystem, so a fixture is checked without a repository. The
existence of the human release-decision artifact itself is a repository-level fact
and is enforced by the Governor wiring, which knows the project root.
"""

from __future__ import annotations


def _non_empty_string(value) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _resolve_ref(envelope, ref) -> bool:
    """Resolve a dotted ``ref`` inside the envelope (existence only)."""
    if not _non_empty_string(ref):
        return False
    node = envelope
    for segment in str(ref).split("."):
        if not isinstance(node, dict) or segment not in node:
            return False
        node = node[segment]
    return True


def declared_package_elements(envelope) -> dict:
    """The declared ``id -> ref`` package element map of an envelope."""
    if not isinstance(envelope, dict):
        return {}
    package = envelope.get("external_holdout_contract")
    if not isinstance(package, dict):
        return {}
    node = package.get("package_elements")
    if not isinstance(node, dict):
        return {}
    elements = node.get("elements")
    if not isinstance(elements, list):
        return {}
    declared: dict = {}
    for element in elements:
        if not isinstance(element, dict):
            continue
        element_id = element.get("id")
        ref = element.get("ref")
        if _non_empty_string(element_id) and _non_empty_string(ref):
            declared.setdefault(str(element_id), []).append(str(ref))
    return declared


def external_holdout_external_rows(classes) -> list:
    """External rows: ``classes.<class>.rows`` (never a ``local_proxy`` block)."""
    rows: list = []
    if not isinstance(classes, dict):
        return rows
    for class_node in classes.values():
        if not isinstance(class_node, dict):
            continue
        entries = class_node.get("rows")
        if not isinstance(entries, list):
            continue
        rows.extend(entry for entry in entries if isinstance(entry, dict))
    return rows


def missing_external_holdout_vocabulary(contract) -> list[str]:
    """Contract declarations the oracle needs; an absent one means 'cannot decide'.

    A missing element list, token set or scalar would make the corresponding
    check impossible to evaluate, so it is reported instead of skipped: that is
    the difference between failing closed and silently accepting every package.
    """
    missing: list[str] = []
    elements = (contract.get("package_elements") or {}).get("required")
    if not isinstance(elements, list) or not elements:
        missing.append("package_elements.required")
    else:
        for element in elements:
            if (
                not isinstance(element, dict)
                or not _non_empty_string(element.get("id"))
                or not _non_empty_string(element.get("ref"))
            ):
                missing.append("package_elements.required[*]")
                break
    for key in (
        "required_invalidation_keys",
        "required_authorization_gates",
        "document_uncertainty_fields",
        "required_margin_metrics",
        "allowed_margin_directions",
        "allowed_margin_claim_scopes",
        "closed_execution_states",
        "closed_decision_states",
        "recorded_decision_states",
        "closed_external_row_states",
        "violations",
    ):
        value = contract.get(key)
        if not isinstance(value, list) or not value:
            missing.append(key)
    for key in (
        "document_unit",
        "fail_visible_value",
        "forbidden_value",
        "retire_value",
        "bytes_absent_value",
        "executed_value",
        "not_executed_value",
    ):
        if not _non_empty_string(contract.get(key)):
            missing.append(key)
    return missing


def _is_number(value) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def _human_release_review(release: dict) -> dict:
    node = release.get("human_release_review")
    return node if isinstance(node, dict) else {}


def external_holdout_package_violations(record, contract) -> set:
    """Every GAR-015 package/citation failure the envelope declares about itself."""
    violations: set = set()
    if missing_external_holdout_vocabulary(contract):
        return {"missing_closed_vocabulary"}

    document_unit = contract["document_unit"]
    fail_visible = contract["fail_visible_value"]
    forbidden = contract["forbidden_value"]
    retire = contract["retire_value"]
    bytes_absent = contract["bytes_absent_value"]
    executed = contract["executed_value"]
    not_executed = contract["not_executed_value"]
    recorded_states = set(contract["recorded_decision_states"])
    closed_decisions = set(contract["closed_decision_states"])
    closed_execution = set(contract["closed_execution_states"])
    closed_row_states = set(contract["closed_external_row_states"])
    allowed_directions = set(contract["allowed_margin_directions"])
    allowed_claim_scopes = set(contract["allowed_margin_claim_scopes"])

    if not isinstance(record, dict):
        return {"package_root_missing"}
    envelope = record
    package = envelope.get("external_holdout_contract")
    release = envelope.get("release_decision_package")
    if not isinstance(package, dict) or not isinstance(release, dict):
        return {"package_root_missing"}

    # ── 1. package element completeness (the six canonical elements) ──────────
    required = contract["package_elements"]["required"]
    required_refs = {str(element["id"]): str(element["ref"]) for element in required}
    node = package.get("package_elements")
    if not isinstance(node, dict):
        violations.add("package_element_missing")
    else:
        if (
            node.get("required_before_citation") is not True
            or node.get("citation_blocked_until_complete") is not True
        ):
            violations.add("package_completeness_lock_eroded")
        elements = node.get("elements")
        if not isinstance(elements, list):
            elements = []
            violations.add("package_element_missing")
        by_id: dict = {}
        for element in elements:
            if not isinstance(element, dict) or not _non_empty_string(element.get("id")):
                violations.add("package_element_missing")
                continue
            by_id.setdefault(str(element["id"]), []).append(element)
        for element_id in required_refs:
            if element_id not in by_id:
                violations.add("package_element_missing")
            elif len(by_id[element_id]) > 1:
                violations.add("package_element_duplicated")
        for element_id in by_id:
            if element_id not in required_refs:
                violations.add("package_element_unexpected")
        for element_id, declared_elements in by_id.items():
            for element in declared_elements:
                ref = element.get("ref")
                if element_id in required_refs and str(ref or "") != required_refs[element_id]:
                    violations.add("package_element_ref_drift")
                if not _resolve_ref(envelope, ref):
                    violations.add("package_element_ref_unresolved")

    # ── 2. acceptance margins declared before execution ───────────────────────
    margins = package.get("acceptance_margins")
    if not isinstance(margins, dict):
        violations.add("margin_missing")
    else:
        if (
            margins.get("declared_before_execution") is not True
            or margins.get("measured") is not False
        ):
            violations.add("margin_measured_before_execution")
        if margins.get("margins_are_provisional") is not True:
            violations.add("margin_not_provisional")
        if (
            margins.get("failed_or_missing_margin_behavior") != fail_visible
            or margins.get("claim_without_measured_margin") != forbidden
        ):
            violations.add("margin_behavior_not_fail_visible")
        floor = margins.get("resolution_floor_documents")
        if (
            not _non_empty_string(margins.get("basis"))
            or margins.get("unit") != document_unit
            or margins.get("same_n_required") is not True
            or not isinstance(floor, int)
            or isinstance(floor, bool)
            or floor < 1
        ):
            violations.add("margin_missing")
        rows = margins.get("margins")
        if not isinstance(rows, list) or not rows:
            rows = []
            violations.add("margin_missing")
        seen_metrics = set()
        for row in rows:
            if not isinstance(row, dict):
                violations.add("margin_missing")
                continue
            metric = row.get("metric")
            if _non_empty_string(metric):
                seen_metrics.add(str(metric))
            if (
                not _non_empty_string(metric)
                or row.get("direction") not in allowed_directions
                or not _is_number(row.get("margin"))
                or not 0 < float(row["margin"]) <= 1
                or not _non_empty_string(row.get("tolerance"))
            ):
                violations.add("margin_missing")
            if row.get("claim_scope") not in allowed_claim_scopes:
                violations.add("pooled_corpus_claim")
        for metric in contract["required_margin_metrics"]:
            if metric not in seen_metrics:
                violations.add("margin_missing")

    # ── 3. document-level uncertainty method ─────────────────────────────────
    reporting = package.get("document_level_reporting")
    if not isinstance(reporting, dict):
        violations.add("uncertainty_method_missing")
    else:
        if not _non_empty_string(reporting.get("uncertainty_method")):
            violations.add("uncertainty_method_missing")
        if (
            reporting.get("unit") != document_unit
            or reporting.get("report_required_before_execution_claim") is not True
        ):
            violations.add("uncertainty_not_document_level")
        fields = reporting.get("fields")
        declared_fields = set(fields) if isinstance(fields, list) else set()
        if not set(contract["document_uncertainty_fields"]).issubset(declared_fields):
            violations.add("uncertainty_fields_missing")

    # ── 4. transfer latency and cost accounting ──────────────────────────────
    transfer = package.get("transfer_accounting")
    if not isinstance(transfer, dict):
        violations.add("transfer_accounting_missing")
    else:
        if transfer.get("measured") is not False:
            violations.add("transfer_measured_before_execution")
        latency = transfer.get("latency")
        cost = transfer.get("cost")
        if not isinstance(latency, dict) or not isinstance(cost, dict):
            violations.add("transfer_accounting_missing")
        else:
            if latency.get("measured") is not False or cost.get("measured") is not False:
                violations.add("transfer_measured_before_execution")
            if (
                latency.get("unknown_latency_is_not_zero") is not True
                or cost.get("unknown_cost_is_not_zero") is not True
            ):
                violations.add("unknown_transfer_read_as_zero")
            if (
                latency.get("pooled_latency_claim") != forbidden
                or cost.get("pooled_cost_claim") != forbidden
            ):
                violations.add("pooled_transfer_claim")
            timeout = latency.get("declared_timeout_secs")
            if (
                not _is_number(timeout)
                or float(timeout) <= 0
                or not _non_empty_string(cost.get("reference"))
            ):
                violations.add("transfer_accounting_missing")
        if (
            transfer.get("transfer_bytes_loaded") is not False
            or transfer.get("transfer_claim_allowed") is not False
        ):
            violations.add("transfer_claim_allowed")

    # ── 5. probe identity frozen apart from production corpora ───────────────
    identity = package.get("identity_binding")
    if not isinstance(identity, dict):
        violations.add("probe_identity_not_frozen")
    else:
        if (
            identity.get("frozen_before_execution") is not True
            or identity.get("frozen_apart_from_production_corpora") is not True
            or identity.get("mismatch_behavior") != fail_visible
            or identity.get("identity_source") != "external_authority_identity_only"
        ):
            violations.add("probe_identity_not_frozen")
        if identity.get("production_corpus_identity_overlap") != forbidden:
            violations.add("probe_identity_overlaps_production")

    # ── 6. result invalidation rules ──────────────────────────────────────────
    invalidation = package.get("result_invalidation")
    if not isinstance(invalidation, dict):
        violations.add("invalidation_rule_missing")
    else:
        if any(
            invalidation.get(key) != retire
            for key in contract["required_invalidation_keys"]
        ) or invalidation.get("stale_result_behavior") != fail_visible:
            violations.add("invalidation_rule_missing")
        if invalidation.get("retired_result_may_be_cited") is not False:
            violations.add("retired_result_citable")

    # ── 7. fail-closed citation rule + byte policy ───────────────────────────
    citation = package.get("citation_rule")
    if not isinstance(citation, dict):
        citation = {}
        violations.add("citation_without_release_decision")
    if (
        citation.get("requires_executed_state") is not True
        or citation.get("citation_of_unexecuted_external_rows") != forbidden
    ):
        violations.add("unexecuted_rows_read_as_evidence")
    if citation.get("unexecuted_external_rows_are_evidence") is not False:
        violations.add("unexecuted_rows_read_as_evidence")
    if citation.get("bytes_absent_rows_are_evidence") is not False:
        violations.add("bytes_absent_rows_read_as_evidence")
    if citation.get("prose_alone_authorizes_citation") is not False:
        violations.add("prose_alone_authorizes_citation")
    if (
        citation.get("requires_recorded_human_release_decision") is not True
        or citation.get("citation_without_release_decision") != forbidden
    ):
        violations.add("citation_without_release_decision")

    byte_policy = package.get("byte_policy")
    if (
        not isinstance(byte_policy, dict)
        or byte_policy.get("external_bytes") != bytes_absent
        or byte_policy.get("load_external_bytes") != forbidden
        or byte_policy.get("download_external_bytes") != forbidden
        or byte_policy.get("materialize_external_files") != forbidden
    ):
        violations.add("external_bytes_load_permitted")
    if isinstance(byte_policy, dict) and byte_policy.get("external_rows_are_evidence") is not False:
        violations.add("bytes_absent_rows_read_as_evidence")

    execution_status = package.get("execution_status")
    if execution_status not in closed_execution:
        violations.add("execution_state_unknown")
    elif execution_status == executed and _human_release_review(release).get(
        "decision"
    ) not in recorded_states:
        violations.add("execution_claimed_without_release")
    elif execution_status == not_executed and _human_release_review(release).get(
        "decision"
    ) not in closed_decisions:
        violations.add("release_decision_not_recorded")

    external_rows = external_holdout_external_rows(envelope.get("classes"))
    if any(row.get("state") not in closed_row_states for row in external_rows):
        violations.add("unexecuted_rows_read_as_evidence")
    if (
        any(row.get("state") == bytes_absent for row in external_rows)
        and isinstance(byte_policy, dict)
        and byte_policy.get("external_bytes") != bytes_absent
    ):
        violations.add("bytes_absent_rows_read_as_evidence")

    # ── 8. the recorded human release gate ───────────────────────────────────
    human = _human_release_review(release)
    if human.get("required") is not True or not _non_empty_string(
        human.get("decision_record")
    ):
        violations.add("release_decision_not_recorded")
    gates = human.get("authorization_requires")
    declared_gates = set(gates) if isinstance(gates, list) else set()
    if (
        human.get("prose_alone_authorizes_release") is not False
        or not set(contract["required_authorization_gates"]).issubset(declared_gates)
    ):
        violations.add("release_decision_prose_only")
    decision_value = human.get("decision")
    if decision_value not in closed_decisions:
        violations.add("release_decision_not_recorded")
    elif decision_value in recorded_states and not _non_empty_string(
        human.get("reviewer")
    ):
        violations.add("release_decision_claimed_without_record")

    return violations
