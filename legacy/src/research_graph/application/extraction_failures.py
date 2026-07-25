"""Typed extraction failure taxonomy for the M201 pilot (S07).

Maps client diagnostics and paper/chunk outcomes into stable operator codes.
Does not authorize graph writes or invent success from partial failures.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal

ExtractionFailureCode = Literal[
    "MALFORMED_OUTPUT",
    "EMPTY_CANDIDATES",
    "TRANSPORT_FAILURE",
    "MISSING_EVIDENCE",
    "PARTIAL_PAPER",
    "QUOTA_OR_AUTH",
    "UNKNOWN",
]


@dataclass(frozen=True)
class ExtractionFailureRecord:
    """Redacted failure record for one extraction attempt."""

    code: ExtractionFailureCode
    stage: str
    message: str
    provider: str | None = None
    fallback_used: bool = False

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "stage": self.stage,
            "message": self.message,
            "provider": self.provider,
            "fallback_used": self.fallback_used,
            "credential_value_logged": False,
        }


def classify_extraction_failure(
    *,
    status: str,
    entity_count: int = 0,
    relation_count: int = 0,
    evidence_linked_count: int = 0,
    diagnostic: str | None = None,
    client_diagnostics: dict[str, Any] | None = None,
) -> ExtractionFailureRecord | None:
    """Return a failure record when the outcome is not a full success.

    Full success: status=done with at least one candidate and no error diagnostic.
    """
    diag = client_diagnostics or {}
    codes = " ".join(str(c) for c in (diag.get("diagnostic_codes") or ()))
    provider = diag.get("provider") or diag.get("used_provider")
    fallback_used = bool(diag.get("fallback_used"))
    message = diagnostic or codes or status

    if status == "failed":
        if "transport" in message or "transport" in codes:
            code: ExtractionFailureCode = "TRANSPORT_FAILURE"
        elif "missing_api_key" in message or "auth" in codes.lower() or "missing_api_key" in codes:
            code = "QUOTA_OR_AUTH"
        else:
            code = "UNKNOWN"
        return ExtractionFailureRecord(
            code=code,
            stage="paper",
            message=message,
            provider=str(provider) if provider else None,
            fallback_used=fallback_used,
        )

    if status == "empty":
        return ExtractionFailureRecord(
            code="PARTIAL_PAPER",
            stage="paper",
            message=message or "empty_chunks",
            provider=str(provider) if provider else None,
            fallback_used=fallback_used,
        )

    if status == "done" and entity_count == 0 and relation_count == 0:
        if "missing_tool_use" in codes or "schema_" in codes or "malformed" in codes:
            code = "MALFORMED_OUTPUT"
        elif "missing_api_key" in codes or "auth" in codes.lower():
            code = "QUOTA_OR_AUTH"
        elif "transport" in codes:
            code = "TRANSPORT_FAILURE"
        else:
            code = "EMPTY_CANDIDATES"
        return ExtractionFailureRecord(
            code=code,
            stage="extract",
            message=message or "empty_candidates",
            provider=str(provider) if provider else None,
            fallback_used=fallback_used,
        )

    if status == "done" and (entity_count + relation_count) > 0 and evidence_linked_count == 0:
        return ExtractionFailureRecord(
            code="MISSING_EVIDENCE",
            stage="evidence_linker",
            message=message or "candidates_without_evidence_path",
            provider=str(provider) if provider else None,
            fallback_used=fallback_used,
        )

    return None


__all__ = [
    "ExtractionFailureCode",
    "ExtractionFailureRecord",
    "classify_extraction_failure",
]
