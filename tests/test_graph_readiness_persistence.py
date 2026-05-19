from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.graph_readiness_persistence import (
    persist_validation_subset,
    select_trusted_candidate_claims,
    selection_to_dict,
    write_refusal_evidence,
)


def _manifest_entry(
    *,
    paper_id: str = "p1",
    route: str = "claim_extraction",
    final_eligibility: str = "eligible",
    granularity: str = "candidate",
    candidate_id: str | None = "c1",
    chunk_id: str | None = "chunk-1:split-0001",
    finding_codes: list[str] | None = None,
) -> dict[str, object]:
    entry: dict[str, object] = {
        "paper_id": paper_id,
        "route": route,
        "granularity": granularity,
        "entry_id": f"{granularity}:{paper_id}:{route}:{candidate_id or chunk_id or 'none'}",
        "parent_route": route,
        "final_eligibility": final_eligibility,
        "independent_review_verdict": "PASS" if final_eligibility == "eligible" else "REPAIR",
        "finding_codes": finding_codes or ["reviewed_claim_candidate_eligible"],
        "source_artifact": f"normalized/{paper_id}.md",
        "required_repairs": [],
        "caveats": [],
    }
    if candidate_id is not None:
        entry["candidate_id"] = candidate_id
    if chunk_id is not None:
        entry["chunk_id"] = chunk_id
    return entry


def _claim_draft(
    *,
    paper_id: str = "p1",
    route: str = "claim_extraction",
    candidate_id: str = "c1",
    chunk_id: str = "chunk-1:split-0001",
) -> dict[str, object]:
    return {
        "paper_id": paper_id,
        "route": route,
        "candidate_id": candidate_id,
        "chunk_id": chunk_id,
        "entry_id": f"candidate:{paper_id}:{route}:{candidate_id}",
        "source_artifact": f"normalized/{paper_id}.md",
        "claim_text_included": False,
        "persisted": False,
        "finding_codes": ["reviewed_claim_candidate_eligible"],
    }


def _manifest(entries: list[dict[str, object]]) -> dict[str, object]:
    return {"schema_version": "s05-eligibility-manifest.v2", "entries": entries}


def _summary(drafts: list[dict[str, object]]) -> dict[str, object]:
    return {"schema_version": "s05-extraction-route-summary.v1", "claim_drafts": drafts}


def test_selects_only_matching_trusted_candidate_claim() -> None:
    result = select_trusted_candidate_claims(
        manifest=_manifest([_manifest_entry()]),
        extraction_summary=_summary([_claim_draft()]),
    )

    assert result.counts == {"trusted_candidate_claims": 1, "refusals": 0}
    claim = result.trusted_claims[0]
    assert claim.paper_id == "p1"
    assert claim.route == "claim_extraction"
    assert claim.candidate_id == "c1"
    assert claim.chunk_id == "chunk-1:split-0001"
    assert claim.final_eligibility == "eligible"
    assert claim.raw_text_included is False
    assert claim.persisted is False
    assert claim.provenance["selection_rule"] == "s06_trusted_candidate_claim_v1"


def test_refuses_non_candidate_and_non_eligible_entries() -> None:
    entries = [
        _manifest_entry(granularity="route", candidate_id=None, chunk_id=None),
        _manifest_entry(final_eligibility="eligible_with_caveat", candidate_id="c-caveat", chunk_id="chunk-caveat"),
        _manifest_entry(final_eligibility="repair_required", candidate_id="c-repair", chunk_id="chunk-repair"),
        _manifest_entry(final_eligibility="route_excluded", candidate_id="c-excluded", chunk_id="chunk-excluded"),
        _manifest_entry(final_eligibility="review_required", candidate_id="c-review", chunk_id="chunk-review"),
    ]

    result = select_trusted_candidate_claims(
        manifest=_manifest(entries),
        extraction_summary=_summary([]),
    )

    assert result.trusted_claims == []
    assert result.counts["refusals"] == 5
    assert result.counts["refused_not_candidate_granularity"] == 1
    assert result.counts["refused_final_eligibility_eligible_with_caveat"] == 1
    assert result.counts["refused_final_eligibility_repair_required"] == 1
    assert result.counts["refused_final_eligibility_route_excluded"] == 1
    assert result.counts["refused_final_eligibility_review_required"] == 1


def test_refuses_route_level_eligible_metadata_and_method_entries() -> None:
    entries = [
        _manifest_entry(route="metadata_graph", granularity="route", candidate_id=None, chunk_id=None),
        _manifest_entry(route="method_extraction", granularity="route", candidate_id=None, chunk_id=None),
    ]

    result = select_trusted_candidate_claims(manifest=_manifest(entries), extraction_summary=_summary([]))

    assert result.trusted_claims == []
    assert result.counts["refused_not_candidate_granularity"] == 2


def test_refuses_candidate_without_explicit_trusted_review_code() -> None:
    result = select_trusted_candidate_claims(
        manifest=_manifest([_manifest_entry(finding_codes=["sample_scoped_claim_promotion"])]),
        extraction_summary=_summary([_claim_draft()]),
    )

    assert result.trusted_claims == []
    assert result.counts["refused_missing_explicit_trusted_review_finding"] == 1


