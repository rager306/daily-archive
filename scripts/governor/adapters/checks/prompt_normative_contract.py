"""Independent oracle for the prompt normative register DATA rule (M510/S02/T03).

The prompt/meta-prompt normatives introduced by M510-jsg1pj/S02/T02 live in
`data/prompts/normatives.yaml`: every obligation carries an identifier, its
scope, the authority it derives from, its preconditions, an evidence obligation
and a verification operation expressed as an ALLOWLISTED typed identifier with
typed parameters. The register also declares the boundaries an optimizer may not
rewrite and marks descriptive prose so it cannot be read as enforced.

This module is the CONTROL-PLANE half of that contract. It is an INDEPENDENT
oracle (ADR-051): it shares no code with the Rust `NormativeRegister`
(`crates/da-domain/src/agent_manifest.rs`) and does not import it. It re-derives
the failure modes from the document a register declares about itself, which is
what makes it falsifiable against the DATA fixtures rather than a restatement of
the Rust validator.

Semantics
---------
Authority is bound to content, not to a label (D418): an obligation that is
unidentified, weakened (`strength` outside the contract's `must_strength`),
renamed into a duplicate, stripped of its authority/preconditions/evidence, or
checked by a non-allowlisted operation does not degrade into a recommendation --
the register containing it fails closed and every obligation in it loses
authority. Concretely the oracle emits a typed token for each failure class:

  * register level: ``unsupported_schema_version``, ``missing_register_id``,
    ``missing_import_lock`` / ``import_eligibility_leak`` (D127: the register
    never grants import eligibility), ``missing_boundaries`` /
    ``incomplete_immutable_boundary_set`` / ``incomplete_forbidden_rewrites``;
  * authority vocabulary: ``missing_authority_sources``,
    ``duplicate_authority_source``, ``unknown_authority_source_kind``;
  * verification vocabulary: ``missing_verification_ops``,
    ``duplicate_verification_op``, ``unknown_parameter_type``;
  * per obligation: ``missing_obligation_id``, ``duplicate_normative_id``,
    ``missing_scope``, ``missing_strength``, ``weak_obligation``,
    ``missing_authority`` / ``unknown_authority``, ``missing_preconditions``,
    ``missing_evidence_obligation``, ``missing_verification`` /
    ``unknown_verification_op``, ``missing_parameter`` / ``untyped_parameter`` /
    ``type_mismatch`` / ``invalid_path_parameter`` / ``invalid_digest_parameter``
    / ``enum_value_not_allowed``, ``missing_immutable_boundary``;
  * descriptive material: ``descriptive_marked_enforced`` (not marked
    ``enforced: false``), ``descriptive_authority_leak``,
    ``descriptive_carries_verification``, ``descriptive_carries_obligation``,
    ``descriptive_immutable_boundary``, ``missing_descriptive_text``.

Verified operations are typed identifiers, never shell strings: a `verification`
item whose `op` is not declared in the register's own ``verification_ops``
allowlist is ``unknown_verification_op``; a parameter the op schema does not
declare is ``untyped_parameter``; a declared parameter that is absent is
``missing_parameter``; and a value that does not match its declared type is
``type_mismatch`` (with dedicated ``invalid_path_parameter`` /
``invalid_digest_parameter`` / ``enum_value_not_allowed`` labels). A raw
``shell:`` field is not part of the schema, so it cannot smuggle an executable
string past the allowlist.

Boundary (stated rather than implied): this is a consistency and fail-closed
contract over the register document, not a tamper-evident attestation. An actor
able to rewrite both the register and this DATA rule can satisfy it. Its value
is that absence, weakening, renaming and self-contradiction stop passing as
compliance, and that no obligation can be "verified" by a shell string.

The module is pure: no filesystem, network, clock or RNG access. Executing the
allowlisted typed operations against the real tree is the caller's job.
"""

from __future__ import annotations

from typing import Any

__all__ = [
    "missing_normative_vocabulary",
    "prompt_normative_violations",
]

#: The register schema this oracle understands.
_SUPPORTED_REGISTER_SCHEMA = "prompt-normatives.v1"

