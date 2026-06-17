# Formerly: src/arxiv_archive/graph_readiness_manifest.py

"""Route and candidate eligibility manifest synthesis.

This module combines automated graph-readiness diagnostics with independent
review findings.  It intentionally treats automated `ok_for_graph` as baseline
input, not final eligibility, because S11/T07 requires semantic review before
route or candidate eligibility can be claimed.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from research_graph.graph.readiness.core import to_redacted_dict

PAPER_ROUTE = "paper"
ROUTE_GRANULARITY = "route"
CANDIDATE_GRANULARITY = "candidate"
FALSE_CONFIDENCE_FINDING = "false_confidence_automated_ok_for_graph"
ELIGIBLE_FINDING_CODES = {
    "reviewed_metadata_eligible",
    "reviewed_statistical_methods_eligible",
    "reviewed_claim_candidate_eligible",
}


@dataclass(frozen=True)
class ManifestEntry:
    """One route or candidate eligibility decision."""

    paper_id: str
    route: str
    automated_state: str
    independent_review_verdict: str
    final_eligibility: str
    review_artifact: str | None = None
    excluded_routes: list[str] = field(default_factory=list)
    required_repairs: list[str] = field(default_factory=list)
    caveats: list[str] = field(default_factory=list)
    finding_codes: list[str] = field(default_factory=list)
    granularity: str = ROUTE_GRANULARITY
    entry_id: str | None = None
    parent_route: str | None = None
    chunk_id: str | None = None
    candidate_id: str | None = None
    source_artifact: str | None = None


@dataclass(frozen=True)
class ManifestResult:
    """In-memory manifest result."""

    scope: str
    entries: list[ManifestEntry]
    global_findings: list[dict[str, Any]]


def create_manifest(
    *,
    graph_summary_path: Path,
    review_events_path: Path,
    output_path: Path | None = None,
    scope: str = "prose_claims_only",
) -> dict[str, Any]:
    """Create and optionally write a route/candidate eligibility manifest."""
    graph_summary = json.loads(Path(graph_summary_path).read_text(encoding="utf-8"))
    review_events = _read_jsonl(review_events_path)
    result = synthesize_manifest(graph_summary, review_events, scope=scope)
    manifest = _manifest_to_dict(result, graph_summary_path, review_events_path)
    if output_path is not None:
        output_path = Path(output_path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(
            json.dumps(to_redacted_dict(manifest), indent=2, sort_keys=True),
            encoding="utf-8",
        )
    return manifest


def synthesize_manifest(
    graph_summary: dict[str, Any],
    review_events: list[dict[str, Any]],
    *,
    scope: str = "prose_claims_only",
) -> ManifestResult:
    """Synthesize route and candidate eligibility from automated and review events."""
    requested = [event for event in review_events if event.get("event") == "independent_review.requested"]
    findings = [event for event in review_events if event.get("event") == "independent_review.finding"]
    global_findings = [finding for finding in findings if finding.get("finding_code") == FALSE_CONFIDENCE_FINDING]
    route_findings = [finding for finding in findings if not _is_candidate_finding(finding)]
    candidate_findings = [finding for finding in findings if _is_candidate_finding(finding)]
    review_artifacts = {event["paper_id"]: event.get("review_artifact_path") for event in requested}
    routes_by_paper = {
        event["paper_id"]: sorted((event.get("routes") or {}).keys())
        for event in requested
    }

    entries: list[ManifestEntry] = []
    for paper in graph_summary.get("papers", []):
        paper_id = str(paper["paper_id"])
        automated_state = str(paper.get("state", "unknown"))
        reviewed_routes = routes_by_paper.get(paper_id)
        if not reviewed_routes:
            entries.append(
                ManifestEntry(
                    paper_id=paper_id,
                    route=PAPER_ROUTE,
                    automated_state=automated_state,
                    independent_review_verdict="NOT_REVIEWED",
                    final_eligibility="review_required",
                    required_repairs=["independent_review_required_before_route_eligibility"],
                )
            )
            continue

        review_routes = sorted({*reviewed_routes, *_finding_routes_for_paper(route_findings, paper_id=paper_id)})
        for route in review_routes:
            matching_route_findings = _matching_findings(route_findings, paper_id=paper_id, route=route)
            entries.append(
                _entry_for_route(
                    paper_id=paper_id,
                    route=route,
                    automated_state=automated_state,
                    review_artifact=review_artifacts.get(paper_id),
                    findings=matching_route_findings,
                    scope=scope,
                )
            )
        entries.extend(
            _entries_for_candidate_findings(
                candidate_findings,
                paper_id=paper_id,
                automated_state=automated_state,
                review_artifact=review_artifacts.get(paper_id),
            )
        )
    return ManifestResult(scope=scope, entries=entries, global_findings=global_findings)


def _entry_for_route(
    *,
    paper_id: str,
    route: str,
    automated_state: str,
    review_artifact: str | None,
    findings: list[dict[str, Any]],
    scope: str,
) -> ManifestEntry:
    blocking_findings = [finding for finding in findings if finding.get("severity") == "blocker"]
    repair_findings = [finding for finding in findings if finding.get("severity") == "repair_required"]
    warn_findings = [finding for finding in findings if finding.get("severity") == "warn"]
    eligible_findings = [finding for finding in findings if finding.get("finding_code") in ELIGIBLE_FINDING_CODES]
    finding_codes = [str(finding.get("finding_code")) for finding in findings]
    messages = [str(finding.get("finding")) for finding in findings]

    if route == "retrieval_only":
        base_eligibility = "eligible_with_caveat"
        base_caveats = ["retrieval_only_is_not_kg_fact_evidence"]
    elif scope == "prose_claims_only" and route not in {
        "claim_extraction",
        "method_extraction",
        "entity_candidate_extraction",
        "relation_extraction",
        "retrieval_only",
        "metadata_graph",
    }:
        base_eligibility = "route_excluded"
        base_caveats = [f"route_excluded_by_scope:{scope}"]
    else:
        base_eligibility = "eligible_with_caveat"
        base_caveats = ["independent_review_required_for_final_pass_claims"]

    if blocking_findings:
        final_eligibility = "blocked"
        independent_review_verdict = "BLOCKER"
    elif base_eligibility == "route_excluded":
        final_eligibility = "route_excluded"
        independent_review_verdict = "REPAIR" if repair_findings else "FLAG" if warn_findings else "PASS"
    elif repair_findings:
        final_eligibility = "repair_required"
        independent_review_verdict = "REPAIR"
    elif warn_findings:
        final_eligibility = "eligible_with_caveat"
        independent_review_verdict = "FLAG"
    elif eligible_findings:
        final_eligibility = "eligible"
        independent_review_verdict = "PASS"
    else:
        final_eligibility = base_eligibility
        independent_review_verdict = "FLAG" if base_eligibility == "eligible_with_caveat" else "PASS"

    required_repairs = messages if final_eligibility in {"repair_required", "blocked"} else []
    if final_eligibility == "route_excluded" and repair_findings:
        required_repairs = messages
    caveats = [*base_caveats]
    if final_eligibility == "eligible_with_caveat":
        caveats.extend(messages)
    excluded_routes = [route] if final_eligibility == "route_excluded" else []
    return ManifestEntry(
        paper_id=paper_id,
        route=route,
        automated_state=automated_state,
        independent_review_verdict=independent_review_verdict,
        final_eligibility=final_eligibility,
        review_artifact=review_artifact,
        excluded_routes=excluded_routes,
        required_repairs=required_repairs,
        caveats=[caveat for caveat in caveats if caveat],
        finding_codes=finding_codes,
        entry_id=f"route:{paper_id}:{route}",
        parent_route=route,
    )


def _entries_for_candidate_findings(
    findings: list[dict[str, Any]],
    *,
    paper_id: str,
    automated_state: str,
    review_artifact: str | None,
) -> list[ManifestEntry]:
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for finding in findings:
        finding_paper = str(finding.get("paper_id", ""))
        if finding_paper not in {paper_id, "*"}:
            continue
        route = str(finding.get("route", ""))
        candidate_key = _candidate_key(finding)
        if not route or not candidate_key:
            continue
        grouped.setdefault((route, candidate_key), []).append(finding)

    entries: list[ManifestEntry] = []
    for (route, candidate_key), candidate_group in sorted(grouped.items()):
        entries.append(
            _entry_for_candidate(
                paper_id=paper_id,
                route=route,
                candidate_key=candidate_key,
                automated_state=automated_state,
                review_artifact=review_artifact,
                findings=candidate_group,
            )
        )
    return entries


def _entry_for_candidate(
    *,
    paper_id: str,
    route: str,
    candidate_key: str,
    automated_state: str,
    review_artifact: str | None,
    findings: list[dict[str, Any]],
) -> ManifestEntry:
    blocking_findings = [finding for finding in findings if finding.get("severity") == "blocker"]
    repair_findings = [finding for finding in findings if finding.get("severity") == "repair_required"]
    warn_findings = [finding for finding in findings if finding.get("severity") == "warn"]
    eligible_findings = [finding for finding in findings if finding.get("finding_code") in ELIGIBLE_FINDING_CODES]
    finding_codes = [str(finding.get("finding_code")) for finding in findings]
    messages = [str(finding.get("finding")) for finding in findings if finding.get("finding")]

    if blocking_findings:
        final_eligibility = "blocked"
        independent_review_verdict = "BLOCKER"
    elif repair_findings:
        final_eligibility = "repair_required"
        independent_review_verdict = "REPAIR"
    elif warn_findings and not eligible_findings:
        final_eligibility = "eligible_with_caveat"
        independent_review_verdict = "FLAG"
    elif eligible_findings:
        final_eligibility = "eligible"
        independent_review_verdict = "PASS"
    else:
        final_eligibility = "review_required"
        independent_review_verdict = "NOT_REVIEWED"

    first = findings[0]
    chunk_id = _string_or_none(first.get("chunk_id"))
    candidate_id = _string_or_none(first.get("candidate_id"))
    source_artifact = _string_or_none(first.get("source_artifact") or first.get("source_artifact_path"))
    required_repairs = messages if final_eligibility in {"repair_required", "blocked"} else []
    caveats = messages if final_eligibility == "eligible_with_caveat" else []
    return ManifestEntry(
        paper_id=paper_id,
        route=route,
        automated_state=automated_state,
        independent_review_verdict=independent_review_verdict,
        final_eligibility=final_eligibility,
        review_artifact=review_artifact,
        required_repairs=required_repairs,
        caveats=caveats,
        finding_codes=finding_codes,
        granularity=CANDIDATE_GRANULARITY,
        entry_id=f"candidate:{paper_id}:{route}:{candidate_key}",
        parent_route=route,
        chunk_id=chunk_id,
        candidate_id=candidate_id,
        source_artifact=source_artifact,
    )


def _finding_routes_for_paper(findings: list[dict[str, Any]], *, paper_id: str) -> set[str]:
    routes: set[str] = set()
    for finding in findings:
        finding_route = str(finding.get("route", ""))
        finding_paper = str(finding.get("paper_id", ""))
        if finding.get("finding_code") == FALSE_CONFIDENCE_FINDING:
            continue
        if finding_route in {"", "all"}:
            continue
        if finding_paper in {"*", paper_id}:
            routes.add(finding_route)
    return routes


def _matching_findings(
    findings: list[dict[str, Any]],
    *,
    paper_id: str,
    route: str,
) -> list[dict[str, Any]]:
    matched: list[dict[str, Any]] = []
    for finding in findings:
        if finding.get("finding_code") == FALSE_CONFIDENCE_FINDING:
            continue
        finding_paper = str(finding.get("paper_id", ""))
        finding_route = str(finding.get("route", ""))
        paper_matches = finding_paper in {"*", paper_id}
        route_matches = finding_route in {"all", route}
        if paper_matches and route_matches:
            matched.append(finding)
    return matched


def _is_candidate_finding(finding: dict[str, Any]) -> bool:
    return bool(_candidate_key(finding))


def _candidate_key(finding: dict[str, Any]) -> str:
    candidate_id = _string_or_none(finding.get("candidate_id"))
    if candidate_id:
        return candidate_id
    chunk_id = _string_or_none(finding.get("chunk_id"))
    return chunk_id or ""


def _string_or_none(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value)
    return text if text else None


def _manifest_to_dict(
    result: ManifestResult,
    graph_summary_path: Path,
    review_events_path: Path,
) -> dict[str, Any]:
    counts: dict[str, int] = {}
    counts_by_granularity: dict[str, dict[str, int]] = {}
    for entry in result.entries:
        counts[entry.final_eligibility] = counts.get(entry.final_eligibility, 0) + 1
        granularity_counts = counts_by_granularity.setdefault(entry.granularity, {})
        granularity_counts[entry.final_eligibility] = granularity_counts.get(entry.final_eligibility, 0) + 1
    return {
        "schema_version": "s05-eligibility-manifest.v2",
        "scope": result.scope,
        "graph_summary_path": str(graph_summary_path),
        "review_events_path": str(review_events_path),
        "global_findings": result.global_findings,
        "counts": counts,
        "counts_by_granularity": counts_by_granularity,
        "entries": [entry.__dict__ for entry in result.entries],
    }


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        if line.strip():
            events.append(json.loads(line))
    return events


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Create route/candidate eligibility manifest.")
    parser.add_argument("--graph-summary", required=True, type=Path)
    parser.add_argument("--review-events", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--scope", default="prose_claims_only")
    args = parser.parse_args(argv)
    manifest = create_manifest(
        graph_summary_path=args.graph_summary,
        review_events_path=args.review_events,
        output_path=args.output,
        scope=args.scope,
    )
    sys.stdout.write(json.dumps(to_redacted_dict(manifest["counts"]), indent=2, sort_keys=True))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
