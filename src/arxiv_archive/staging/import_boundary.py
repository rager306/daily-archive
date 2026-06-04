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

from arxiv_archive.identity.canonicalization import canonical_import_candidate_id, canonical_package_id

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
        "network_fetch_attempted": False,
        "raw_payload_embedded_in_metadata": False,
        "chunk_ready_claimed_for_non_parser_ready_rows": False,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "kg_readiness_claimed": False,
        "graph_write_attempted": False,
        "ladybugdb_written": False,
        "production_ladybugdb_write_allowed": False,
        "production_persistence_attempted": False,
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
                        candidate_id=canonical_import_candidate_id(method_id=method_id, refusal_reason=reason, index=index + 1),
                        method_id=method_id,
                        package_id=canonical_package_id(method_id=method_id),
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


def build_m031_import_boundary_rehearsal(
    *,
    summary_path: str | Path,
    closeout_summary_path: str | Path,
    graph_readiness_package_paths: list[str | Path] | tuple[str | Path, ...],
    independent_review_events_path: str | Path,
    rehearsal_id: str = "m031-s05-refusal-only-import-boundary",
) -> dict[str, Any]:
    """Build the M031 refusal-only import-boundary rehearsal from S04 artifacts.

    S04 can contain structural route/state labels that look graph-ready, but the
    independent graph-readiness output contract is still pending. This adapter
    therefore emits one rejected candidate per S04 row: parser-ready packages are
    refused until a completed independent review exists, and non-parser-ready
    rows preserve their zero-chunk refusal diagnostic codes. The resulting
    contract is metadata-only and uses the same fail-closed validator as the M005
    negative rehearsal path.
    """
    summary = _read_json_object(Path(summary_path))
    closeout_summary = _read_json_object(Path(closeout_summary_path))
    review_events = _read_jsonl_objects(Path(independent_review_events_path))
    graph_packages = [_read_json_object(Path(path)) for path in graph_readiness_package_paths]
    graph_packages_by_key = {str(package.get("package_key") or package.get("paper_id")): package for package in graph_packages}
    completed_review_package_ids = _completed_review_package_ids(review_events)

    candidates: list[ImportCandidate] = []
    for row_index, row in enumerate(_list_of_dicts(summary.get("results")), start=1):
        package_id = str(row.get("package_key") or row.get("identity") or f"m031-row-{row_index}")
        if row.get("parser_ready") is True and row.get("status") == "chunked":
            graph_package = graph_packages_by_key.get(package_id, {})
            output_completed = bool(graph_package.get("output_contract_completed"))
            review_completed = package_id in completed_review_package_ids or bool(row.get("independent_review_completed"))
            refused_for_review = not (output_completed and review_completed)
            reason = "completed_independent_graph_readiness_review_required" if refused_for_review else "positive_import_blocked"
            candidate = ImportCandidate(
                candidate_id=canonical_import_candidate_id(method_id="m031_graph_readiness", refusal_reason=reason, index=row_index),
                method_id="m031_graph_readiness",
                package_id=package_id,
                candidate_type="graph_readiness_package",
                route=_single_key_or_none(graph_package.get("counts_by_route")) or _string_or_none(row.get("source_role")),
                state=_single_key_or_none(graph_package.get("counts_by_state")) or _string_or_none(row.get("terminal_state")),
                import_eligible=False,
                refusal_reasons=(reason,),
                remediation_hints=("independent_graph_readiness_review_required", "complete_output_contract_before_import"),
            ).to_contract()
            candidate.update(
                {
                    "review_state": graph_package.get("review_state") or row.get("review_status"),
                    "output_contract_completed": output_completed,
                    "independent_review_completed": review_completed,
                    "source_json_path": row.get("json_path"),
                    "chunk_count": int(row.get("chunk_count") or graph_package.get("chunk_count") or 0),
                }
            )
        else:
            reason = str(row.get("diagnostic_code") or row.get("refusal_code") or "non_parser_ready_zero_chunk_refusal")
            candidate = ImportCandidate(
                candidate_id=canonical_import_candidate_id(method_id="m031_zero_chunk_refusal", refusal_reason=reason, index=row_index),
                method_id="m031_zero_chunk_refusal",
                package_id=package_id,
                candidate_type="zero_chunk_refusal",
                route=_string_or_none(row.get("source_role")),
                state=_string_or_none(row.get("terminal_state") or row.get("status")),
                import_eligible=False,
                refusal_reasons=(reason,),
                remediation_hints=("repair_parser_ready_source_before_chunking",),
            ).to_contract()
            candidate.update(
                {
                    "review_state": row.get("review_status"),
                    "output_contract_completed": False,
                    "independent_review_completed": bool(row.get("independent_review_completed")),
                    "source_json_path": row.get("json_path"),
                    "chunk_count": int(row.get("chunk_count") or 0),
                }
            )
        candidates.append(candidate)

    contract = {
        "schema_version": SCHEMA_VERSION,
        "rehearsal_id": rehearsal_id,
        "source_benchmark_id": str(summary.get("selection_id") or "m031-catalog-backed-replay-v1"),
        "candidate_count": len(candidates),
        "accepted_count": sum(1 for candidate in candidates if candidate["accepted"] is True),
        "rejected_count": sum(1 for candidate in candidates if candidate["rejected"] is True),
        "candidates": candidates,
        "refusal_counts": _merge_refusals(candidates),
        "recommendation": "positive_import_blocked",
        "remediation_hints": [
            "complete_independent_graph_readiness_review",
            "keep_trusted_kg_import_disabled_until_review_completion",
        ],
        "caveats": ["m031_refusal_only_import_boundary_rehearsal", "metadata_only_no_write_rehearsal"],
        "source_m031_summary": {
            "row_count": summary.get("row_count"),
            "parser_ready_row_count": summary.get("parser_ready_row_count"),
            "zero_chunk_refusal_count": summary.get("zero_chunk_refusal_count"),
            "package_count": summary.get("package_count"),
            "graph_readiness_package_count": summary.get("graph_readiness_package_count"),
            "pending_graph_readiness_review_count": closeout_summary.get("pending_graph_readiness_review_count"),
            "independent_review_completed_count": closeout_summary.get("independent_review_completed_count"),
            "trusted_kg_import_allowed": closeout_summary.get("trusted_kg_import_allowed"),
            "graph_import_allowed": closeout_summary.get("graph_import_allowed"),
            "production_import_attempted": closeout_summary.get("production_import_attempted"),
            "ladybugdb_written": closeout_summary.get("ladybugdb_written"),
        },
        **_safety_flags(),
    }
    return contract