#: Authority source kinds the register recognises (mirror of the Rust const).
_AUTHORITY_SOURCE_KINDS = frozenset(
    {"requirement", "constraint", "operability", "adr", "decision", "prompt_contract"}
)

#: Typed parameter types the register recognises (mirror of the Rust const).
_PARAM_TYPES = frozenset({"string", "int", "bool", "path", "digest", "enum"})

#: Obligation fields a descriptive entry must not carry.
_DESCRIPTIVE_FORBIDDEN_FIELDS = (
    "strength",
    "evidence_obligation",
    "preconditions",
)

#: Contract keys the oracle cannot work without. Without them it cannot tell a
#: weakened obligation from an enforced one and would accept every register
#: (fail open), so a missing/empty value is reported to the caller.
_REQUIRED_REGISTER_KEYS = (
    "path",
    "schema_version",
    "allowed_strengths",
    "must_strength",
    "required_obligations",
    "required_optimizer_immutable",
    "d127_key",
)
_REQUIRED_MARKER_KEYS = (
    "prompt_contract_prefix",
    "optimizer_immutable_marker",
    "prompt_marker_paths",
)

def _string_set(value: Any) -> set[str]:
    """The non-empty strings inside a collection, or an empty set."""
    if not isinstance(value, (list, tuple, set)):
        return set()
    return {item for item in value if isinstance(item, str) and item}

def missing_normative_vocabulary(contract: Any) -> list[str]:
    """Names of required contract keys that are absent or empty.

    A contract with no register schema, no `must_strength` or no required
    obligation/immutable sets cannot distinguish an enforced obligation from a
    recommendation, so the rule would accept every register. Returning the names
    lets the check flag the contract instead of failing open.
    """
    missing: list[str] = []
    register = (contract or {}).get("register")
    if not isinstance(register, dict):
        missing.append("register")
    else:
        for key in _REQUIRED_REGISTER_KEYS:
            value = register.get(key)
            if key in ("allowed_strengths", "required_obligations",
                       "required_optimizer_immutable"):
                if not _string_set(value):
                    missing.append(f"register.{key}")
            elif not isinstance(value, str) or not value:
                missing.append(f"register.{key}")
        allowed = _string_set(register.get("allowed_strengths"))
        must = register.get("must_strength")
        if isinstance(must, str) and must and allowed and must not in allowed:
            missing.append("register.must_strength")
    markers = (contract or {}).get("markers")
    if not isinstance(markers, dict):
        missing.append("markers")
    else:
        for key in _REQUIRED_MARKER_KEYS:
            value = markers.get(key)
            if key == "prompt_marker_paths":
                if not _string_set(value):
                    missing.append(f"markers.{key}")
            elif not isinstance(value, str) or not value:
                missing.append(f"markers.{key}")
    fixtures = (contract or {}).get("fixtures")
    if not isinstance(fixtures, dict) or not isinstance(fixtures.get("positive"), dict) \
            or not isinstance(fixtures.get("negative"), dict):
        missing.append("fixtures")
    return missing

def _is_repo_relative_path(value: str) -> bool:
    """True for a repo-relative path that cannot escape the repository."""
    if not value or "\0" in value or "://" in value:
        return False
    if value.startswith("/"):
        return False
    return ".." not in value.replace("\\", "/").split("/")

def _is_sha256_hex(value: str) -> bool:
    return len(value) == 64 and all(c in "0123456789abcdef" for c in value)

def _param_type_error(spec: Any, value: Any) -> str | None:
    """The typed violation code for a parameter value, or None when it matches."""
    if not isinstance(spec, dict):
        return "unknown_parameter_type"
    param_type = spec.get("type")
    if param_type not in _PARAM_TYPES:
        return "unknown_parameter_type"
    if param_type == "string":
        return None if isinstance(value, str) else "type_mismatch"
    if param_type == "int":
        return None if isinstance(value, int) and not isinstance(value, bool) else "type_mismatch"
    if param_type == "bool":
        return None if isinstance(value, bool) else "type_mismatch"
    if param_type == "path":
        if not isinstance(value, str):
            return "type_mismatch"
        return None if _is_repo_relative_path(value) else "invalid_path_parameter"
    if param_type == "digest":
        if not isinstance(value, str):
            return "type_mismatch"
        return None if _is_sha256_hex(value) else "invalid_digest_parameter"
    # enum
    if not isinstance(value, str):
        return "type_mismatch"
    allowed = _string_set(spec.get("values"))
    return None if value in allowed else "enum_value_not_allowed"

