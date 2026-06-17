# Formerly: src/arxiv_archive/universal_kb_sidecar_boundary.py

"""Adaptix boundary adapter for Universal KB sidecar candidates.

Adaptix is used here only as an anti-corruption mapping boundary for external
sidecar JSON. Semantic safety remains in Universal KB domain contracts.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from adaptix import Retort

from arxiv_archive.universal_kb_contracts import CandidatePacket


@dataclass(frozen=True, slots=True)
class BoundaryDiagnostic:
    code: str
    path: str
    message: str


class BoundaryMappingError(ValueError):
    """Raised when external sidecar JSON cannot be mapped into boundary DTOs."""

    def __init__(self, diagnostics: list[BoundaryDiagnostic]) -> None:
        super().__init__(diagnostics[0].message if diagnostics else "sidecar mapping failed")
        self.diagnostics = diagnostics


class BoundaryDomainInvariantError(ValueError):
    """Raised when mapped DTOs violate Universal KB domain invariants."""

    def __init__(self, diagnostics: list[BoundaryDiagnostic]) -> None:
        super().__init__(diagnostics[0].message if diagnostics else "candidate domain invariant failed")
        self.diagnostics = diagnostics


@dataclass(frozen=True, slots=True)
class _ExternalCandidate:
    id: str
    type: str
    evidence_refs: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class _ExternalReview:
    state: str = "pending"


@dataclass(frozen=True, slots=True)
class _ExternalSource:
    sidecar: str
    version: str


@dataclass(frozen=True, slots=True)
class _ExternalSidecarCandidate:
    candidate: _ExternalCandidate
    review: _ExternalReview
    source: _ExternalSource


_RETORT = Retort()


def candidate_packet_from_sidecar_json(payload: dict[str, Any]) -> CandidatePacket:
    """Map external sidecar JSON into a safe candidate packet.

    Mapping/schema errors are boundary errors. Candidate authority or safety
    violations are domain invariant errors raised after constructing the
    `CandidatePacket`.
    """
    try:
        mapped = _RETORT.load(payload, _ExternalSidecarCandidate)
    except Exception as exc:  # Adaptix exposes several loader exception types.
        raise BoundaryMappingError(
            [
                BoundaryDiagnostic(
                    code="sidecar_mapping_error",
                    path="/",
                    message="external sidecar JSON could not be mapped to candidate input",
                )
            ]
        ) from exc

    try:
        return CandidatePacket(
            candidate_id=mapped.candidate.id,
            evidence_refs=mapped.candidate.evidence_refs,
            candidate_type=mapped.candidate.type,
            review_state=mapped.review.state,
        )
    except ValueError as exc:
        raise BoundaryDomainInvariantError(
            [
                BoundaryDiagnostic(
                    code="candidate_domain_invariant_error",
                    path="/candidate",
                    message="mapped candidate violates Universal KB candidate invariants",
                )
            ]
        ) from exc
