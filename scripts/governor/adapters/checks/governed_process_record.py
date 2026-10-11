"""governed_process_record contract check — pure predicates (Governor v2).

The S10 slice links the policy FSM, the prompt registry and the runtime
admission into one governed process: one driven request produces one typed
``GovernedProcessRecord`` (``crates/da-domain/src/governed_process_record.rs``)
binding the DATA route decision, the per-stage admission codes and the
registry-bound prompt lineage. This module holds the *pure* predicates the
governor rule runs against a committed record, so the same logic can be
asserted by the Rust half of the shared oracle
(``crates/da-adapters/tests/governed_process_record_oracle.rs``) without the
governor machinery.

Every predicate is fail-closed and returns a set of violation tokens from a
closed vocabulary (``consult_violation``, ``lineage_violation``,
``admission_violation``, ``lock_leak``). The governor maps each token to its
own named flag; a record that triggers no token is clean.

The live DATA is the single truth: the allowed/forbidden decision points are
read from ``data/policies/registry.yaml`` ``consult_status`` (never duplicated
in the check YAML), and the prompt lineage is resolved against
``data/ontology/prompt_registry.yaml`` + ``data/ontology/prompt_tasks.yaml`` by
recomputing the fingerprints from source the same way the da-adapters loader
does (D418 recompute-from-source).
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Callable, Iterable, Mapping

import yaml

__all__ = [
    "ADMISSION_VIOLATION",
    "CONSULT_VIOLATION",
    "LINEAGE_VIOLATION",
    "LOCK_LEAK",
    "VIOLATION_TOKENS",
    "consult_violations",
    "governed_process_record_violations",
    "load_lineage_resolver",
    "resolve_consult_sets",
]

#: Violation vocabulary. Each token maps to its own governor flag family.
CONSULT_VIOLATION = "consult_violation"
LINEAGE_VIOLATION = "lineage_violation"
ADMISSION_VIOLATION = "admission_violation"
LOCK_LEAK = "lock_leak"

#: The closed token set the governor and the Rust oracle both speak.
VIOLATION_TOKENS: tuple[str, ...] = (
    CONSULT_VIOLATION,
    LINEAGE_VIOLATION,
    ADMISSION_VIOLATION,
    LOCK_LEAK,
)


def _is_lowercase_hex64(value: object) -> bool:
    """Exactly 64 lowercase hexadecimal characters (ContentDigest contract)."""
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(char in "0123456789abcdef" for char in value)
    )


def resolve_consult_sets(consult_status: Mapping[str, object], allowed_key: str) -> tuple[set[str], set[str]]:
    """Split a live ``consult_status`` block into (allowed, forbidden).

    ``allowed`` is the declared list under ``allowed_key`` (the decision points
    the parse-pdf FSM may consult). ``forbidden`` is the union of every *other*
    list in the block (the later / unregistered / native-rule-only names), so
    the check never duplicates the set in its own YAML.
    """
    allowed_raw = consult_status.get(allowed_key)
    if not isinstance(allowed_raw, list) or not allowed_raw:
        raise ValueError(
            f"consult_status.{allowed_key} must be a non-empty list of decision points"
        )
    allowed = {str(name) for name in allowed_raw}
    forbidden: set[str] = set()
    for key, value in consult_status.items():
        if key == allowed_key or not isinstance(value, list):
            continue
        forbidden.update(str(name) for name in value)
    return allowed, forbidden


def consult_violations(
    consulted: Iterable[str], allowed: Iterable[str], forbidden: Iterable[str]
) -> set[str]:
    """Pure consult predicate.

    A record violates the consult contract when it consults *no* decision point
    at all, when any consulted point is outside the declared allowed set, or
    when a later / unregistered / forbidden name is consulted.
    """
    consulted_set = {str(name) for name in consulted}
    allowed_set = {str(name) for name in allowed}
    forbidden_set = {str(name) for name in forbidden}
    if (
        not consulted_set
        or not consulted_set <= allowed_set
        or bool(consulted_set & forbidden_set)
    ):
        return {CONSULT_VIOLATION}
    return set()


def _consulted_points(record: Mapping[str, object]) -> set[str]:
    decisions = record.get("policy_decisions")
    if not isinstance(decisions, list):
        return set()
    return {
        str(decision.get("decision_point", ""))
        for decision in decisions
        if isinstance(decision, dict)
    }


def governed_process_record_violations(
    record: object,
    *,
    allowed_consults: Iterable[str],
    forbidden_consults: Iterable[str],
    closed_codes: Iterable[str],
    forbidden_locks: Iterable[str],
    resolve_lineage: Callable[[str, str], dict | None],
) -> set[str]:
    """Every violation a governed-process record triggers, as a token set.

    Fail-closed: a non-mapping record is not a record and triggers the whole
    vocabulary, so a malformed fixture can never read as clean.
    """
    if not isinstance(record, dict):
        return set(VIOLATION_TOKENS)

    closed = {str(code) for code in closed_codes}
    locks = {str(name) for name in forbidden_locks}
    violations: set[str] = set()

    consulted = _consulted_points(record)
    violations |= consult_violations(consulted, allowed_consults, forbidden_consults)

    # D127 / ADR-055: the candidate-only lock stays closed, and no lock name is
    # ever consulted as a decision point.
    if record.get("import_eligible") is not False:
        violations.add(LOCK_LEAK)
    if consulted & locks:
        violations.add(LOCK_LEAK)

    admissions = record.get("admissions")
    if isinstance(admissions, list):
        for admission in admissions:
            code = admission.get("code") if isinstance(admission, dict) else None
            if code not in closed:
                violations.add(ADMISSION_VIOLATION)
    else:
        violations.add(ADMISSION_VIOLATION)

    lineages = record.get("prompt_lineages")
    if isinstance(lineages, list):
        for lineage in lineages:
            if not isinstance(lineage, dict):
                violations.add(LINEAGE_VIOLATION)
                continue
            prompt_id = lineage.get("prompt_id")
            prompt_version = lineage.get("prompt_version")
            resolved = (
                resolve_lineage(str(prompt_id), str(prompt_version))
                if isinstance(prompt_id, str) and isinstance(prompt_version, str)
                else None
            )
            if resolved is None:
                violations.add(LINEAGE_VIOLATION)
                continue
            if (
                lineage.get("prompt_sha256") != resolved.get("prompt_sha256")
                or lineage.get("schema_sha256") != resolved.get("schema_sha256")
            ):
                violations.add(LINEAGE_VIOLATION)
            if not _is_lowercase_hex64(lineage.get("prompt_sha256")) or not _is_lowercase_hex64(
                lineage.get("schema_sha256")
            ):
                violations.add(LINEAGE_VIOLATION)
    else:
        violations.add(LINEAGE_VIOLATION)

    return violations


def _task_template_sha256(task: Mapping[str, object]) -> str:
    """Mirror ``PromptTaskSpec::template_sha256`` (serde_json tuple of the pair)."""
    pair = [task.get("system_template", ""), task.get("user_template", "")]
    payload = json.dumps(pair, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def _schema_sha256(schema: object) -> str:
    """Mirror ``PromptOutputSchema::sha256`` (serde_json Value, sorted keys)."""
    payload = json.dumps(
        schema, separators=(",", ":"), ensure_ascii=False, sort_keys=True
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def load_lineage_resolver(
    root: Path, registry_rel: str, task_registry_rel: str
) -> Callable[[str, str], dict | None]:
    """Resolve ``(prompt_id, prompt_version)`` to live registry fingerprints.

    Mirrors ``da_adapters::prompt_registry::load_prompt_registry`` for the two
    kinds the registry declares: a ``task`` entry resolves in
    ``data/ontology/prompt_tasks.yaml`` and fingerprints the template pair plus
    the output schema; a ``meta_prompt`` entry fingerprints the referenced
    source and schema bytes. A missing file or an unresolvable entry returns
    ``None`` so the caller fails closed.
    """
    try:
        registry_doc = yaml.safe_load(
            (root / registry_rel).read_text(encoding="utf-8")
        ) or {}
        tasks_doc = yaml.safe_load(
            (root / task_registry_rel).read_text(encoding="utf-8")
        ) or {}
    except (OSError, yaml.YAMLError):
        return lambda _prompt_id, _prompt_version: None

    tasks = {
        str(task.get("task_id")): task
        for task in (tasks_doc.get("tasks") or [])
        if isinstance(task, dict) and task.get("task_id")
    }
    schemas = {
        str(schema.get("output_schema_id")): schema
        for schema in (tasks_doc.get("output_schemas") or [])
        if isinstance(schema, dict) and schema.get("output_schema_id")
    }
    entries = {
        (str(entry.get("prompt_id")), str(entry.get("prompt_version"))): entry
        for entry in (registry_doc.get("entries") or [])
        if isinstance(entry, dict) and entry.get("prompt_id") and entry.get("prompt_version")
    }

    def resolve(prompt_id: str, prompt_version: str) -> dict | None:
        entry = entries.get((prompt_id, prompt_version))
        if entry is None:
            return None
        kind = entry.get("kind")
        if kind == "task":
            task = tasks.get(prompt_id)
            if task is None or str(task.get("version")) != prompt_version:
                return None
            schema = schemas.get(str(task.get("output_schema_id")))
            if schema is None:
                return None
            return {
                "prompt_sha256": _task_template_sha256(task),
                "schema_sha256": _schema_sha256(schema.get("schema")),
            }
        if kind == "meta_prompt":
            source_ref = entry.get("source_ref")
            schema_ref = entry.get("schema_ref")
            if not isinstance(source_ref, str) or not isinstance(schema_ref, str):
                return None
            try:
                prompt_bytes = (root / source_ref).read_bytes()
                schema_bytes = (root / schema_ref).read_bytes()
            except OSError:
                return None
            return {
                "prompt_sha256": hashlib.sha256(prompt_bytes).hexdigest(),
                "schema_sha256": hashlib.sha256(schema_bytes).hexdigest(),
            }
        return None

    return resolve
