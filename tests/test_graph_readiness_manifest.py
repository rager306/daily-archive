from __future__ import annotations

import json
from pathlib import Path

from research_graph.infrastructure.graph.readiness.manifest import (
    create_manifest,
    synthesize_manifest,
)


def _summary() -> dict[str, object]:
    return {
        "paper_count": 3,
        "papers": [
            {"paper_id": "p1", "state": "ok_for_graph", "trust_level": "trusted_graph"},
            {"paper_id": "p2", "state": "ok_for_graph", "trust_level": "trusted_graph"},
            {"paper_id": "p3", "state": "ok_for_graph", "trust_level": "trusted_graph"},
        ],
    }


def _events() -> list[dict[str, object]]:
    return [
        {
            "event": "independent_review.requested",
            "paper_id": "p1",
            "review_artifact_path": "review/p1-review.md",
            "routes": {
                "claim_extraction": {"eligible": 2, "blocked": 0},
                "retrieval_only": {"eligible": 2, "blocked": 0},
                "table_extraction": {"eligible": 1, "blocked": 0},
            },
        },
        {
            "event": "independent_review.requested",
            "paper_id": "p2",
            "review_artifact_path": "review/p2-review.md",
            "routes": {
                "claim_extraction": {"eligible": 2, "blocked": 0},
                "citation_graph": {"eligible": 1, "blocked": 0},
            },
        },
        {
            "event": "independent_review.finding",
            "finding_code": "false_confidence_automated_ok_for_graph",
            "paper_id": "*",
            "route": "all",
            "severity": "repair_required",
            "finding": "Automated ok_for_graph is structural only.",
        },
        {
            "event": "independent_review.finding",
            "finding_code": "oversized_claim_chunks",
            "paper_id": "*",
            "route": "claim_extraction",
            "severity": "repair_required",
            "finding": "Split oversized claim chunks.",
        },
        {
            "event": "independent_review.finding",
            "finding_code": "table_lineage_missing",
            "paper_id": "p1",
            "route": "table_extraction",
            "severity": "warn",
            "finding": "Table lineage missing.",
        },
        {
            "event": "independent_review.finding",
            "finding_code": "coarse_reference_chunks",
            "paper_id": "*",
            "route": "citation_graph",
            "severity": "warn",
            "finding": "Citation chunks are coarse.",
        },
    ]


def _entry(manifest: dict[str, object], paper_id: str, route: str) -> dict[str, object]:
    entries = manifest["entries"]
    assert isinstance(entries, list)
    for item in entries:
        assert isinstance(item, dict)
        if item["paper_id"] == paper_id and item["route"] == route and item.get("granularity") == "route":
            return item
    raise AssertionError(f"missing route entry {paper_id} {route}")


def _candidate_entry(manifest: dict[str, object], paper_id: str, route: str, candidate_id: str) -> dict[str, object]:
    entries = manifest["entries"]
    assert isinstance(entries, list)
    for item in entries:
        assert isinstance(item, dict)
        if (
            item["paper_id"] == paper_id
            and item["route"] == route
            and item.get("granularity") == "candidate"
            and item.get("candidate_id") == candidate_id
        ):
            return item
    raise AssertionError(f"missing candidate entry {paper_id} {route} {candidate_id}")


def test_manifest_applies_repair_findings_to_matching_routes() -> None:
    result = synthesize_manifest(_summary(), _events())
    manifest = {
        "entries": [entry.__dict__ for entry in result.entries],
        "global_findings": result.global_findings,
    }

    p1_claim = _entry(manifest, "p1", "claim_extraction")
    p2_claim = _entry(manifest, "p2", "claim_extraction")

    assert p1_claim["final_eligibility"] == "repair_required"
    assert p2_claim["final_eligibility"] == "repair_required"
    assert "oversized_claim_chunks" in p1_claim["finding_codes"]


