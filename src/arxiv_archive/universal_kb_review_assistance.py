"""Diagnostic-only review assistance contracts for M035 Universal KB."""

from __future__ import annotations

import hashlib
import re
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from arxiv_archive.universal_kb_contracts import (
    AUTHORITATIVE_REVIEW_STATES,
    FORBIDDEN_DIAGNOSTIC_KEYS,
    CandidatePacket,
    SafetyFlags,
    ToolInvocationRecord,
)

REVIEW_ASSISTANCE_SCHEMA_VERSION = "m035-review-assistance.v1"
REVIEW_ASSISTANCE_PROMPT_VERSION = "universal_kb_review_assistance_v1"
PROMPT_PATH = Path("prompts/universal_kb_review_assistance_v1.md")
_SECRET_SHAPED_PATTERN = re.compile(
    r"(?i)(sk-[a-z0-9][a-z0-9._-]{8,}|bearer\s+[a-z0-9._-]{12,}|x-api-key\s*[:=]\s*[^\s]+)"
)

_AUTHORITY_KEYS = frozenset(
    {
        "approved",
        "ready",
        "import_eligible",
        "graph_import_allowed",
        "graphdb_written",
        "ladybugdb_written",
        "production_import_attempted",
        "trusted_kg_import_allowed",
        "promoted",
    }
)


def _tuple(values: tuple[str, ...] | list[str] | None) -> tuple[str, ...]:
    if values is None:
        return ()
    return tuple(str(value) for value in values)


def _require_non_empty(value: str, field_name: str) -> None:
    if not value.strip():
        raise ValueError(f"{field_name} must be non-empty")


def _require_metadata_safe(value: str, field_name: str) -> None:
    lowered = value.lower()
    if any(forbidden in lowered for forbidden in FORBIDDEN_DIAGNOSTIC_KEYS) or _SECRET_SHAPED_PATTERN.search(value):
        raise ValueError(f"{field_name} must be metadata-only")


def _safe_dict(value: Any) -> Any:
    if isinstance(value, SafetyFlags):
        return value.to_dict()
    if isinstance(value, tuple):
        return list(value)
    if hasattr(value, "__dataclass_fields__"):
        return {key: _safe_dict(item) for key, item in asdict(value).items()}
    return value


@dataclass(frozen=True, slots=True)
class ReviewAssistancePacket:
    candidate_id: str
    schema_version: str
    diagnostics: tuple[str, ...]
    confidence: float
    flags: tuple[str, ...]
    review_state: str = "diagnostic_only"
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        _require_non_empty(self.candidate_id, "candidate_id")
        _require_non_empty(self.schema_version, "schema_version")
        if self.review_state in AUTHORITATIVE_REVIEW_STATES or self.review_state != "diagnostic_only":
            raise ValueError("review assistance must remain diagnostic-only")
        if not 0 <= self.confidence <= 1:
            raise ValueError("confidence must be between 0 and 1")
        diagnostics = _tuple(self.diagnostics)
        flags = _tuple(self.flags)
        if not diagnostics:
            raise ValueError("diagnostics must be non-empty")
        for diagnostic in diagnostics:
            _require_non_empty(diagnostic, "diagnostic")
            _require_metadata_safe(diagnostic, "diagnostic")
        for flag in flags:
            _require_non_empty(flag, "flag")
            _require_metadata_safe(flag, "flag")
            if flag in _AUTHORITY_KEYS:
                raise ValueError("review assistance flags cannot grant authority")
        self.safety_flags.assert_no_write()
        object.__setattr__(self, "diagnostics", diagnostics)
        object.__setattr__(self, "flags", flags)

    def to_dict(self) -> dict[str, Any]:
        return _safe_dict(self)


def review_assistance_prompt_hash(prompt_path: Path = PROMPT_PATH) -> str:
    content = prompt_path.read_bytes()
    return f"sha256:{hashlib.sha256(content).hexdigest()}"


def build_review_assistance_packet(
    *,
    candidate: CandidatePacket,
    diagnostics: tuple[str, ...] | list[str],
    confidence: float,
    flags: tuple[str, ...] | list[str] | None = None,
) -> ReviewAssistancePacket:
    candidate.assert_no_write()
    return ReviewAssistancePacket(
        candidate_id=candidate.candidate_id,
        schema_version=REVIEW_ASSISTANCE_SCHEMA_VERSION,
        diagnostics=_tuple(diagnostics),
        confidence=confidence,
        flags=_tuple(flags),
    )


def build_review_tool_invocation_record(
    *,
    invocation_id: str,
    model: str,
    input_hash: str,
    review_packet: ReviewAssistancePacket,
    diagnostic_refs: tuple[str, ...] | list[str] | None = None,
    latency_ms: int | None = None,
    cost_units: float | None = None,
    redaction_state: str = "redacted",
) -> ToolInvocationRecord:
    """Build a sanitized helper trace for review assistance.

    The trace references diagnostics by stable metadata ids only. It does not
    persist prompt text, model output payloads, corpus text, secrets, or
    internal reasoning.
    """
    refs = _tuple(diagnostic_refs)
    if not refs:
        refs = tuple(f"{review_packet.candidate_id}:{diagnostic}" for diagnostic in review_packet.diagnostics)
    for diagnostic_ref in refs:
        _require_metadata_safe(diagnostic_ref, "diagnostic_ref")
    return ToolInvocationRecord(
        invocation_id=invocation_id,
        tool_name="universal_kb_review_assistance",
        model=model,
        prompt_version=REVIEW_ASSISTANCE_PROMPT_VERSION,
        input_hash=input_hash,
        schema_version=review_packet.schema_version,
        redaction_state=redaction_state,
        diagnostic_refs=refs,
        latency_ms=latency_ms,
        cost_units=cost_units,
    )


def validate_review_assistance_tool_input(payload: dict[str, Any], *, candidate: CandidatePacket) -> ReviewAssistancePacket:
    if not isinstance(payload, dict):
        raise ValueError("review assistance tool input must be an object")
    unsafe_keys = sorted(key for key in payload if key in _AUTHORITY_KEYS and payload.get(key))
    if unsafe_keys:
        raise ValueError("review assistance tool input cannot grant authority")
    diagnostics = payload.get("diagnostics")
    confidence = payload.get("confidence")
    flags = payload.get("flags", ())
    if not isinstance(diagnostics, list) or not all(isinstance(item, str) for item in diagnostics):
        raise ValueError("diagnostics must be a list of strings")
    if not isinstance(flags, list) or not all(isinstance(item, str) for item in flags):
        raise ValueError("flags must be a list of strings")
    if not isinstance(confidence, int | float):
        raise ValueError("confidence must be numeric")
    return build_review_assistance_packet(
        candidate=candidate,
        diagnostics=diagnostics,
        confidence=float(confidence),
        flags=flags,
    )