def _validate_obligation(
    entry: dict,
    violations: set[str],
    op_specs: dict[str, dict],
    source_ids: set[str],
    required_immutable: set[str],
    must_strength: str,
    allowed_strengths: set[str],
) -> None:
    """Validate one `kind: obligation` entry (fail-closed, content-bound)."""
    if entry.get("enforced") is not None:
        violations.add("obligation_marked_descriptive")

    strength = entry.get("strength")
    if strength is None:
        violations.add("missing_strength")
    elif strength not in allowed_strengths:
        violations.add("unknown_strength")
    elif strength != must_strength:
        # Weakening `must` to `should`/`may` does not downgrade the obligation:
        # it destroys the authority the register exists to carry.
        violations.add("weak_obligation")

    scope = entry.get("scope")
    if not isinstance(scope, str) or not scope.strip():
        violations.add("missing_scope")

    authority = entry.get("authority")
    if not isinstance(authority, list) or not authority:
        violations.add("missing_authority")
    else:
        for source in authority:
            if not isinstance(source, str) or not source:
                violations.add("invalid_authority")
            elif source not in source_ids:
                violations.add("unknown_authority")

    preconditions = entry.get("preconditions")
    if not isinstance(preconditions, list) or not all(
        isinstance(p, str) and p.strip() for p in preconditions
    ):
        violations.add("missing_preconditions")

    evidence = entry.get("evidence_obligation")
    if not isinstance(evidence, str) or not evidence.strip():
        violations.add("missing_evidence_obligation")

    verifications = entry.get("verification")
    if not isinstance(verifications, list) or not verifications:
        violations.add("missing_verification")
    else:
        for verification in verifications:
            if not isinstance(verification, dict):
                violations.add("invalid_verification")
                continue
            op = verification.get("op")
            spec = op_specs.get(op) if isinstance(op, str) else None
            if spec is None:
                violations.add("unknown_verification_op")
                continue
            params = verification.get("params")
            if not isinstance(params, dict):
                violations.add("missing_parameters")
                continue
            declared = spec
            for name, declared_spec in declared.items():
                if name not in params:
                    violations.add("missing_parameter")
                    continue
                error = _param_type_error(declared_spec, params[name])
                if error:
                    violations.add(error)
            for name in params:
                if name not in declared:
                    violations.add("untyped_parameter")

    immutable = entry.get("optimizer_immutable")
    if not isinstance(immutable, list) or not immutable:
        violations.add("missing_immutable_boundary")
    elif not required_immutable.issubset(_string_set(immutable)):
        violations.add("missing_immutable_boundary")

def _validate_descriptive(entry: dict, violations: set[str]) -> None:
    """Validate one `kind: descriptive` entry (never an enforced obligation)."""
    if entry.get("enforced") is not False:
        violations.add("descriptive_marked_enforced")
    if entry.get("authority"):
        violations.add("descriptive_authority_leak")
    if entry.get("verification"):
        violations.add("descriptive_carries_verification")
    if any(field in entry for field in _DESCRIPTIVE_FORBIDDEN_FIELDS):
        violations.add("descriptive_carries_obligation")
    if entry.get("optimizer_immutable"):
        violations.add("descriptive_immutable_boundary")
    text = entry.get("text")
    if not isinstance(text, str) or not text.strip():
        violations.add("missing_descriptive_text")