def test_refuses_candidate_without_matching_claim_draft() -> None:
    result = select_trusted_candidate_claims(
        manifest=_manifest([_manifest_entry(candidate_id="c1", chunk_id="chunk-1")]),
        extraction_summary=_summary([_claim_draft(candidate_id="c2", chunk_id="chunk-2")]),
    )

    assert result.trusted_claims == []
    assert result.counts["refused_missing_matching_claim_draft"] == 1
    assert result.counts["refused_claim_draft_without_trusted_manifest_entry"] == 1


def test_selection_serialization_is_redacted_and_json_safe() -> None:
    result = select_trusted_candidate_claims(
        manifest=_manifest([_manifest_entry()]),
        extraction_summary=_summary([_claim_draft()]),
    )

    payload = selection_to_dict(result)
    serialized = json.dumps(payload)

    assert payload["schema_version"] == "s06-trusted-candidate-selection.v1"
    assert payload["raw_text_included"] is False
    assert "claim_text" not in serialized
    assert payload["trusted_claims"][0]["raw_text_included"] is False


def test_persist_validation_subset_writes_redacted_jsonl_and_summary(tmp_path: Path) -> None:
    manifest_path = tmp_path / "manifest.json"
    summary_path = tmp_path / "extraction-summary.json"
    output_dir = tmp_path / "out"
    manifest_path.write_text(json.dumps(_manifest([_manifest_entry()])), encoding="utf-8")
    summary_path.write_text(json.dumps(_summary([_claim_draft()])), encoding="utf-8")

    result = persist_validation_subset(
        manifest_path=manifest_path,
        extraction_summary_path=summary_path,
        output_dir=output_dir,
    )

    assert result.claims_path.exists()
    assert result.summary_path.exists()
    lines = result.claims_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1
    persisted = json.loads(lines[0])
    assert persisted["schema_version"] == "s06-persisted-candidate-claim.v1"
    assert persisted["persisted_scope"] == "validation_subset"
    assert persisted["persisted"] is True
    assert persisted["raw_text_included"] is False
    assert persisted["claim_text_included"] is False
    assert persisted["embeddings_included"] is False
    assert "text" not in persisted
    assert "Local markdown" not in json.dumps(persisted)
    summary = json.loads(result.summary_path.read_text(encoding="utf-8"))
    assert summary["schema_version"] == "s06-validation-persistence-summary.v1"
    assert summary["persisted_count"] == 1
    assert summary["selected_count"] == 1
    assert summary["refused_count"] == 0
    assert summary["kg_persistence_attempted"] is False
    assert summary["ladybugdb_written"] is False


def test_persist_validation_subset_records_refusal_counts(tmp_path: Path) -> None:
    manifest_path = tmp_path / "manifest.json"
    summary_path = tmp_path / "extraction-summary.json"
    output_dir = tmp_path / "out"
    manifest_path.write_text(
        json.dumps(
            _manifest(
                [
                    _manifest_entry(),
                    _manifest_entry(final_eligibility="repair_required", candidate_id="c2", chunk_id="chunk-2"),
                ]
            )
        ),
        encoding="utf-8",
    )
    summary_path.write_text(json.dumps(_summary([_claim_draft()])), encoding="utf-8")

    result = persist_validation_subset(
        manifest_path=manifest_path,
        extraction_summary_path=summary_path,
        output_dir=output_dir,
    )

    assert result.summary["persisted_count"] == 1
    assert result.summary["refused_count"] == 1
    assert result.summary["refusal_counts"] == {"final_eligibility_repair_required": 1}


def test_write_refusal_evidence_groups_negative_paths(tmp_path: Path) -> None:
    selection = select_trusted_candidate_claims(
        manifest=_manifest(
            [
                _manifest_entry(),
                _manifest_entry(granularity="route", candidate_id=None, chunk_id=None),
                _manifest_entry(final_eligibility="eligible_with_caveat", candidate_id="c-caveat", chunk_id="chunk-caveat"),
                _manifest_entry(final_eligibility="repair_required", candidate_id="c-repair", chunk_id="chunk-repair"),
                _manifest_entry(final_eligibility="route_excluded", candidate_id="c-excluded", chunk_id="chunk-excluded"),
                _manifest_entry(final_eligibility="review_required", candidate_id="c-review", chunk_id="chunk-review"),
                _manifest_entry(route="metadata_graph", granularity="route", candidate_id=None, chunk_id=None),
            ]
        ),
        extraction_summary=_summary([_claim_draft()]),
    )
    output_path = tmp_path / "persistence-refusals.json"

    payload = write_refusal_evidence(selection=selection, output_path=output_path)

    assert output_path.exists()
    assert payload["schema_version"] == "s06-persistence-refusals.v1"
    assert payload["persisted_count"] == 1
    assert payload["refused_count"] == 6
    assert payload["raw_text_included"] is False
    assert payload["embeddings_included"] is False
    assert payload["refusal_counts"]["not_candidate_granularity"] == 2
    assert payload["refusal_counts"]["final_eligibility_eligible_with_caveat"] == 1
    assert payload["refusal_counts"]["final_eligibility_repair_required"] == 1
    assert payload["refusal_counts"]["final_eligibility_route_excluded"] == 1
    assert payload["refusal_counts"]["final_eligibility_review_required"] == 1
    serialized = output_path.read_text(encoding="utf-8")
    assert "Local markdown" not in serialized