def test_manifest_flags_warn_findings_without_blocking_route() -> None:
    result = synthesize_manifest(_summary(), _events())
    manifest = {"entries": [entry.__dict__ for entry in result.entries]}

    table = _entry(manifest, "p1", "table_extraction")
    citation = _entry(manifest, "p2", "citation_graph")

    assert table["final_eligibility"] == "route_excluded"
    assert "table_extraction" in table["excluded_routes"]
    assert citation["final_eligibility"] == "route_excluded"
    assert "coarse_reference_chunks" in citation["finding_codes"]


def test_manifest_marks_unreviewed_papers_review_required() -> None:
    result = synthesize_manifest(_summary(), _events())
    manifest = {"entries": [entry.__dict__ for entry in result.entries]}

    p3 = _entry(manifest, "p3", "paper")

    assert p3["independent_review_verdict"] == "NOT_REVIEWED"
    assert p3["final_eligibility"] == "review_required"
    assert p3["required_repairs"] == ["independent_review_required_before_route_eligibility"]


def test_false_confidence_finding_is_preserved_as_global_not_route_blocker() -> None:
    result = synthesize_manifest(_summary(), _events())

    assert result.global_findings
    assert result.global_findings[0]["finding_code"] == "false_confidence_automated_ok_for_graph"
    retrieval_entry = next(
        entry
        for entry in result.entries
        if entry.paper_id == "p1" and entry.route == "retrieval_only" and entry.granularity == "route"
    )
    assert retrieval_entry.final_eligibility == "eligible_with_caveat"


def test_manifest_adds_explicit_entries_for_finding_only_excluded_routes() -> None:
    result = synthesize_manifest(_summary(), _events())
    manifest = {"entries": [entry.__dict__ for entry in result.entries]}

    p1_citation = _entry(manifest, "p1", "citation_graph")

    assert p1_citation["final_eligibility"] == "route_excluded"
    assert "citation_graph" in p1_citation["excluded_routes"]
    assert "coarse_reference_chunks" in p1_citation["finding_codes"]


def test_manifest_promotes_only_explicit_reviewed_eligible_findings() -> None:
    events = [
        {
            "event": "independent_review.requested",
            "paper_id": "p1",
            "review_artifact_path": "review/p1-review.md",
            "routes": {
                "metadata_graph": {"eligible": 1, "blocked": 0},
                "method_extraction": {"eligible": 1, "blocked": 0},
                "claim_extraction": {"eligible": 2, "blocked": 0},
                "table_extraction": {"eligible": 1, "blocked": 0},
            },
        },
        {
            "event": "independent_review.finding",
            "finding_code": "reviewed_metadata_eligible",
            "paper_id": "p1",
            "route": "metadata_graph",
            "severity": "info",
            "finding": "Reviewed metadata is eligible.",
        },
        {
            "event": "independent_review.finding",
            "finding_code": "reviewed_statistical_methods_eligible",
            "paper_id": "p1",
            "route": "method_extraction",
            "severity": "info",
            "finding": "Reviewed methods are eligible.",
        },
        {
            "event": "independent_review.finding",
            "finding_code": "sample_scoped_claim_promotion",
            "paper_id": "p1",
            "route": "claim_extraction",
            "severity": "warn",
            "finding": "Claims are sample-scoped only.",
        },
        {
            "event": "independent_review.finding",
            "finding_code": "table_lineage_not_reviewed",
            "paper_id": "*",
            "route": "table_extraction",
            "severity": "repair_required",
            "finding": "Table lineage was not reviewed.",
        },
    ]
    result = synthesize_manifest(
        {"paper_count": 1, "papers": [{"paper_id": "p1", "state": "ok_for_graph"}]},
        events,
    )
    manifest = {"entries": [entry.__dict__ for entry in result.entries]}

    assert _entry(manifest, "p1", "metadata_graph")["final_eligibility"] == "eligible"
    assert _entry(manifest, "p1", "method_extraction")["final_eligibility"] == "eligible"
    assert _entry(manifest, "p1", "claim_extraction")["final_eligibility"] == "eligible_with_caveat"
    table = _entry(manifest, "p1", "table_extraction")
    assert table["final_eligibility"] == "route_excluded"
    assert table["independent_review_verdict"] == "REPAIR"
    assert table["required_repairs"] == ["Table lineage was not reviewed."]