def prompt_normative_violations(record: Any, contract: Any) -> set[str]:
    """Violations of one prompt normative register against the contract.

    Matches the shared fixture-oracle signature ``violations_fn(record,
    contract)``. An empty set means the register identifies every obligation,
    binds authority to registered sources, checks with allowlisted typed
    operations only, preserves the optimizer-immutable boundaries and keeps
    descriptive material explicitly non-enforced.
    """
    register_contract = (contract or {}).get("register") or {}
    allowed_strengths = _string_set(register_contract.get("allowed_strengths"))
    must_strength = str(register_contract.get("must_strength") or "must")
    required_immutable = _string_set(register_contract.get("required_optimizer_immutable"))
    d127_key = str(register_contract.get("d127_key") or "d127_import_eligible")
    expected_schema = str(register_contract.get("schema_version") or _SUPPORTED_REGISTER_SCHEMA)

    if not isinstance(record, dict):
        return {"missing_register_object"}

    violations: set[str] = set()

    if record.get("schema_version") != expected_schema:
        violations.add("unsupported_schema_version")
    register_id = record.get("register_id")
    if not isinstance(register_id, str) or not register_id.strip():
        violations.add("missing_register_id")

    if d127_key not in record or record[d127_key] is None:
        violations.add("missing_import_lock")
    elif record[d127_key] is not False:
        violations.add("import_eligibility_leak")

    boundaries = record.get("boundaries")
    if not isinstance(boundaries, dict):
        violations.add("missing_boundaries")
    else:
        if not required_immutable.issubset(
            _string_set(boundaries.get("required_optimizer_immutable"))
        ):
            violations.add("incomplete_immutable_boundary_set")
        required_forbidden = _string_set(
            register_contract.get("required_forbidden_rewrites")
        ) or {"obligation_id", "authority", "evidence_obligation"}
        if not required_forbidden.issubset(
            _string_set(boundaries.get("forbidden_rewrites"))
        ):
            violations.add("incomplete_forbidden_rewrites")

    sources = record.get("authority_sources")
    source_ids: set[str] = set()
    if not isinstance(sources, list) or not sources:
        violations.add("missing_authority_sources")
    else:
        for source in sources:
            if not isinstance(source, dict):
                violations.add("invalid_authority_source")
                continue
            source_id = source.get("id")
            if not isinstance(source_id, str) or not source_id:
                violations.add("missing_authority_source_id")
                continue
            if source_id in source_ids:
                violations.add("duplicate_authority_source")
            source_ids.add(source_id)
            if source.get("kind") not in _AUTHORITY_SOURCE_KINDS:
                violations.add("unknown_authority_source_kind")

    ops = record.get("verification_ops")
    op_specs: dict[str, dict] = {}
    if not isinstance(ops, list) or not ops:
        violations.add("missing_verification_ops")
    else:
        for op in ops:
            if not isinstance(op, dict):
                violations.add("invalid_verification_op")
                continue
            op_id = op.get("id")
            if not isinstance(op_id, str) or not op_id:
                violations.add("invalid_verification_op")
                continue
            if op_id in op_specs:
                violations.add("duplicate_verification_op")
            params = op.get("params")
            if not isinstance(params, dict) or not params:
                violations.add("verification_op_without_params")
                op_specs[op_id] = {}
                continue
            for name, spec in params.items():
                if not isinstance(spec, dict) or spec.get("type") not in _PARAM_TYPES:
                    violations.add("unknown_parameter_type")
            op_specs[op_id] = params

    normatives = record.get("normatives")
    if not isinstance(normatives, list) or not normatives:
        violations.add("missing_normatives")
        return violations

    seen_ids: set[str] = set()
    for entry in normatives:
        if not isinstance(entry, dict):
            violations.add("invalid_normative_entry")
            continue
        entry_id = entry.get("id")
        if not isinstance(entry_id, str) or not entry_id:
            violations.add("missing_obligation_id")
        elif entry_id in seen_ids:
            violations.add("duplicate_normative_id")
        else:
            seen_ids.add(entry_id)

        kind = entry.get("kind")
        if kind == "obligation":
            _validate_obligation(
                entry, violations, op_specs, source_ids,
                required_immutable, must_strength, allowed_strengths,
            )
        elif kind == "descriptive":
            _validate_descriptive(entry, violations)
        else:
            violations.add("unknown_normative_kind")

    return violations
