# Formerly: src/arxiv_archive/graph_readiness_extraction_gate.py

"""Route-gated extraction decision for S05.

This module is deliberately conservative: it decides whether scientific
extraction may run from the refreshed eligibility manifest and emits diagnostic
artifacts. It does not fabricate low-confidence scientific facts when only
caveated routes are available.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.infrastructure.graph.readiness.core import to_redacted_dict

TRUSTED_ELIGIBILITY = "eligible"
CAVEATED_ELIGIBILITY = "eligible_with_caveat"
EXTRACTION_ROUTES = {
    "claim_extraction",
    "method_extraction",
    "entity_candidate_extraction",
    "relation_extraction",
    "metadata_graph",
}
EXCLUDED_EXTRACTION_ROUTES = {
    "retrieval_only",
    "table_extraction",
    "citation_graph",
    "figure_evidence",
}


@dataclass(frozen=True)
class ExtractionGateResult:
    """Decision emitted by the extraction route gate."""

    extraction_attempted: bool
    blocked_reason: str | None
    trusted_entries: list[dict[str, Any]]
    caveated_entries: list[dict[str, Any]]
    excluded_entries: list[dict[str, Any]]
    skipped_entries: list[dict[str, Any]]
    claim_drafts: list[dict[str, Any]]


def run_extraction_gate(
    *,
    manifest_path: Path,
    output_dir: Path,
    require_trusted: bool = True,
) -> dict[str, Any]:
    """Run the S05 extraction gate and write summary/events diagnostics."""
    manifest = json.loads(Path(manifest_path).read_text(encoding="utf-8"))
    result = decide_extraction(manifest, require_trusted=require_trusted)
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    summary = _summary_payload(manifest, result)
    summary_path = output_dir / "extraction-route-summary.json"
    events_path = output_dir / "extraction-events.jsonl"
    summary_path.write_text(
        json.dumps(to_redacted_dict(summary), indent=2, sort_keys=True), encoding="utf-8"
    )
    events_path.write_text("", encoding="utf-8")
    _append_event(events_path, {"event": "extraction_gate.summary", **summary})
    for entry in result.trusted_entries:
        _append_event(
            events_path, {"event": "extraction_gate.trusted_route", **_entry_event(entry)}
        )
    for entry in result.caveated_entries:
        _append_event(
            events_path, {"event": "extraction_gate.caveated_route", **_entry_event(entry)}
        )
    for entry in result.excluded_entries:
        _append_event(
            events_path, {"event": "extraction_gate.excluded_route", **_entry_event(entry)}
        )
    for entry in result.skipped_entries:
        _append_event(
            events_path, {"event": "extraction_gate.skipped_route", **_entry_event(entry)}
        )
    for draft in result.claim_drafts:
        _append_event(
            events_path,
            {"event": "extraction_gate.claim_draft", **draft, "raw_text_included": False},
        )
    return summary


def decide_extraction(
    manifest: dict[str, Any], *, require_trusted: bool = True
) -> ExtractionGateResult:
    """Decide whether extraction can run from manifest entries."""
    trusted_entries: list[dict[str, Any]] = []
    caveated_entries: list[dict[str, Any]] = []
    excluded_entries: list[dict[str, Any]] = []
    skipped_entries: list[dict[str, Any]] = []
    claim_drafts: list[dict[str, Any]] = []
    for raw_entry in manifest.get("entries", []):
        entry = dict(raw_entry)
        route = str(entry.get("route"))
        final_eligibility = str(entry.get("final_eligibility"))
        if route in EXCLUDED_EXTRACTION_ROUTES or final_eligibility == "route_excluded":
            excluded_entries.append(entry)
        elif route in EXTRACTION_ROUTES and final_eligibility == TRUSTED_ELIGIBILITY:
            trusted_entries.append(entry)
            if _is_trusted_claim_candidate(entry):
                claim_drafts.append(_claim_draft(entry))
        elif route in EXTRACTION_ROUTES and final_eligibility == CAVEATED_ELIGIBILITY:
            caveated_entries.append(entry)
        else:
            skipped_entries.append(entry)

    if trusted_entries:
        return ExtractionGateResult(
            extraction_attempted=True,
            blocked_reason=None,
            trusted_entries=trusted_entries,
            caveated_entries=caveated_entries,
            excluded_entries=excluded_entries,
            skipped_entries=skipped_entries,
            claim_drafts=claim_drafts,
        )
    if caveated_entries and not require_trusted:
        return ExtractionGateResult(
            extraction_attempted=True,
            blocked_reason=None,
            trusted_entries=trusted_entries,
            caveated_entries=caveated_entries,
            excluded_entries=excluded_entries,
            skipped_entries=skipped_entries,
            claim_drafts=claim_drafts,
        )
    return ExtractionGateResult(
        extraction_attempted=False,
        blocked_reason="no_trusted_extraction_routes_after_review",
        trusted_entries=trusted_entries,
        caveated_entries=caveated_entries,
        excluded_entries=excluded_entries,
        skipped_entries=skipped_entries,
        claim_drafts=claim_drafts,
    )


def _summary_payload(manifest: dict[str, Any], result: ExtractionGateResult) -> dict[str, Any]:
    return {
        "schema_version": "s05-extraction-route-summary.v1",
        "manifest_schema_version": manifest.get("schema_version"),
        "manifest_scope": manifest.get("scope"),
        "extraction_attempted": result.extraction_attempted,
        "blocked_reason": result.blocked_reason,
        "counts": {
            "trusted_routes": len(result.trusted_entries),
            "trusted_candidates": sum(
                1 for entry in result.trusted_entries if entry.get("granularity") == "candidate"
            ),
            "caveated_routes": len(result.caveated_entries),
            "excluded_routes": len(result.excluded_entries),
            "skipped_routes": len(result.skipped_entries),
            "claims": len(result.claim_drafts),
            "entities": 0,
            "relations": 0,
        },
        "eligible_routes": [_safe_entry(entry) for entry in result.trusted_entries],
        "claim_drafts": list(result.claim_drafts),
        "caveated_routes": [_safe_entry(entry) for entry in result.caveated_entries],
        "excluded_routes": [_safe_entry(entry) for entry in result.excluded_entries],
        "skipped_routes": [_safe_entry(entry) for entry in result.skipped_entries],
        "safety_note": (
            "No scientific facts are emitted unless at least one route or candidate has final_eligibility=eligible. "
            "eligible_with_caveat routes remain diagnostic. Candidate claim drafts are redacted source-spanned "
            "drafts only and are not persisted KG facts."
        ),
    }


def _safe_entry(entry: dict[str, Any]) -> dict[str, Any]:
    return {
        "paper_id": entry.get("paper_id"),
        "route": entry.get("route"),
        "granularity": entry.get("granularity", "route"),
        "entry_id": entry.get("entry_id"),
        "chunk_id": entry.get("chunk_id"),
        "candidate_id": entry.get("candidate_id"),
        "parent_route": entry.get("parent_route"),
        "source_artifact": entry.get("source_artifact"),
        "final_eligibility": entry.get("final_eligibility"),
        "independent_review_verdict": entry.get("independent_review_verdict"),
        "finding_codes": list(entry.get("finding_codes", [])),
        "caveats": list(entry.get("caveats", [])),
        "required_repairs": list(entry.get("required_repairs", [])),
    }


def _is_trusted_claim_candidate(entry: dict[str, Any]) -> bool:
    return (
        entry.get("granularity") == "candidate"
        and entry.get("route") == "claim_extraction"
        and entry.get("final_eligibility") == TRUSTED_ELIGIBILITY
    )


def _claim_draft(entry: dict[str, Any]) -> dict[str, Any]:
    return {
        "paper_id": entry.get("paper_id"),
        "route": entry.get("route"),
        "chunk_id": entry.get("chunk_id"),
        "candidate_id": entry.get("candidate_id"),
        "entry_id": entry.get("entry_id"),
        "source_artifact": entry.get("source_artifact"),
        "source_span_traceable": True,
        "finding_codes": list(entry.get("finding_codes", [])),
        "claim_text_included": False,
        "persisted": False,
    }


def _entry_event(entry: dict[str, Any]) -> dict[str, Any]:
    payload = _safe_entry(entry)
    payload["raw_text_included"] = False
    return payload


def _append_event(path: Path, payload: dict[str, Any]) -> None:
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(to_redacted_dict(payload), sort_keys=True) + "\n")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run S05 extraction route gate.")
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument(
        "--allow-caveated-extraction",
        action="store_true",
        help="Allow eligible_with_caveat routes to proceed. Disabled by default for S05 quality validation.",
    )
    args = parser.parse_args(argv)
    summary = run_extraction_gate(
        manifest_path=args.manifest,
        output_dir=args.output_dir,
        require_trusted=not args.allow_caveated_extraction,
    )
    sys.stdout.write(json.dumps(to_redacted_dict(summary["counts"]), indent=2, sort_keys=True))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