def test_candidate_findings_do_not_promote_or_repair_entire_route() -> None:
    events = [
        {
            "event": "independent_review.requested",
            "paper_id": "p1",
            "review_artifact_path": "review/p1-review.md",
            "routes": {"claim_extraction": {"eligible": 2, "blocked": 0}},
        },
        {
            "event": "independent_review.finding",
            "finding_code": "reviewed_claim_candidate_eligible",
            "paper_id": "p1",
            "route": "claim_extraction",
            "severity": "info",
            "finding": "Candidate has one atomic source-spanned result claim.",
            "candidate_id": "c-good",
            "chunk_id": "chunk-1:split-0001",
            "source_artifact": "normalized/p1.md",
        },
        {
            "event": "independent_review.finding",
            "finding_code": "administrative_roadmap_routed_to_claim_extraction",
            "paper_id": "p1",
            "route": "claim_extraction",
            "severity": "repair_required",
            "finding": "Candidate is only a paper organization roadmap.",
            "candidate_id": "c-bad",
            "chunk_id": "chunk-1:split-0002",
            "source_artifact": "normalized/p1.md",
        },
    ]
    result = synthesize_manifest(
        {"paper_count": 1, "papers": [{"paper_id": "p1", "state": "ok_for_graph"}]},
        events,
    )
    manifest = {"entries": [entry.__dict__ for entry in result.entries]}

    route = _entry(manifest, "p1", "claim_extraction")
    good = _candidate_entry(manifest, "p1", "claim_extraction", "c-good")
    bad = _candidate_entry(manifest, "p1", "claim_extraction", "c-bad")

    assert route["granularity"] == "route"
    assert route["final_eligibility"] == "eligible_with_caveat"
    assert route["finding_codes"] == []
    assert good["granularity"] == "candidate"
    assert good["final_eligibility"] == "eligible"
    assert good["parent_route"] == "claim_extraction"
    assert good["chunk_id"] == "chunk-1:split-0001"
    assert good["source_artifact"] == "normalized/p1.md"
    assert bad["final_eligibility"] == "repair_required"
    assert bad["required_repairs"] == ["Candidate is only a paper organization roadmap."]


def test_create_manifest_writes_candidate_counts_by_granularity(tmp_path: Path) -> None:
    summary_path = tmp_path / "summary.json"
    events_path = tmp_path / "events.jsonl"
    output_path = tmp_path / "manifest.json"
    summary_path.write_text(json.dumps(_summary()), encoding="utf-8")
    events = [
        *_events(),
        {
            "event": "independent_review.finding",
            "finding_code": "reviewed_claim_candidate_eligible",
            "paper_id": "p1",
            "route": "claim_extraction",
            "severity": "info",
            "finding": "Reviewed candidate is eligible.",
            "candidate_id": "c-good",
            "chunk_id": "chunk-1:split-0001",
        },
    ]
    events_path.write_text("\n".join(json.dumps(event) for event in events), encoding="utf-8")

    manifest = create_manifest(
        graph_summary_path=summary_path,
        review_events_path=events_path,
        output_path=output_path,
    )

    assert output_path.exists()
    assert manifest["schema_version"] == "s05-eligibility-manifest.v2"
    assert manifest["scope"] == "prose_claims_only"
    written = json.loads(output_path.read_text(encoding="utf-8"))
    assert written["counts"]["repair_required"] == 2
    assert written["counts"]["review_required"] == 1
    assert written["counts_by_granularity"]["candidate"]["eligible"] == 1
    assert written["counts_by_granularity"]["route"]["repair_required"] == 2
