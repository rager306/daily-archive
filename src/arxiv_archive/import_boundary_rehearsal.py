"""Negative isolated import boundary rehearsal contract for M005/S07.

This module models a dry-run import boundary that proves current package-like
candidates are rejected before trusted KG writes. It serializes refusal and
remediation diagnostics only; no raw text, embeddings, vectors, or production
write state are allowed.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m005-negative-import-boundary-rehearsal.v1"
TRUSTED_IMPORT_USE = "trusted_kg_import"

FORBIDDEN_RAW_FIELDS = frozenset({"text", "raw_text", "chunk_text", "paper_text", "claim_text"})
FORBIDDEN_EMBEDDING_FIELDS = frozenset({"embedding", "embeddings"})
FORBIDDEN_VECTOR_FIELDS = frozenset({"vector", "vectors"})
FORBIDDEN_SECRET_FIELDS = frozenset({"secret", "secrets", "token", "tokens", "api_key", "credentials"})
FORBIDDEN_OPTIMIZER_FIELDS = frozenset({"optimizer_trace", "optimizer_traces"})


def _safety_flags() -> dict[str, bool]:
    return {
        "raw_text_included": False,
        "chunk_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "optimizer_traces_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


@dataclass(frozen=True)
class RehearsalDiagnostic:
    """One redacted validation diagnostic for import-boundary rehearsal evidence."""

    reason: str
    object_id: str | None = None
    object_type: str | None = None
    blocks_import: bool = True


@dataclass(frozen=True)
class RehearsalValidationResult:
    """Validation result for one negative import rehearsal artifact."""

    valid_rehearsal: bool
    diagnostics: tuple[RehearsalDiagnostic, ...]

    @property
    def passed(self) -> bool:
        return self.valid_rehearsal and not self.diagnostics

    @property
    def refusal_counts(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        for diagnostic in self.diagnostics:
            counts[diagnostic.reason] = counts.get(diagnostic.reason, 0) + 1
        return dict(sorted(counts.items()))


@dataclass(frozen=True)
class ImportCandidate:
    """A redacted import candidate identity and refusal context."""

    candidate_id: str
    method_id: str
    package_id: str
    candidate_type: str
    route: str | None = None
    state: str | None = None
    import_eligible: bool = False
    refusal_reasons: tuple[str, ...] = ()
    remediation_hints: tuple[str, ...] = ()

    def to_contract(self) -> dict[str, Any]:
        accepted = self.import_eligible and not self.refusal_reasons
        return {
            "candidate_id": self.candidate_id,
            "method_id": self.method_id,
            "package_id": self.package_id,
            "candidate_type": self.candidate_type,
            "route": self.route,
            "state": self.state,
            "accepted": accepted,
            "rejected": not accepted,
            "import_eligible": self.import_eligible,
            "refusal_reasons": list(self.refusal_reasons),
            "remediation_hints": list(self.remediation_hints),
            "allowed_uses": ["import_boundary_diagnostics"],
            "excluded_uses": [TRUSTED_IMPORT_USE, "production_ladybugdb_write", "embedding_generation"],
            **_safety_flags(),
        }


@dataclass(frozen=True)
class ImportBoundaryRehearsal:
    """Negative isolated import boundary rehearsal artifact."""

    rehearsal_id: str
    source_benchmark_id: str
    candidates: tuple[ImportCandidate, ...]
    recommendation: str = "positive_import_blocked"
    remediation_hints: tuple[str, ...] = ()
    caveats: tuple[str, ...] = ()

    def to_contract(self) -> dict[str, Any]:
        candidate_records = [candidate.to_contract() for candidate in self.candidates]
        accepted_count = sum(1 for candidate in candidate_records if candidate["accepted"])
        rejected_count = sum(1 for candidate in candidate_records if candidate["rejected"])
        return {
            "schema_version": SCHEMA_VERSION,
            "rehearsal_id": self.rehearsal_id,
            "source_benchmark_id": self.source_benchmark_id,
            "candidate_count": len(candidate_records),
            "accepted_count": accepted_count,
            "rejected_count": rejected_count,
            "candidates": candidate_records,
            "refusal_counts": _merge_refusals(candidate_records),
            "recommendation": self.recommendation,
            "remediation_hints": list(self.remediation_hints),
            "caveats": list(self.caveats),
            **_safety_flags(),
        }


def build_import_boundary_rehearsal_from_benchmark(
    *,
    summary_path: str | Path,
    diagnostics_path: str | Path,
    rehearsal_id: str = "m005-s07-negative-import-boundary",
) -> dict[str, Any]:
    """Build negative rehearsal evidence from S06 benchmark artifacts.

    S06 diagnostics are intentionally aggregate and redacted. This adapter
    expands aggregate refusal counts into synthetic candidate identities so the
    import boundary can prove that every benchmarked candidate is rejected
    without reading raw paper files or attempting any graph writes.
    """
    summary = _read_json_object(Path(summary_path))
    diagnostics = _read_jsonl_objects(Path(diagnostics_path))
    candidates: list[ImportCandidate] = []
    for method_record in diagnostics:
        method_id = str(method_record.get("method_id") or "unknown_method")
        state = _single_key_or_none(method_record.get("counts_by_state"))
        route = _single_key_or_none(method_record.get("counts_by_route"))
        candidate_type = _single_key_or_none(method_record.get("counts_by_chunk_type")) or "benchmark_candidate"
        for reason, count in sorted(_int_counts(method_record.get("refusal_counts")).items()):
            for index in range(count):
                candidates.append(
                    ImportCandidate(
                        candidate_id=f"{method_id}:{reason}:{index + 1:06d}",
                        method_id=method_id,
                        package_id=f"benchmark-method:{method_id}",
                        candidate_type=candidate_type,
                        route=route,
                        state=state,
                        import_eligible=False,
                        refusal_reasons=(reason,),
                        remediation_hints=(_remediation_hint(reason),),
                    )
                )
    rehearsal = ImportBoundaryRehearsal(
        rehearsal_id=rehearsal_id,
        source_benchmark_id=str(summary.get("input_corpus") or "m005-s06-chunking-benchmark"),
        candidates=tuple(candidates),
        remediation_hints=(
            "create_reviewed_import_eligible_subset",
            "complete_positive_import_review_before_trusted_kg_import",
        ),
        caveats=tuple(_string_list(summary.get("caveats")))
        + tuple(_missing_source_caveats(summary.get("aggregate", {}).get("missing_source_counts"))),
    ).to_contract()
    rehearsal["source_benchmark_summary"] = {
        "method_count": summary.get("method_count"),
        "total_chunk_count": summary.get("aggregate", {}).get("total_chunk_count"),
        "total_refused_chunk_count": summary.get("aggregate", {}).get("total_refused_chunk_count"),
        "total_import_eligible_chunk_count": summary.get("aggregate", {}).get("total_import_eligible_chunk_count"),
        "recommendation_status": summary.get("recommendation_status"),
    }
    return rehearsal


def validate_import_boundary_rehearsal(rehearsal: dict[str, Any]) -> RehearsalValidationResult:
    """Validate negative import-boundary evidence and redaction/no-write invariants."""
    diagnostics: list[RehearsalDiagnostic] = []
    rehearsal_id = _string_or_none(rehearsal.get("rehearsal_id"))
    diagnostics.extend(
        _required_fields(
            rehearsal,
            fields=(
                "schema_version",
                "rehearsal_id",
                "source_benchmark_id",
                "candidate_count",
                "accepted_count",
                "rejected_count",
                "candidates",
                "refusal_counts",
            ),
            object_id=rehearsal_id,
            object_type="rehearsal",
        )
    )
    diagnostics.extend(_validate_redaction(rehearsal, object_id=rehearsal_id, object_type="rehearsal"))
    diagnostics.extend(_validate_safety_flags(rehearsal, object_id=rehearsal_id, object_type="rehearsal"))
    if rehearsal.get("schema_version") != SCHEMA_VERSION:
        diagnostics.append(RehearsalDiagnostic(reason="schema_version_mismatch", object_id=rehearsal_id, object_type="rehearsal"))
    candidates = _list_of_dicts(rehearsal.get("candidates"))
    if rehearsal.get("candidate_count") != len(candidates):
        diagnostics.append(RehearsalDiagnostic(reason="candidate_count_mismatch", object_id=rehearsal_id, object_type="rehearsal"))
    accepted_count = sum(1 for candidate in candidates if candidate.get("accepted") is True)
    rejected_count = sum(1 for candidate in candidates if candidate.get("rejected") is True)
    if rehearsal.get("accepted_count") != accepted_count:
        diagnostics.append(RehearsalDiagnostic(reason="accepted_count_mismatch", object_id=rehearsal_id, object_type="rehearsal"))
    if rehearsal.get("rejected_count") != rejected_count:
        diagnostics.append(RehearsalDiagnostic(reason="rejected_count_mismatch", object_id=rehearsal_id, object_type="rehearsal"))
    if rehearsal.get("refusal_counts") != _merge_refusals(candidates):
        diagnostics.append(RehearsalDiagnostic(reason="refusal_counts_mismatch", object_id=rehearsal_id, object_type="rehearsal"))
    for candidate in candidates:
        diagnostics.extend(_validate_candidate(candidate))
    return RehearsalValidationResult(valid_rehearsal=not diagnostics, diagnostics=tuple(diagnostics))


def _validate_candidate(candidate: dict[str, Any]) -> list[RehearsalDiagnostic]:
    candidate_id = _string_or_none(candidate.get("candidate_id"))
    diagnostics = _required_fields(
        candidate,
        fields=(
            "candidate_id",
            "method_id",
            "package_id",
            "candidate_type",
            "accepted",
            "rejected",
            "import_eligible",
            "refusal_reasons",
            "allowed_uses",
            "excluded_uses",
        ),
        object_id=candidate_id,
        object_type="candidate",
    )
    diagnostics.extend(_validate_safety_flags(candidate, object_id=candidate_id, object_type="candidate"))
    accepted = candidate.get("accepted") is True
    rejected = candidate.get("rejected") is True
    import_eligible = candidate.get("import_eligible") is True
    refusal_reasons = _string_list(candidate.get("refusal_reasons"))
    if accepted and rejected:
        diagnostics.append(
            RehearsalDiagnostic(reason="candidate_both_accepted_and_rejected", object_id=candidate_id, object_type="candidate")
        )
    if accepted and (not import_eligible or refusal_reasons):
        diagnostics.append(
            RehearsalDiagnostic(reason="accepted_candidate_not_import_eligible", object_id=candidate_id, object_type="candidate")
        )
    if rejected and import_eligible and not refusal_reasons:
        diagnostics.append(
            RehearsalDiagnostic(reason="rejected_candidate_missing_refusal", object_id=candidate_id, object_type="candidate")
        )
    if rejected and not refusal_reasons:
        diagnostics.append(RehearsalDiagnostic(reason="rejected_candidate_missing_refusal", object_id=candidate_id, object_type="candidate"))
    if TRUSTED_IMPORT_USE in _string_list(candidate.get("allowed_uses")):
        diagnostics.append(RehearsalDiagnostic(reason="candidate_allows_trusted_import", object_id=candidate_id, object_type="candidate"))
    if TRUSTED_IMPORT_USE not in _string_list(candidate.get("excluded_uses")):
        diagnostics.append(RehearsalDiagnostic(reason="candidate_missing_import_exclusion", object_id=candidate_id, object_type="candidate"))
    return diagnostics


def _validate_safety_flags(payload: dict[str, Any], *, object_id: str | None, object_type: str) -> list[RehearsalDiagnostic]:
    diagnostics: list[RehearsalDiagnostic] = []
    for field_name, expected in _safety_flags().items():
        if field_name in payload and payload.get(field_name) is not expected:
            diagnostics.append(RehearsalDiagnostic(reason=f"unsafe_{field_name}", object_id=object_id, object_type=object_type))
    return diagnostics


def _validate_redaction(payload: Any, *, object_id: str | None, object_type: str) -> list[RehearsalDiagnostic]:
    return _validate_nested_redaction(payload, object_id=object_id, object_type=object_type, path=())


def _validate_nested_redaction(
    value: Any,
    *,
    object_id: str | None,
    object_type: str,
    path: tuple[str, ...],
) -> list[RehearsalDiagnostic]:
    diagnostics: list[RehearsalDiagnostic] = []
    if isinstance(value, dict):
        forbidden = (
            FORBIDDEN_RAW_FIELDS
            | FORBIDDEN_EMBEDDING_FIELDS
            | FORBIDDEN_VECTOR_FIELDS
            | FORBIDDEN_SECRET_FIELDS
            | FORBIDDEN_OPTIMIZER_FIELDS
        ) & set(value)
        for field_name in sorted(forbidden):
            diagnostics.append(
                RehearsalDiagnostic(
                    reason=_leakage_reason(field_name),
                    object_id=_redaction_path(object_id=object_id, object_type=object_type, path=(*path, str(field_name))),
                    object_type=object_type,
                )
            )
        for key, nested_value in value.items():
            diagnostics.extend(_validate_nested_redaction(nested_value, object_id=object_id, object_type=object_type, path=(*path, str(key))))
    elif isinstance(value, list):
        for index, nested_value in enumerate(value):
            diagnostics.extend(_validate_nested_redaction(nested_value, object_id=object_id, object_type=object_type, path=(*path, str(index))))
    return diagnostics


def _leakage_reason(field_name: str) -> str:
    if field_name in FORBIDDEN_RAW_FIELDS:
        return "raw_text_leakage"
    if field_name in FORBIDDEN_EMBEDDING_FIELDS:
        return "embedding_leakage"
    if field_name in FORBIDDEN_VECTOR_FIELDS:
        return "vector_leakage"
    if field_name in FORBIDDEN_SECRET_FIELDS:
        return "secret_leakage"
    return "optimizer_trace_leakage"


def _merge_refusals(candidates: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for candidate in candidates:
        for reason in _string_list(candidate.get("refusal_reasons")):
            counts[reason] = counts.get(reason, 0) + 1
    return dict(sorted(counts.items()))


def _required_fields(
    payload: dict[str, Any],
    *,
    fields: tuple[str, ...],
    object_id: str | None,
    object_type: str,
) -> list[RehearsalDiagnostic]:
    diagnostics: list[RehearsalDiagnostic] = []
    for field_name in fields:
        if field_name not in payload or payload.get(field_name) is None:
            diagnostics.append(RehearsalDiagnostic(reason=f"missing_{field_name}", object_id=object_id, object_type=object_type))
    return diagnostics


def _read_json_object(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"Expected JSON object at {path}")
    return value


def _read_jsonl_objects(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            stripped = line.strip()
            if not stripped:
                continue
            value = json.loads(stripped)
            if not isinstance(value, dict):
                raise ValueError(f"Expected JSON object at {path}:{line_number}")
            records.append(value)
    return records


def _int_counts(value: Any) -> dict[str, int]:
    if not isinstance(value, dict):
        return {}
    counts: dict[str, int] = {}
    for key, count in value.items():
        if count is None:
            continue
        integer_count = int(count)
        if integer_count > 0:
            counts[str(key)] = integer_count
    return counts


def _single_key_or_none(value: Any) -> str | None:
    counts = _int_counts(value)
    if len(counts) == 1:
        return next(iter(counts))
    return None


def _missing_source_caveats(value: Any) -> list[str]:
    return [f"{reason}:{count}" for reason, count in _int_counts(value).items()]


def _remediation_hint(reason: str) -> str:
    if "baseline" in reason:
        return "replace_baseline_with_reviewed_structure_aware_candidate"
    if "estimated" in reason:
        return "run_real_chunker_and_review_output"
    if "source" in reason or "pdf" in reason:
        return "repair_missing_source_artifact"
    if "route" in reason or "requires_review" in reason:
        return "complete_route_specific_review"
    return "create_reviewed_import_eligible_subset"


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _string_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [str(item) for item in value if item is not None]


def _string_or_none(value: Any) -> str | None:
    if value is None:
        return None
    return str(value)


def _redaction_path(*, object_id: str | None, object_type: str, path: tuple[str, ...]) -> str:
    prefix = object_id or object_type
    return f"{prefix}:{'.'.join(path)}"
