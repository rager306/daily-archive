# Formerly: src/arxiv_archive/universal_kb_contracts.py

"""Executable Universal KB contracts for M035 no-write prototypes.

These records implement the fail-closed subset of the M034 contract package.
They are intentionally small frozen dataclasses: boundary conversion belongs in
sidecar adapters, while semantic safety invariants live here.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from typing import Any

SAFE_REDACTION_STATES = frozenset({"redacted", "metadata_only", "sanitized"})
AUTHORITATIVE_REVIEW_STATES = frozenset({"approved", "ready", "import_eligible"})
FORBIDDEN_DIAGNOSTIC_KEYS = frozenset(
    {
        "text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "claim_text",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
        "secret",
        "token",
        "api_key",
        "credential",
        "prompt",
        "raw_prompt",
    }
)


def _tuple(values: tuple[str, ...] | list[str] | None) -> tuple[str, ...]:
    if values is None:
        return ()
    return tuple(values)


def _as_json_safe_dict(value: Any) -> dict[str, Any]:
    data = asdict(value)
    return {key: _json_safe(item) for key, item in data.items()}


def _json_safe(value: Any) -> Any:
    if isinstance(value, tuple):
        return [_json_safe(item) for item in value]
    if isinstance(value, dict):
        return {str(key): _json_safe(item) for key, item in value.items()}
    return value


def _require_non_empty(value: str, field_name: str) -> None:
    if not value:
        raise ValueError(f"{field_name} must be non-empty")


def _reject_forbidden_diagnostic_keys(data: dict[str, Any]) -> None:
    lower_keys = {key.lower() for key in data}
    forbidden = sorted(lower_keys & FORBIDDEN_DIAGNOSTIC_KEYS)
    if forbidden:
        raise ValueError(f"diagnostic payload contains forbidden keys: {', '.join(forbidden)}")


@dataclass(frozen=True, slots=True)
class SafetyFlags:
    """Fail-closed graph/import authorization flags from M034."""

    graph_import_allowed: bool = False
    graphdb_written: bool = False
    ladybugdb_written: bool = False
    production_import_attempted: bool = False
    import_eligible: bool = False

    def __post_init__(self) -> None:
        self.assert_no_write()

    def assert_no_write(self) -> None:
        if any(
            (
                self.graph_import_allowed,
                self.graphdb_written,
                self.ladybugdb_written,
                self.production_import_attempted,
                self.import_eligible,
            )
        ):
            raise ValueError(
                "M034 forbids graph import, graph writes, production import, "
                "and import eligibility in no-write prototype contracts"
            )

    def to_dict(self) -> dict[str, bool]:
        return _as_json_safe_dict(self)


@dataclass(frozen=True, slots=True)
class EvidenceArtifactRecord:
    artifact_id: str
    artifact_type: str
    producer: str
    input_hash: str
    tool_version: str
    output_path: str
    diagnostic_refs: tuple[str, ...] = field(default_factory=tuple)

    def __post_init__(self) -> None:
        for field_name in (
            "artifact_id",
            "artifact_type",
            "producer",
            "input_hash",
            "tool_version",
            "output_path",
        ):
            _require_non_empty(str(getattr(self, field_name)), field_name)
        object.__setattr__(self, "diagnostic_refs", _tuple(self.diagnostic_refs))

    def to_dict(self) -> dict[str, Any]:
        return _as_json_safe_dict(self)


@dataclass(frozen=True, slots=True)
class CandidatePacket:
    candidate_id: str
    evidence_refs: tuple[str, ...]
    candidate_type: str
    review_state: str = "pending"
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        _require_non_empty(self.candidate_id, "candidate_id")
        _require_non_empty(self.candidate_type, "candidate_type")
        object.__setattr__(self, "evidence_refs", _tuple(self.evidence_refs))
        if self.review_state in AUTHORITATIVE_REVIEW_STATES:
            raise ValueError("candidate packet cannot carry authoritative review state")
        self.assert_no_write()

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return _as_json_safe_dict(self)


@dataclass(frozen=True, slots=True)
class ReviewPacket:
    packet_id: str
    candidate_refs: tuple[str, ...]
    diagnostics: tuple[str, ...]
    review_required: bool = True
    review_state: str = "pending"
    reviewer_refs: tuple[str, ...] = field(default_factory=tuple)
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        _require_non_empty(self.packet_id, "packet_id")
        object.__setattr__(self, "candidate_refs", _tuple(self.candidate_refs))
        object.__setattr__(self, "diagnostics", _tuple(self.diagnostics))
        object.__setattr__(self, "reviewer_refs", _tuple(self.reviewer_refs))
        if self.review_state in AUTHORITATIVE_REVIEW_STATES or not self.review_required:
            raise ValueError("review packet cannot approve readiness or bypass review")
        self.assert_no_write()

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return _as_json_safe_dict(self)


@dataclass(frozen=True, slots=True)
class ProcessingJob:
    job_id: str
    stage: str
    status: str
    attempt_count: int
    retry_after: str | None
    last_error_code: str | None
    input_refs: tuple[str, ...]
    output_paths: tuple[str, ...]

    def __post_init__(self) -> None:
        for field_name in ("job_id", "stage", "status"):
            _require_non_empty(str(getattr(self, field_name)), field_name)
        if self.attempt_count < 0:
            raise ValueError("attempt_count must be >= 0")
        object.__setattr__(self, "input_refs", _tuple(self.input_refs))
        object.__setattr__(self, "output_paths", _tuple(self.output_paths))

    def to_dict(self) -> dict[str, Any]:
        return _as_json_safe_dict(self)


@dataclass(frozen=True, slots=True)
class DependencyRecord:
    dependency_id: str
    upstream_ref: str
    downstream_ref: str
    required_state: str
    stale_on_hash_change: bool

    def __post_init__(self) -> None:
        for field_name in ("dependency_id", "upstream_ref", "downstream_ref", "required_state"):
            _require_non_empty(str(getattr(self, field_name)), field_name)

    def to_dict(self) -> dict[str, Any]:
        return _as_json_safe_dict(self)


@dataclass(frozen=True, slots=True)
class FailureRecord:
    failure_id: str
    job_id: str
    failure_class: str
    error_code: str
    retryable: bool
    redacted_message: str
    occurred_at: str

    def __post_init__(self) -> None:
        for field_name in (
            "failure_id",
            "job_id",
            "failure_class",
            "error_code",
            "redacted_message",
            "occurred_at",
        ):
            _require_non_empty(str(getattr(self, field_name)), field_name)

    def to_dict(self) -> dict[str, Any]:
        return _as_json_safe_dict(self)


@dataclass(frozen=True, slots=True)
class ToolInvocationRecord:
    """Sanitized trace for bounded helper/tool calls.

    This record intentionally stores metadata and diagnostic references only.
    It must not persist raw prompts, raw corpus text, credentials, or model
    reasoning. Helper output is never source of truth for graph promotion.
    """

    invocation_id: str
    tool_name: str
    model: str
    prompt_version: str
    input_hash: str
    schema_version: str
    redaction_state: str
    diagnostic_refs: tuple[str, ...]
    latency_ms: int | None = None
    cost_units: float | None = None
    helper_evidence_only: bool = True
    minimax_source_of_truth: bool = False
    raw_prompt_persisted: bool = False
    credential_value_logged: bool = False

    def __post_init__(self) -> None:
        for field_name in (
            "invocation_id",
            "tool_name",
            "model",
            "prompt_version",
            "input_hash",
            "schema_version",
            "redaction_state",
        ):
            _require_non_empty(str(getattr(self, field_name)), field_name)
        object.__setattr__(self, "diagnostic_refs", _tuple(self.diagnostic_refs))
        if self.redaction_state not in SAFE_REDACTION_STATES:
            raise ValueError("redaction_state must be safe for persistence")
        if self.latency_ms is not None and self.latency_ms < 0:
            raise ValueError("latency_ms must be >= 0")
        if self.cost_units is not None and self.cost_units < 0:
            raise ValueError("cost_units must be >= 0")
        if not self.helper_evidence_only or self.minimax_source_of_truth:
            raise ValueError("tool invocation records are helper evidence only")
        if self.raw_prompt_persisted or self.credential_value_logged:
            raise ValueError("tool invocation records must not persist prompts or credential values")

    def to_sanitized_dict(self) -> dict[str, Any]:
        data = _as_json_safe_dict(self)
        _reject_forbidden_diagnostic_keys(data)
        return data

    def to_dict(self) -> dict[str, Any]:
        return self.to_sanitized_dict()
