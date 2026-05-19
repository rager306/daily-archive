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
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from arxiv_archive.graph_readiness import to_redacted_dict

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
                source_artifact=_string_or_none(entry.get("source_artifact") or draft.get("source_artifact")),
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


def selection_to_dict(result: SelectionResult) -> dict[str, Any]:
    """Serialize selection result without raw text."""
    return {
        "schema_version": "s06-trusted-candidate-selection.v1",
        "counts": result.counts,
        "trusted_claims": [claim.__dict__ for claim in result.trusted_claims],
        "refusals": [refusal.__dict__ for refusal in result.refusals],
        "raw_text_included": False,
    }


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
    args = parser.parse_args(argv)
    result = load_and_select(args.manifest, args.extraction_summary)
    payload = to_redacted_dict(selection_to_dict(result))
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    sys.stdout.write(json.dumps(payload["counts"], indent=2, sort_keys=True))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