def write_import_boundary_rehearsal_run(
    *,
    summary_path: str | Path,
    diagnostics_path: str | Path,
    output_dir: str | Path,
    rehearsal_id: str = "m005-s07-negative-import-boundary",
) -> dict[str, Path]:
    """Write redacted negative import-boundary summary and candidate diagnostics."""
    rehearsal = build_import_boundary_rehearsal_from_benchmark(
        summary_path=summary_path,
        diagnostics_path=diagnostics_path,
        rehearsal_id=rehearsal_id,
    )
    validation = validate_import_boundary_rehearsal(rehearsal)
    if not validation.valid_rehearsal:
        reasons = ", ".join(validation.refusal_counts)
        raise ValueError(f"Invalid import-boundary rehearsal artifact: {reasons}")
    destination = Path(output_dir)
    destination.mkdir(parents=True, exist_ok=True)
    summary_record = {key: value for key, value in rehearsal.items() if key != "candidates"}
    summary_file = destination / "import-boundary-summary.json"
    diagnostics_file = destination / "import-boundary-diagnostics.jsonl"
    summary_file.write_text(json.dumps(summary_record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    with diagnostics_file.open("w", encoding="utf-8") as handle:
        for candidate in rehearsal["candidates"]:
            handle.write(json.dumps(candidate, sort_keys=True, separators=(",", ":")) + "\n")
    return {"summary_path": summary_file, "diagnostics_path": diagnostics_file}


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


def _completed_review_package_ids(events: list[dict[str, Any]]) -> set[str]:
    package_ids: set[str] = set()
    for event in events:
        if event.get("independent_review_completed") is not True or event.get("output_contract_completed") is not True:
            continue
        package_id = _string_or_none(event.get("paper_id"))
        if package_id:
            package_ids.add(package_id)
        for selected_id in _string_list(event.get("selected_paper_ids")):
            package_ids.add(selected_id)
    return package_ids


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


__all__ = [
    "SCHEMA_VERSION",
    "TRUSTED_IMPORT_USE",
    "FORBIDDEN_RAW_FIELDS",
    "FORBIDDEN_EMBEDDING_FIELDS",
    "FORBIDDEN_VECTOR_FIELDS",
    "FORBIDDEN_SECRET_FIELDS",
    "FORBIDDEN_OPTIMIZER_FIELDS",
    "RehearsalDiagnostic",
    "RehearsalValidationResult",
    "ImportCandidate",
    "ImportBoundaryRehearsal",
    "build_import_boundary_rehearsal_from_benchmark",
    "build_m031_import_boundary_rehearsal",
    "write_import_boundary_rehearsal_run",
    "validate_import_boundary_rehearsal",
]
