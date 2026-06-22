# Formerly: src/arxiv_archive/graph_readiness_persistence.py

"""Trusted candidate claim selector for graph-readiness persistence validation.

This module is deliberately read-only. It joins the S13 candidate-level
eligibility manifest with the extraction gate summary and selects only the
reviewed claim candidates that are safe for the S06 isolated persistence
validation. It does not write LadybugDB, run extraction, or include raw paper
text in diagnostics.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Any

from research_graph.infrastructure.graph.readiness.core import to_redacted_dict

TRUSTED_ELIGIBILITY = "eligible"
TRUSTED_ROUTE = "claim_extraction"
TRUSTED_GRANULARITY = "candidate"
TRUSTED_FINDING_CODES = frozenset(
    {
        "reviewed_claim_candidate_eligible",
        "atomic_claim_candidate_safe_to_promote",
    }
)


@dataclass(frozen=True)
class TrustedCandidateClaim:
    """A reviewed candidate claim allowed into S06 validation persistence."""

    paper_id: str
    route: str
    candidate_id: str
    chunk_id: str
    entry_id: str
    source_artifact: str | None
    finding_codes: list[str]
    independent_review_verdict: str
    final_eligibility: str
    claim_draft_id: str
    persisted: bool = False
    raw_text_included: bool = False
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class PersistenceRefusal:
    """Structured reason an entry or draft is not allowed for persistence."""

    paper_id: str | None
    route: str | None
    candidate_id: str | None
    chunk_id: str | None
    reason: str
    granularity: str | None = None
    final_eligibility: str | None = None
    entry_id: str | None = None


@dataclass(frozen=True)
class SelectionResult:
    """Allowed trusted candidates and refused entries/drafts."""

    trusted_claims: list[TrustedCandidateClaim]
    refusals: list[PersistenceRefusal]

    @property
    def counts(self) -> dict[str, int]:
        by_reason: dict[str, int] = {}
        for refusal in self.refusals:
            by_reason[refusal.reason] = by_reason.get(refusal.reason, 0) + 1
        return {
            "trusted_candidate_claims": len(self.trusted_claims),
            "refusals": len(self.refusals),
            **{f"refused_{reason}": count for reason, count in sorted(by_reason.items())},
        }


@dataclass(frozen=True)
class PersistenceArtifactResult:
    """Files and summary produced by isolated validation persistence."""

    selection: SelectionResult
    claims_path: Path
    summary_path: Path
    summary: dict[str, Any]


def select_trusted_candidate_claims(
    *,
    manifest: dict[str, Any],
    extraction_summary: dict[str, Any],
) -> SelectionResult:
    """Select only trusted reviewed candidate claim drafts for S06 validation persistence."""
    trusted_claims: list[TrustedCandidateClaim] = []
    refusals: list[PersistenceRefusal] = []

    claim_drafts_by_key = _claim_drafts_by_key(extraction_summary.get("claim_drafts", []))
    matched_draft_keys: set[tuple[str, str, str]] = set()
    for raw_entry in manifest.get("entries", []):
        entry = dict(raw_entry)
        refusal_reason = _entry_refusal_reason(entry)
        if refusal_reason is not None:
            refusals.append(_refusal_from_entry(entry, refusal_reason))
            continue

        key = _entry_key(entry)
        draft = claim_drafts_by_key.get(key)
        if draft is None:
            refusals.append(_refusal_from_entry(entry, "missing_matching_claim_draft"))
            continue

        finding_codes = [str(code) for code in entry.get("finding_codes", [])]
        trusted_claims.append(
            TrustedCandidateClaim(
                paper_id=str(entry["paper_id"]),
                route=str(entry["route"]),
                candidate_id=str(entry.get("candidate_id") or entry.get("chunk_id")),
                chunk_id=str(entry.get("chunk_id") or entry.get("candidate_id")),
                entry_id=str(entry.get("entry_id") or draft.get("entry_id") or ":".join(key)),
                source_artifact=_source_artifact_for_claim(
                    entry=entry,
                    draft=draft,
                    paper_id=str(entry["paper_id"]),
                ),
                finding_codes=finding_codes,
                independent_review_verdict=str(entry.get("independent_review_verdict", "PASS")),
                final_eligibility=str(entry.get("final_eligibility")),
                claim_draft_id=str(draft.get("entry_id") or ":".join(key)),
                provenance={
                    "manifest_schema_version": str(manifest.get("schema_version", "unknown")),
                    "extraction_summary_schema_version": str(extraction_summary.get("schema_version", "unknown")),
                    "selection_rule": "s06_trusted_candidate_claim_v1",
                },
            )
        )
        matched_draft_keys.add(key)

    for key, draft in sorted(claim_drafts_by_key.items()):
        if key in matched_draft_keys:
            continue
        refusals.append(
            PersistenceRefusal(
                paper_id=_string_or_none(draft.get("paper_id")),
                route=_string_or_none(draft.get("route")),
                candidate_id=_string_or_none(draft.get("candidate_id")),
                chunk_id=_string_or_none(draft.get("chunk_id")),
                entry_id=_string_or_none(draft.get("entry_id")),
                reason="claim_draft_without_trusted_manifest_entry",
            )
        )

    return SelectionResult(trusted_claims=trusted_claims, refusals=refusals)


def load_and_select(manifest_path: Path, extraction_summary_path: Path) -> SelectionResult:
    """Load manifest and extraction summary from disk and select trusted candidate claims."""
    manifest = json.loads(Path(manifest_path).read_text(encoding="utf-8"))
    extraction_summary = json.loads(Path(extraction_summary_path).read_text(encoding="utf-8"))
    return select_trusted_candidate_claims(manifest=manifest, extraction_summary=extraction_summary)


def persist_validation_subset(
    *,
    manifest_path: Path,
    extraction_summary_path: Path,
    output_dir: Path,
) -> PersistenceArtifactResult:
    """Write isolated validation persistence artifacts for trusted candidate claims."""
    selection = load_and_select(manifest_path, extraction_summary_path)
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    claims_path = output_dir / "persisted-candidate-claims.jsonl"
    summary_path = output_dir / "persistence-summary.json"

    persisted_claims = [replace(claim, persisted=True) for claim in selection.trusted_claims]
    claims_path.write_text(
        "".join(json.dumps(to_redacted_dict(_persisted_claim_payload(claim)), sort_keys=True) + "\n" for claim in persisted_claims),
        encoding="utf-8",
    )
    summary = _persistence_summary(selection=selection, persisted_claims=persisted_claims, claims_path=claims_path)
    summary_path.write_text(json.dumps(to_redacted_dict(summary), indent=2, sort_keys=True), encoding="utf-8")
    return PersistenceArtifactResult(selection=selection, claims_path=claims_path, summary_path=summary_path, summary=summary)


def selection_to_dict(result: SelectionResult) -> dict[str, Any]:
    """Serialize selection result without raw text."""
    return {
        "schema_version": "s06-trusted-candidate-selection.v1",
        "counts": result.counts,
        "trusted_claims": [claim.__dict__ for claim in result.trusted_claims],
        "refusals": [refusal.__dict__ for refusal in result.refusals],
        "raw_text_included": False,
    }


def write_refusal_evidence(*, selection: SelectionResult, output_path: Path) -> dict[str, Any]:
    """Write structured refusal evidence grouped by refusal reason."""
    output_path = Path(output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    by_reason: dict[str, list[dict[str, Any]]] = {}
    for refusal in selection.refusals:
        by_reason.setdefault(refusal.reason, []).append(refusal.__dict__)
    payload = {
        "schema_version": "s06-persistence-refusals.v1",
        "refused_count": len(selection.refusals),
        "persisted_count": len(selection.trusted_claims),
        "refusal_counts": {reason: len(items) for reason, items in sorted(by_reason.items())},
        "refusals_by_reason": dict(sorted(by_reason.items())),
        "raw_text_included": False,
        "embeddings_included": False,
    }
    output_path.write_text(json.dumps(to_redacted_dict(payload), indent=2, sort_keys=True), encoding="utf-8")
    return payload


def _persistence_summary(
    *,
    selection: SelectionResult,
    persisted_claims: list[TrustedCandidateClaim],
    claims_path: Path,
) -> dict[str, Any]:
    refusal_counts: dict[str, int] = {}
    for refusal in selection.refusals:
        refusal_counts[refusal.reason] = refusal_counts.get(refusal.reason, 0) + 1
    return {
        "schema_version": "s06-validation-persistence-summary.v1",
        "persisted_scope": "validation_subset",
        "persisted_count": len(persisted_claims),
        "selected_count": len(selection.trusted_claims),
        "refused_count": len(selection.refusals),
        "refusal_counts": refusal_counts,
        "claims_path": str(claims_path),
        "raw_text_included": False,
        "embeddings_included": False,
        "kg_persistence_attempted": False,
        "ladybugdb_written": False,
        "allowed_entry_rule": "granularity=candidate route=claim_extraction final_eligibility=eligible matching_claim_draft explicit_trusted_review_finding",
    }


def _persisted_claim_payload(claim: TrustedCandidateClaim) -> dict[str, Any]:
    return {
        "schema_version": "s06-persisted-candidate-claim.v1",
        "persisted_scope": "validation_subset",
        "paper_id": claim.paper_id,
        "route": claim.route,
        "candidate_id": claim.candidate_id,
        "chunk_id": claim.chunk_id,
        "entry_id": claim.entry_id,
        "source_artifact": claim.source_artifact,
        "finding_codes": list(claim.finding_codes),
        "independent_review_verdict": claim.independent_review_verdict,
        "final_eligibility": claim.final_eligibility,
        "claim_draft_id": claim.claim_draft_id,
        "persisted": claim.persisted,
        "raw_text_included": False,
        "claim_text_included": False,
        "embeddings_included": False,
        "provenance": dict(claim.provenance),
    }


def _source_artifact_for_claim(*, entry: dict[str, Any], draft: dict[str, Any], paper_id: str) -> str:
    """Return a redacted deterministic source artifact identifier without reading raw text."""
    explicit = _string_or_none(entry.get("source_artifact") or draft.get("source_artifact"))
    if explicit is not None:
        return explicit
    return f"normalized_markdown:{paper_id}"


def _entry_refusal_reason(entry: dict[str, Any]) -> str | None:
    if entry.get("granularity") != TRUSTED_GRANULARITY:
        return "not_candidate_granularity"
    if entry.get("route") != TRUSTED_ROUTE:
        return "not_claim_extraction_route"
    if entry.get("final_eligibility") != TRUSTED_ELIGIBILITY:
        return f"final_eligibility_{entry.get('final_eligibility', 'unknown')}"
    if not ({str(code) for code in entry.get("finding_codes", [])} & TRUSTED_FINDING_CODES):
        return "missing_explicit_trusted_review_finding"
    if not (entry.get("candidate_id") or entry.get("chunk_id")):
        return "missing_candidate_identifier"
    return None


def _entry_key(entry: dict[str, Any]) -> tuple[str, str, str]:
    paper_id = str(entry.get("paper_id"))
    route = str(entry.get("route"))
    candidate_id = str(entry.get("candidate_id") or entry.get("chunk_id"))
    return paper_id, route, candidate_id


def _draft_key(draft: dict[str, Any]) -> tuple[str, str, str]:
    paper_id = str(draft.get("paper_id"))
    route = str(draft.get("route"))
    candidate_id = str(draft.get("candidate_id") or draft.get("chunk_id"))
    return paper_id, route, candidate_id


def _claim_drafts_by_key(raw_drafts: Any) -> dict[tuple[str, str, str], dict[str, Any]]:
    drafts: dict[tuple[str, str, str], dict[str, Any]] = {}
    if not isinstance(raw_drafts, list):
        return drafts
    for raw_draft in raw_drafts:
        if not isinstance(raw_draft, dict):
            continue
        draft = dict(raw_draft)
        if draft.get("route") != TRUSTED_ROUTE:
            continue
        drafts[_draft_key(draft)] = draft
    return drafts


def _refusal_from_entry(entry: dict[str, Any], reason: str) -> PersistenceRefusal:
    return PersistenceRefusal(
        paper_id=_string_or_none(entry.get("paper_id")),
        route=_string_or_none(entry.get("route")),
        candidate_id=_string_or_none(entry.get("candidate_id")),
        chunk_id=_string_or_none(entry.get("chunk_id")),
        granularity=_string_or_none(entry.get("granularity")),
        final_eligibility=_string_or_none(entry.get("final_eligibility")),
        entry_id=_string_or_none(entry.get("entry_id")),
        reason=reason,
    )


def _string_or_none(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value)
    return text if text else None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Select trusted candidate claim drafts for S06 persistence validation.")
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--extraction-summary", required=True, type=Path)
    parser.add_argument("--output", required=False, type=Path)
    parser.add_argument("--persist-output-dir", required=False, type=Path)
    parser.add_argument("--refusals-output", required=False, type=Path)
    args = parser.parse_args(argv)
    if args.persist_output_dir is not None:
        persisted = persist_validation_subset(
            manifest_path=args.manifest,
            extraction_summary_path=args.extraction_summary,
            output_dir=args.persist_output_dir,
        )
        sys.stdout.write(json.dumps(to_redacted_dict(persisted.summary), indent=2, sort_keys=True))
        sys.stdout.write("\n")
        return 0

    result = load_and_select(args.manifest, args.extraction_summary)
    if args.refusals_output is not None:
        refusal_payload = write_refusal_evidence(selection=result, output_path=args.refusals_output)
        sys.stdout.write(json.dumps(to_redacted_dict(refusal_payload["refusal_counts"]), indent=2, sort_keys=True))
        sys.stdout.write("\n")
        return 0

    payload = to_redacted_dict(selection_to_dict(result))
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    sys.stdout.write(json.dumps(payload["counts"], indent=2, sort_keys=True))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
