# Formerly: src/arxiv_archive/chunk_repair_contract.py

"""Review-only chunk repair contract validation.

This module implements the M022/S02 executable contract for chunk and section
repair evidence. It validates JSON-like dictionaries without reading source
files, generating embeddings, importing KG facts, or writing production storage.
Diagnostics are intentionally path/code based and never include payload values.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any

from research_graph.repair.candidate_locators import (
    ALLOWED_CANDIDATE_TYPES,
    ALLOWED_COORDINATE_SPACES,
    ALLOWED_ROUTES,
    ALLOWED_STATES,
    ALLOWED_USES,
    EXCLUDED_USES,
    FORBIDDEN_PAYLOAD_KEYS,
)

CHUNK_REPAIR_CONTRACT_VERSION = "chunk-repair-contract.v1"
CHUNK_REPAIR_VALIDATION_VERSION = "chunk-repair-contract-validation.v1"

ALLOWED_REPAIR_KINDS = frozenset(
    {
        "chunk_span_repair",
        "section_route_review",
        "section_boundary_repair",
        "retrieval_only_review",
    }
)
ALLOWED_REVIEW_STATUSES = frozenset({"pending_review", "accepted", "rejected", "needs_revision"})
REQUIRED_TARGET_FIELDS = (
    "target_id",
    "paper_id",
    "locator_id",
    "repair_kind",
    "candidate_type",
    "route",
    "state",
    "review_status",
    "section_path",
    "source_artifact_refs",
    "source_spans",
    "diagnostic_codes",
    "allowed_uses",
    "excluded_uses",
    "safety_boundaries",
    "reviewer",
)
REQUIRED_SPAN_FIELDS = ("span_id", "source_id", "coordinate_space", "char_start", "char_end", "span_hash")
REQUIRED_FALSE_SAFETY_FIELDS = (
    "import_eligible",
    "promoted_to_fact",
    "trusted_kg_import_allowed",
    "production_write_attempted",
    "ladybugdb_written",
    "semantic_ready_for_kg",
    "raw_text_included",
    "chunk_text_included",
    "embeddings_included",
    "vectors_included",
    "secrets_included",
)
REQUIRED_DIAGNOSTIC_FIELDS = (
    "target_count",
    "pending_review_count",
    "accepted_count",
    "import_eligible_count",
    "promoted_to_fact_count",
    "production_write_count",
    "semantic_ready_count",
    "raw_text_included",
    "chunk_text_included",
    "embeddings_included",
    "vectors_included",
    "secrets_included",
    "ladybugdb_written",
    "production_import_attempted",
)
ACCEPTED_REVIEWER_FIELDS = ("reviewer_id", "reviewed_at", "decision_summary", "evidence_checked")
RETRIEVAL_ONLY_ROUTES = frozenset({"retrieval_context"})
REPAIR_ROUTES = frozenset({"repair_context"})
SAFE_HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")
MARKDOWN_FORBIDDEN_PATTERNS = ("```", "raw_text", "chunk_text", "paper_text", "claim_text", "embedding=", "vector=", "api_key", "token=")


@dataclass(frozen=True)
class ForbiddenPayloadKeyFinding:
    """One redacted forbidden-key finding."""

    path: str
    key: str
    code: str


@dataclass(frozen=True)
class ChunkRepairDiagnostic:
    """One structured, redacted chunk repair contract finding."""

    code: str
    path: str
    object_id: str | None = None
    object_type: str | None = None
    route: str | None = None
    blocks_import: bool = True


@dataclass(frozen=True)
class ChunkRepairValidationResult:
    """Validation outcome for one chunk repair contract payload."""

    valid_contract: bool
    target_count: int
    import_eligible_count: int
    production_write_count: int
    semantic_ready_count: int
    diagnostics: list[ChunkRepairDiagnostic]

    @property
    def passed(self) -> bool:
        """True when the repair contract is structurally valid and safe."""
        return self.valid_contract and not self.diagnostics

    @property
    def refusal_counts(self) -> dict[str, int]:
        """Diagnostic code counts for review surfaces and tests."""
        counts: dict[str, int] = {}
        for diagnostic in self.diagnostics:
            counts[diagnostic.code] = counts.get(diagnostic.code, 0) + 1
        return dict(sorted(counts.items()))


def validate_chunk_repair_contract(payload: dict[str, Any], expected_audit: dict[str, Any] | None = None) -> ChunkRepairValidationResult:
    """Validate one review-only chunk repair contract payload.

    ``expected_audit`` may provide ``locator_ids``, ``source_ids``, and
    ``paper_ids`` collections from a prior locator audit. Missing or malformed
    audit collections produce diagnostics rather than exceptions.
    """
    diagnostics: list[ChunkRepairDiagnostic] = []
    if not isinstance(payload, dict):
        return ChunkRepairValidationResult(
            valid_contract=False,
            target_count=0,
            import_eligible_count=0,
            production_write_count=0,
            semantic_ready_count=0,
            diagnostics=[ChunkRepairDiagnostic(code="payload_not_object", path="/", object_type="contract")],
        )

    diagnostics.extend(_validate_header(payload))
    diagnostics.extend(_diagnostics_from_forbidden_keys(scan_forbidden_payload_keys(payload)))

    expected = _expected_sets(expected_audit)
    diagnostics.extend(expected["diagnostics"])

    source_ids = expected["source_ids"] if expected["source_ids"] is not None else _source_ids(payload.get("source_ledger"))

    repair_targets = _list_of_dicts(payload.get("repair_targets"))
    if not isinstance(payload.get("repair_targets"), list):
        diagnostics.append(ChunkRepairDiagnostic(code="missing_repair_targets", path="/repair_targets", object_type="contract"))

    import_eligible_count = 0
    production_write_count = 0
    semantic_ready_count = 0
    pending_review_count = 0
    accepted_count = 0
    promoted_to_fact_count = 0

    for index, target in enumerate(repair_targets):
        path = f"/repair_targets/{index}"
        safety = target.get("safety_boundaries")
        if isinstance(safety, dict):
            import_eligible_count += int(safety.get("import_eligible") is True)
            production_write_count += int(safety.get("production_write_attempted") is True or safety.get("ladybugdb_written") is True)
            semantic_ready_count += int(safety.get("semantic_ready_for_kg") is True)
            promoted_to_fact_count += int(safety.get("promoted_to_fact") is True)
        if target.get("review_status") == "pending_review":
            pending_review_count += 1
        if target.get("review_status") == "accepted":
            accepted_count += 1
        diagnostics.extend(
            _validate_target(
                target,
                path=path,
                package_paper_id=_string_or_none(payload.get("paper_id")),
                known_source_ids=source_ids,
                expected_locator_ids=expected["locator_ids"],
                expected_paper_ids=expected["paper_ids"],
            )
        )

    diagnostics.extend(
        _validate_contract_diagnostics(
            payload.get("diagnostics"),
            target_count=len(repair_targets),
            pending_review_count=pending_review_count,
            accepted_count=accepted_count,
            import_eligible_count=import_eligible_count,
            promoted_to_fact_count=promoted_to_fact_count,
            production_write_count=production_write_count,
            semantic_ready_count=semantic_ready_count,
        )
    )

    return ChunkRepairValidationResult(
        valid_contract=not diagnostics,
        target_count=len(repair_targets),
        import_eligible_count=import_eligible_count,
        production_write_count=production_write_count,
        semantic_ready_count=semantic_ready_count,
        diagnostics=diagnostics,
    )


def validation_to_dict(result: ChunkRepairValidationResult) -> dict[str, Any]:
    """Serialize validation results without raw text, vectors, or secrets."""
    return {
        "schema_version": CHUNK_REPAIR_VALIDATION_VERSION,
        "contract_version": CHUNK_REPAIR_CONTRACT_VERSION,
        "valid_contract": result.valid_contract,
        "passed": result.passed,
        "target_count": result.target_count,
        "import_eligible_count": result.import_eligible_count,
        "production_write_count": result.production_write_count,
        "semantic_ready_count": result.semantic_ready_count,
        "refusal_counts": result.refusal_counts,
        "diagnostics": [diagnostic.__dict__ for diagnostic in result.diagnostics],
        "raw_text_included": False,
        "chunk_text_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def build_chunk_repair_contract_from_audit(audit: dict[str, Any], *, source_audit_path: str) -> dict[str, Any]:
    """Build a review-only empty repair contract from one redacted S01 audit."""
    audit_diagnostics = validate_locator_evidence_audit_for_repair_contract(audit)
    if audit_diagnostics:
        codes = ", ".join(diagnostic.code for diagnostic in audit_diagnostics)
        raise ValueError(f"source audit is not eligible for chunk repair contract rendering: {codes}")

    stable_ids = audit["stable_ids"]
    invariants = audit["first_proof_invariants"]
    source_ids = [str(source_id) for source_id in stable_ids["source_ids"]]
    source_ledger = [
        {
            "source_id": source_id,
            "paper_id": _paper_id_from_source_id(source_id),
            "source_hash_present": True,
            "source_hash_algorithm": "sha256",
            "source_payload_embedded": False,
            "source_binary_embedded": False,
        }
        for source_id in source_ids
    ]
    diagnostics = {
        "target_count": 0,
        "pending_review_count": 0,
        "accepted_count": 0,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "production_write_count": 0,
        "semantic_ready_count": 0,
        "raw_text_included": False,
        "chunk_text_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }
    return {
        "schema_version": CHUNK_REPAIR_CONTRACT_VERSION,
        "contract_version": CHUNK_REPAIR_CONTRACT_VERSION,
        "run_id": "m022-s02-chunk-repair-contract-from-s01-audit",
        "created_at": "not-recorded",
        "paper_id": "m021-s01-audit-batch",
        "source_audit": {
            "schema_version": audit["schema_version"],
            "input_schema_version": audit["input_schema_version"],
            "strict": audit["strict"],
            "audit_path": source_audit_path,
            "locator_input_path": audit.get("input_path"),
            "redacted": True,
        },
        "expected_invariants": dict(sorted(invariants.items())),
        "stable_id_counts": {
            "paper_count": invariants["paper_count"],
            "source_count": len(source_ids),
            "locator_count": len(stable_ids["locator_ids"]),
            "span_count": len(stable_ids["span_ids"]),
            "ambiguous_span_count": invariants["ambiguous_span_count"],
            "retrieval_only_count": invariants["retrieval_only_count"],
        },
        "stable_ids": {
            "source_ids": source_ids,
            "locator_ids": [str(locator_id) for locator_id in stable_ids["locator_ids"]],
            "span_ids": [str(span_id) for span_id in stable_ids["span_ids"]],
        },
        "source_ledger": source_ledger,
        "safety_boundary": {
            "review_only": True,
            "repair_targets_created": False,
            "import_eligible": False,
            "promoted_to_fact": False,
            "trusted_kg_import_allowed": False,
            "production_write_attempted": False,
            "ladybugdb_written": False,
            "semantic_ready_for_kg": False,
            "source_payloads_included": False,
            "embeddings_included": False,
            "vectors_included": False,
            "secrets_included": False,
        },
        "allowed_vocabularies": {
            "repair_kinds": sorted(ALLOWED_REPAIR_KINDS),
            "review_statuses": sorted(ALLOWED_REVIEW_STATUSES),
            "candidate_types": sorted(ALLOWED_CANDIDATE_TYPES),
            "routes": sorted(ALLOWED_ROUTES),
            "states": sorted(ALLOWED_STATES),
            "coordinate_spaces": sorted(ALLOWED_COORDINATE_SPACES),
            "allowed_uses": list(ALLOWED_USES),
            "excluded_uses": list(EXCLUDED_USES),
        },
        "target_field_requirements": {
            "required_target_fields": list(REQUIRED_TARGET_FIELDS),
            "required_span_fields": list(REQUIRED_SPAN_FIELDS),
            "required_false_safety_fields": list(REQUIRED_FALSE_SAFETY_FIELDS),
            "accepted_reviewer_fields": list(ACCEPTED_REVIEWER_FIELDS),
        },
        "reviewer_output_requirements": [
            "Use stable source, locator, and span identifiers from this contract only.",
            "Record review status using the allowed review vocabulary.",
            "Record section paths as labels or headings, not source snippets.",
            "Keep every import, fact promotion, semantic readiness, and production write flag false.",
            "Reference evidence by stable IDs, coordinate spans, and hashes only.",
        ],
        "explicit_blockers": [
            "No repair target is created by this renderer.",
            "KG import eligibility remains false for every future repair target.",
            "Fact promotion and semantic KG readiness remain blocked.",
            "Production LadybugDB writes are not allowed from this contract.",
            "Corpus snippets, model payloads, embeddings, vectors, and secrets are not allowed in artifacts.",
        ],
        "repair_targets": [],
        "diagnostics": diagnostics,
    }


def validate_locator_evidence_audit_for_repair_contract(audit: dict[str, Any]) -> list[ChunkRepairDiagnostic]:
    """Validate that one redacted S01 audit can seed the S02 repair contract."""
    diagnostics: list[ChunkRepairDiagnostic] = []
    if not isinstance(audit, dict):
        return [ChunkRepairDiagnostic(code="audit_not_object", path="/", object_type="source_audit")]
    if audit.get("schema_version") != "locator_evidence_audit.v1":
        diagnostics.append(ChunkRepairDiagnostic(code="audit_schema_version_mismatch", path="/schema_version", object_type="source_audit"))
    if audit.get("strict") is not True:
        diagnostics.append(ChunkRepairDiagnostic(code="audit_not_strict", path="/strict", object_type="source_audit"))
    invariants = audit.get("first_proof_invariants")
    if not isinstance(invariants, dict):
        diagnostics.append(ChunkRepairDiagnostic(code="missing_audit_invariants", path="/first_proof_invariants", object_type="source_audit"))
        invariants = {}
    stable_ids = audit.get("stable_ids")
    if not isinstance(stable_ids, dict):
        diagnostics.append(ChunkRepairDiagnostic(code="missing_audit_stable_ids", path="/stable_ids", object_type="source_audit"))
        stable_ids = {}
    for field in ("source_ids", "locator_ids", "span_ids"):
        value = stable_ids.get(field)
        if not isinstance(value, list) or any(not _string_or_none(item) for item in value):
            diagnostics.append(ChunkRepairDiagnostic(code=f"malformed_audit_{field}", path=f"/stable_ids/{field}", object_type="source_audit"))
    count_checks = {
        "source_count": len(stable_ids.get("source_ids", [])) if isinstance(stable_ids.get("source_ids"), list) else None,
        "locator_count": len(stable_ids.get("locator_ids", [])) if isinstance(stable_ids.get("locator_ids"), list) else None,
    }
    for field, observed in count_checks.items():
        if observed is not None and invariants.get(field) != observed:
            diagnostics.append(ChunkRepairDiagnostic(code=f"audit_{field}_drift", path=f"/first_proof_invariants/{field}", object_type="source_audit"))
    for field in ("import_eligible_count", "promoted_to_fact_count", "repair_required_count", "missing_span_count", "conflicting_evidence_count"):
        if invariants.get(field) != 0:
            diagnostics.append(ChunkRepairDiagnostic(code=f"audit_{field}_nonzero", path=f"/first_proof_invariants/{field}", object_type="source_audit"))
    blockers = audit.get("safety_blockers")
    if not isinstance(blockers, dict):
        diagnostics.append(ChunkRepairDiagnostic(code="missing_audit_safety_blockers", path="/safety_blockers", object_type="source_audit"))
    else:
        for field in ("validator_diagnostics", "forbidden_payload_key_paths", "unsafe_safety_flag_paths", "summary_drift", "invariant_drift"):
            if blockers.get(field) != []:
                diagnostics.append(ChunkRepairDiagnostic(code=f"audit_{field}_present", path=f"/safety_blockers/{field}", object_type="source_audit"))
        if blockers.get("no_import_blocker_intact") is not True:
            diagnostics.append(ChunkRepairDiagnostic(code="audit_no_import_blocker_not_intact", path="/safety_blockers/no_import_blocker_intact", object_type="source_audit"))
    diagnostics.extend(_diagnostics_from_forbidden_keys(scan_forbidden_payload_keys(audit)))
    return diagnostics


def render_chunk_repair_contract_markdown(contract: dict[str, Any]) -> str:
    """Render a reviewer-facing Markdown contract without snippets or code fences."""
    invariants = contract["expected_invariants"]
    counts = contract["stable_id_counts"]
    safety = contract["safety_boundary"]
    vocab = contract["allowed_vocabularies"]
    fields = contract["target_field_requirements"]
    lines = [
        "# S02 Chunk Repair Contract",
        "",
        "This artifact translates the S01 locator audit into a reviewer-facing contract. It contains counts, stable identifiers, allowed vocabularies, field requirements, and explicit blockers only. It does not create repair claims or authorize KG writes.",
        "",
        "## Source Audit",
        "",
        f"- Contract schema: {contract['schema_version']}",
        f"- Audit schema: {contract['source_audit']['schema_version']}",
        f"- Locator input schema: {contract['source_audit']['input_schema_version']}",
        f"- Strict audit: {contract['source_audit']['strict']}",
        f"- Audit path: {contract['source_audit']['audit_path']}",
        "- Audit redacted: true",
        "",
        "## Expected Invariants",
        "",
        "| Invariant | Value |",
        "|---|---:|",
    ]
    for key, value in invariants.items():
        lines.append(f"| {key} | {value} |")
    lines.extend([
        "",
        "## Stable ID Counts",
        "",
        f"- Papers: {counts['paper_count']}",
        f"- Sources: {counts['source_count']}",
        f"- Locators: {counts['locator_count']}",
        f"- Spans: {counts['span_count']}",
        f"- Ambiguous spans: {counts['ambiguous_span_count']}",
        f"- Retrieval-only contexts: {counts['retrieval_only_count']}",
        "",
        "## Safety Boundary",
        "",
    ])
    lines.extend(f"- {_markdown_safe_field_name(key)}: {str(value).lower()}" for key, value in safety.items())
    lines.extend([
        "",
        "## Allowed Review Vocabulary",
        "",
        f"- Repair kinds: {', '.join(vocab['repair_kinds'])}",
        f"- Review statuses: {', '.join(vocab['review_statuses'])}",
        f"- Routes: {', '.join(vocab['routes'])}",
        f"- States: {', '.join(vocab['states'])}",
        f"- Coordinate spaces: {', '.join(vocab['coordinate_spaces'])}",
        "",
        "## Target Field Contract",
        "",
        f"- Required target fields: {', '.join(fields['required_target_fields'])}",
        f"- Required span fields: {', '.join(fields['required_span_fields'])}",
        f"- Required false safety fields: {', '.join(_markdown_safe_field_names(fields['required_false_safety_fields']))}",
        f"- Accepted-review reviewer fields: {', '.join(fields['accepted_reviewer_fields'])}",
        "",
        "## Reviewer Questions",
        "",
    ])
    lines.extend(f"- {requirement}" for requirement in contract["reviewer_output_requirements"])
    lines.extend(["", "## Explicit No-Go Constraints", ""])
    lines.extend(f"- {blocker}" for blocker in contract["explicit_blockers"])
    lines.append("")
    markdown = "\n".join(lines)
    markdown_diagnostics = validate_chunk_repair_contract_markdown(markdown)
    if markdown_diagnostics:
        codes = ", ".join(diagnostic.code for diagnostic in markdown_diagnostics)
        raise ValueError(f"chunk repair contract Markdown is not safe to render: {codes}")
    return markdown


def validate_chunk_repair_contract_markdown(markdown: str) -> list[ChunkRepairDiagnostic]:
    """Return redacted Markdown leakage diagnostics for the contract renderer."""
    diagnostics: list[ChunkRepairDiagnostic] = []
    for pattern in MARKDOWN_FORBIDDEN_PATTERNS:
        if pattern in markdown:
            diagnostics.append(ChunkRepairDiagnostic(code="markdown_forbidden_pattern", path="/markdown", object_id=pattern, object_type="markdown"))
    unsafe_true_markers = (
        "import_eligible: true",
        "promoted_to_fact: true",
        "trusted_kg_import_allowed: true",
        "production_write_attempted: true",
        "ladybugdb_written: true",
        "semantic_ready_for_kg: true",
        "embeddings_included: true",
        "vectors_included: true",
        "secrets_included: true",
    )
    lowered = markdown.lower()
    if any(marker in lowered for marker in unsafe_true_markers):
        diagnostics.append(ChunkRepairDiagnostic(code="markdown_true_safety_flag", path="/markdown", object_type="markdown"))
    return diagnostics


def _markdown_safe_field_names(fields: list[str]) -> list[str]:
    return [_markdown_safe_field_name(field) for field in fields]


def _markdown_safe_field_name(field: str) -> str:
    replacements = {
        "raw_text_included": "source payload included",
        "chunk_text_included": "chunk payload included",
        "embeddings_included": "model embedding payloads included",
        "vectors_included": "model vector payloads included",
        "secrets_included": "secret values included",
        "import_eligible": "import eligibility",
        "promoted_to_fact": "fact promotion",
        "trusted_kg_import_allowed": "trusted KG import allowed",
        "production_write_attempted": "production write attempted",
        "ladybugdb_written": "LadybugDB write attempted",
        "semantic_ready_for_kg": "semantic KG readiness",
        "repair_targets_created": "repair targets created",
        "review_only": "review only",
        "source_payloads_included": "source payloads included",
    }
    return replacements.get(field, field)


def expected_audit_from_contract(contract: dict[str, Any]) -> dict[str, Any]:
    """Return validator expected-audit sets from a rendered contract."""
    stable_ids = contract.get("stable_ids") if isinstance(contract.get("stable_ids"), dict) else {}
    return {
        "source_ids": stable_ids.get("source_ids", []),
        "locator_ids": stable_ids.get("locator_ids", []),
        "paper_ids": [_paper_id_from_source_id(source_id) for source_id in stable_ids.get("source_ids", [])],
    }


def scan_forbidden_payload_keys(value: Any, *, path: str = "") -> list[ForbiddenPayloadKeyFinding]:
    """Return redacted recursive forbidden-key findings for JSON-like values."""
    findings: list[ForbiddenPayloadKeyFinding] = []
    if isinstance(value, dict):
        for key in sorted(value):
            key_path = f"{path}/{_escape_path(str(key))}"
            if key in FORBIDDEN_PAYLOAD_KEYS:
                findings.append(ForbiddenPayloadKeyFinding(path=key_path, key=str(key), code=_forbidden_key_code(str(key))))
            findings.extend(scan_forbidden_payload_keys(value[key], path=key_path))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            findings.extend(scan_forbidden_payload_keys(item, path=f"{path}/{index}"))
    return findings


def _paper_id_from_source_id(source_id: Any) -> str:
    text = str(source_id)
    if text.startswith("source-") and text.endswith("-full-text"):
        return text.removeprefix("source-").removesuffix("-full-text")
    return text


def _validate_header(payload: dict[str, Any]) -> list[ChunkRepairDiagnostic]:
    diagnostics: list[ChunkRepairDiagnostic] = []
    if payload.get("schema_version") != CHUNK_REPAIR_CONTRACT_VERSION:
        diagnostics.append(ChunkRepairDiagnostic(code="schema_version_mismatch", path="/schema_version", object_type="contract"))
    if payload.get("contract_version") != CHUNK_REPAIR_CONTRACT_VERSION:
        diagnostics.append(ChunkRepairDiagnostic(code="contract_version_mismatch", path="/contract_version", object_type="contract"))
    for field in ("run_id", "paper_id", "source_ledger", "repair_targets", "diagnostics"):
        if field not in payload or payload.get(field) is None:
            diagnostics.append(ChunkRepairDiagnostic(code=f"missing_{field}", path=f"/{field}", object_type="contract"))
    return diagnostics


def _validate_target(
    target: dict[str, Any],
    *,
    path: str,
    package_paper_id: str | None,
    known_source_ids: set[str],
    expected_locator_ids: set[str] | None,
    expected_paper_ids: set[str] | None,
) -> list[ChunkRepairDiagnostic]:
    diagnostics: list[ChunkRepairDiagnostic] = []
    target_id = _string_or_none(target.get("target_id"))
    route = _string_or_none(target.get("route"))
    state = _string_or_none(target.get("state"))
    for field in REQUIRED_TARGET_FIELDS:
        if field not in target or (target.get(field) is None and field != "reviewer"):
            diagnostics.append(ChunkRepairDiagnostic(code=f"missing_{field}", path=f"{path}/{field}", object_id=target_id, object_type="repair_target", route=route))
    if target_id is None:
        diagnostics.append(ChunkRepairDiagnostic(code="missing_target_id", path=f"{path}/target_id", object_type="repair_target", route=route))
    target_paper_id = _string_or_none(target.get("paper_id"))
    paper_id_known = expected_paper_ids is not None and target_paper_id in expected_paper_ids
    if expected_paper_ids is not None and not paper_id_known:
        diagnostics.append(ChunkRepairDiagnostic(code="unresolved_paper_id", path=f"{path}/paper_id", object_id=target_id, object_type="repair_target", route=route))
    if not paper_id_known and target_paper_id != package_paper_id:
        diagnostics.append(ChunkRepairDiagnostic(code="paper_id_mismatch", path=f"{path}/paper_id", object_id=target_id, object_type="repair_target", route=route))
    locator_id = _string_or_none(target.get("locator_id"))
    if locator_id is None:
        diagnostics.append(ChunkRepairDiagnostic(code="missing_locator_id", path=f"{path}/locator_id", object_id=target_id, object_type="repair_target", route=route))
    elif expected_locator_ids is not None and locator_id not in expected_locator_ids:
        diagnostics.append(ChunkRepairDiagnostic(code="unresolved_locator_id", path=f"{path}/locator_id", object_id=target_id, object_type="repair_target", route=route))
    if target.get("repair_kind") not in ALLOWED_REPAIR_KINDS:
        diagnostics.append(ChunkRepairDiagnostic(code="invalid_repair_kind", path=f"{path}/repair_kind", object_id=target_id, object_type="repair_target", route=route))
    if target.get("candidate_type") not in ALLOWED_CANDIDATE_TYPES:
        diagnostics.append(ChunkRepairDiagnostic(code="invalid_candidate_type", path=f"{path}/candidate_type", object_id=target_id, object_type="repair_target", route=route))
    if route not in ALLOWED_ROUTES:
        diagnostics.append(ChunkRepairDiagnostic(code="invalid_repair_route", path=f"{path}/route", object_id=target_id, object_type="repair_target", route=route))
    if state not in ALLOWED_STATES:
        diagnostics.append(ChunkRepairDiagnostic(code="invalid_repair_state", path=f"{path}/state", object_id=target_id, object_type="repair_target", route=route))
    diagnostics.extend(_validate_route_state(route=route, state=state, path=path, object_id=target_id))
    if target.get("review_status") not in ALLOWED_REVIEW_STATUSES:
        diagnostics.append(ChunkRepairDiagnostic(code="invalid_review_status", path=f"{path}/review_status", object_id=target_id, object_type="repair_target", route=route))
    diagnostics.extend(_validate_string_list(target.get("section_path"), path=f"{path}/section_path", code="missing_section_path", object_id=target_id, object_type="repair_target", route=route))
    diagnostics.extend(_validate_source_refs(target.get("source_artifact_refs"), path=f"{path}/source_artifact_refs", known_source_ids=known_source_ids, object_id=target_id, route=route))
    diagnostics.extend(_validate_spans(target.get("source_spans"), path=f"{path}/source_spans", known_source_ids=known_source_ids, object_id=target_id, route=route))
    diagnostics.extend(_validate_uses(target, path=path, object_id=target_id, route=route))
    diagnostics.extend(_validate_safety_boundaries(target.get("safety_boundaries"), path=f"{path}/safety_boundaries", object_id=target_id, route=route))
    diagnostics.extend(_validate_reviewer(target, path=path, object_id=target_id, route=route))
    return diagnostics


def _validate_route_state(*, route: str | None, state: str | None, path: str, object_id: str | None) -> list[ChunkRepairDiagnostic]:
    diagnostics: list[ChunkRepairDiagnostic] = []
    if route in RETRIEVAL_ONLY_ROUTES and state not in {"retrieval_only", "review_required"}:
        diagnostics.append(ChunkRepairDiagnostic(code="route_state_confusion", path=f"{path}/state", object_id=object_id, object_type="repair_target", route=route))
    if route in REPAIR_ROUTES and state not in {"repair_required", "missing_span", "ambiguous_span", "conflicting_evidence"}:
        diagnostics.append(ChunkRepairDiagnostic(code="route_state_confusion", path=f"{path}/state", object_id=object_id, object_type="repair_target", route=route))
    return diagnostics


def _validate_string_list(value: Any, *, path: str, code: str, object_id: str | None, object_type: str, route: str | None) -> list[ChunkRepairDiagnostic]:
    if not isinstance(value, list) or not value or any(not _string_or_none(item) for item in value):
        return [ChunkRepairDiagnostic(code=code, path=path, object_id=object_id, object_type=object_type, route=route)]
    return []


def _validate_source_refs(value: Any, *, path: str, known_source_ids: set[str], object_id: str | None, route: str | None) -> list[ChunkRepairDiagnostic]:
    diagnostics = _validate_string_list(value, path=path, code="missing_source_artifact_refs", object_id=object_id, object_type="repair_target", route=route)
    if diagnostics:
        return diagnostics
    for index, source_id in enumerate(value):
        if str(source_id) not in known_source_ids:
            diagnostics.append(ChunkRepairDiagnostic(code="unresolved_source_id", path=f"{path}/{index}", object_id=object_id, object_type="repair_target", route=route))
    return diagnostics


def _validate_spans(value: Any, *, path: str, known_source_ids: set[str], object_id: str | None, route: str | None) -> list[ChunkRepairDiagnostic]:
    diagnostics: list[ChunkRepairDiagnostic] = []
    if not isinstance(value, list) or not value:
        return [ChunkRepairDiagnostic(code="missing_source_spans", path=path, object_id=object_id, object_type="repair_target", route=route)]
    for index, span in enumerate(value):
        span_path = f"{path}/{index}"
        if not isinstance(span, dict):
            diagnostics.append(ChunkRepairDiagnostic(code="invalid_source_span", path=span_path, object_id=object_id, object_type="source_span", route=route))
            continue
        span_id = _string_or_none(span.get("span_id"))
        for field in REQUIRED_SPAN_FIELDS:
            if field not in span or span.get(field) is None:
                diagnostics.append(ChunkRepairDiagnostic(code=f"missing_{field}", path=f"{span_path}/{field}", object_id=span_id or object_id, object_type="source_span", route=route))
        if span.get("coordinate_space") not in ALLOWED_COORDINATE_SPACES:
            diagnostics.append(ChunkRepairDiagnostic(code="unsupported_coordinate_space", path=f"{span_path}/coordinate_space", object_id=span_id or object_id, object_type="source_span", route=route))
        if not isinstance(span.get("char_start"), int) or not isinstance(span.get("char_end"), int) or span.get("char_end", 0) <= span.get("char_start", 0):
            diagnostics.append(ChunkRepairDiagnostic(code="invalid_coordinate_bounds", path=f"{span_path}/char_start", object_id=span_id or object_id, object_type="source_span", route=route))
        if _string_or_none(span.get("source_id")) not in known_source_ids:
            diagnostics.append(ChunkRepairDiagnostic(code="unresolved_source_id", path=f"{span_path}/source_id", object_id=span_id or object_id, object_type="source_span", route=route))
        span_hash = _string_or_none(span.get("span_hash"))
        if span_hash is None:
            diagnostics.append(ChunkRepairDiagnostic(code="missing_span_hash", path=f"{span_path}/span_hash", object_id=span_id or object_id, object_type="source_span", route=route))
        elif not SAFE_HEX_SHA256.match(span_hash):
            diagnostics.append(ChunkRepairDiagnostic(code="invalid_span_hash", path=f"{span_path}/span_hash", object_id=span_id or object_id, object_type="source_span", route=route))
        if span.get("raw_text_embedded") is not False:
            diagnostics.append(ChunkRepairDiagnostic(code="raw_text_leakage", path=f"{span_path}/raw_text_embedded", object_id=span_id or object_id, object_type="source_span", route=route))
    return diagnostics


def _validate_uses(target: dict[str, Any], *, path: str, object_id: str | None, route: str | None) -> list[ChunkRepairDiagnostic]:
    diagnostics: list[ChunkRepairDiagnostic] = []
    allowed = _string_set(target.get("allowed_uses"))
    excluded = _string_set(target.get("excluded_uses"))
    if allowed != set(ALLOWED_USES):
        diagnostics.append(ChunkRepairDiagnostic(code="allowed_uses_mismatch", path=f"{path}/allowed_uses", object_id=object_id, object_type="repair_target", route=route))
    missing_excluded = set(EXCLUDED_USES) - excluded
    if missing_excluded:
        diagnostics.append(ChunkRepairDiagnostic(code="excluded_uses_mismatch", path=f"{path}/excluded_uses", object_id=object_id, object_type="repair_target", route=route))
    if "trusted_kg_import" in allowed:
        diagnostics.append(ChunkRepairDiagnostic(code="trusted_kg_import_allowed", path=f"{path}/allowed_uses", object_id=object_id, object_type="repair_target", route=route))
    return diagnostics


def _validate_safety_boundaries(value: Any, *, path: str, object_id: str | None, route: str | None) -> list[ChunkRepairDiagnostic]:
    diagnostics: list[ChunkRepairDiagnostic] = []
    if not isinstance(value, dict):
        return [ChunkRepairDiagnostic(code="missing_safety_boundaries", path=path, object_id=object_id, object_type="repair_target", route=route)]
    true_code_by_field = {
        "import_eligible": "import_eligible_true",
        "promoted_to_fact": "promoted_to_fact_true",
        "trusted_kg_import_allowed": "trusted_kg_import_allowed_true",
        "production_write_attempted": "production_write_attempted",
        "ladybugdb_written": "ladybugdb_written",
        "semantic_ready_for_kg": "semantic_ready_for_kg_true",
        "raw_text_included": "raw_text_leakage",
        "chunk_text_included": "raw_text_leakage",
        "embeddings_included": "embedding_leakage",
        "vectors_included": "vector_leakage",
        "secrets_included": "secret_leakage",
    }
    for field in REQUIRED_FALSE_SAFETY_FIELDS:
        if field not in value:
            diagnostics.append(ChunkRepairDiagnostic(code=f"missing_{field}", path=f"{path}/{field}", object_id=object_id, object_type="repair_target", route=route))
        elif value.get(field) is not False:
            diagnostics.append(ChunkRepairDiagnostic(code=true_code_by_field[field], path=f"{path}/{field}", object_id=object_id, object_type="repair_target", route=route))
    return diagnostics


def _validate_reviewer(target: dict[str, Any], *, path: str, object_id: str | None, route: str | None) -> list[ChunkRepairDiagnostic]:
    if target.get("review_status") != "accepted":
        return []
    reviewer = target.get("reviewer")
    if not isinstance(reviewer, dict):
        return [ChunkRepairDiagnostic(code="missing_reviewer", path=f"{path}/reviewer", object_id=object_id, object_type="repair_target", route=route)]
    diagnostics: list[ChunkRepairDiagnostic] = []
    for field in ACCEPTED_REVIEWER_FIELDS:
        if field not in reviewer or reviewer.get(field) in (None, "", []):
            diagnostics.append(ChunkRepairDiagnostic(code=f"missing_{field}", path=f"{path}/reviewer/{field}", object_id=object_id, object_type="reviewer", route=route))
    return diagnostics


def _validate_contract_diagnostics(
    value: Any,
    *,
    target_count: int,
    pending_review_count: int,
    accepted_count: int,
    import_eligible_count: int,
    promoted_to_fact_count: int,
    production_write_count: int,
    semantic_ready_count: int,
) -> list[ChunkRepairDiagnostic]:
    diagnostics: list[ChunkRepairDiagnostic] = []
    if not isinstance(value, dict):
        return [ChunkRepairDiagnostic(code="missing_diagnostics", path="/diagnostics", object_type="diagnostics")]
    for field in REQUIRED_DIAGNOSTIC_FIELDS:
        if field not in value:
            diagnostics.append(ChunkRepairDiagnostic(code=f"missing_{field}", path=f"/diagnostics/{field}", object_type="diagnostics"))
    expected_counts = {
        "target_count": target_count,
        "pending_review_count": pending_review_count,
        "accepted_count": accepted_count,
        "import_eligible_count": import_eligible_count,
        "promoted_to_fact_count": promoted_to_fact_count,
        "production_write_count": production_write_count,
        "semantic_ready_count": semantic_ready_count,
    }
    mismatch_codes = {
        "target_count": "diagnostics_target_count_mismatch",
        "pending_review_count": "diagnostics_pending_count_mismatch",
        "accepted_count": "diagnostics_accepted_count_mismatch",
        "import_eligible_count": "diagnostics_import_count_mismatch",
        "promoted_to_fact_count": "diagnostics_fact_count_mismatch",
        "production_write_count": "diagnostics_write_count_mismatch",
        "semantic_ready_count": "diagnostics_semantic_ready_count_mismatch",
    }
    for field, expected in expected_counts.items():
        if value.get(field) != expected:
            diagnostics.append(ChunkRepairDiagnostic(code=mismatch_codes[field], path=f"/diagnostics/{field}", object_type="diagnostics"))
    for field in (
        "raw_text_included",
        "chunk_text_included",
        "embeddings_included",
        "vectors_included",
        "secrets_included",
        "ladybugdb_written",
        "production_import_attempted",
    ):
        if value.get(field) is not False:
            code = "raw_text_leakage" if field in {"raw_text_included", "chunk_text_included"} else f"{field}_true"
            diagnostics.append(ChunkRepairDiagnostic(code=code, path=f"/diagnostics/{field}", object_type="diagnostics"))
    return diagnostics


def _diagnostics_from_forbidden_keys(findings: list[ForbiddenPayloadKeyFinding]) -> list[ChunkRepairDiagnostic]:
    return [ChunkRepairDiagnostic(code=finding.code, path=finding.path, object_type="payload") for finding in findings]


def _forbidden_key_code(key: str) -> str:
    if key in {"text", "raw_text", "chunk_text", "paper_text", "claim_text"}:
        return "raw_text_leakage"
    if key in {"embedding", "embeddings"}:
        return "embedding_leakage"
    if key in {"vector", "vectors"}:
        return "vector_leakage"
    if key in {"secret", "secrets", "token", "tokens", "api_key", "credentials"}:
        return "secret_leakage"
    if key in {"optimizer_trace", "optimizer_traces"}:
        return "optimizer_trace_leakage"
    return "forbidden_payload_key"


def _expected_sets(expected_audit: dict[str, Any] | None) -> dict[str, Any]:
    diagnostics: list[ChunkRepairDiagnostic] = []
    result: dict[str, Any] = {"locator_ids": None, "source_ids": None, "paper_ids": None, "diagnostics": diagnostics}
    if expected_audit is None:
        return result
    for field in ("locator_ids", "source_ids", "paper_ids"):
        value = expected_audit.get(field)
        if value is None:
            continue
        if not isinstance(value, (list, set, tuple)):
            diagnostics.append(ChunkRepairDiagnostic(code=f"malformed_expected_{field}", path=f"/expected_audit/{field}", object_type="expected_audit"))
            result[field] = set()
        else:
            result[field] = {str(item) for item in value if item is not None}
    return result


def _source_ids(value: Any) -> set[str]:
    if not isinstance(value, list):
        return set()
    return {_string_or_none(item.get("source_id")) for item in value if isinstance(item, dict) and _string_or_none(item.get("source_id"))}  # type: ignore[misc]


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _string_set(value: Any) -> set[str]:
    if not isinstance(value, list):
        return set()
    return {str(item) for item in value if item is not None}


def _string_or_none(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value)
    return text if text else None


def _escape_path(value: str) -> str:
    return value.replace("~", "~0").replace("/", "~1")
