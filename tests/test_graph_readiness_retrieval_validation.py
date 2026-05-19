from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.graph_readiness_retrieval_validation import (
    fixture_load_to_dict,
    load_retrieval_fixture,
    run_exclusion_checks,
    run_retrieval_validation,
)


def _valid_record(**overrides: object) -> dict[str, object]:
    record: dict[str, object] = {
        "schema_version": "s06-persisted-candidate-claim.v1",
        "persisted_scope": "validation_subset",
        "paper_id": "p1",
        "candidate_id": "c1",
        "chunk_id": "chunk-1:split-0001",
        "entry_id": "candidate:p1:claim_extraction:c1",
        "claim_draft_id": "candidate:p1:claim_extraction:c1",
        "source_artifact": "normalized/p1.md",
        "finding_codes": ["reviewed_claim_candidate_eligible"],
        "persisted": True,
        "raw_text_included": False,
        "claim_text_included": False,
        "embeddings_included": False,
    }
    record.update(overrides)
    return record


def _write_jsonl(path: Path, records: list[dict[str, object] | str]) -> None:
    lines = [record if isinstance(record, str) else json.dumps(record) for record in records]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def test_load_retrieval_fixture_accepts_valid_redacted_records(tmp_path: Path) -> None:
    path = tmp_path / "persisted-candidate-claims.jsonl"
    _write_jsonl(path, [_valid_record(), _valid_record(paper_id="p2", candidate_id="c2", chunk_id="chunk-2")])

    result = load_retrieval_fixture(path)

    assert result.refusals == []
    assert result.counts == {
        "accepted_records": 2,
        "refused_records": 0,
        "raw_text_included": 0,
        "embeddings_included": 0,
    }
    assert result.records[0].paper_id == "p1"
    assert result.records[0].candidate_id == "c1"
    assert result.records[0].persisted_scope == "validation_subset"
    assert result.records[0].raw_text_included is False
    assert result.records[0].embeddings_included is False


def test_load_retrieval_fixture_rejects_wrong_scope_and_unpersisted_records(tmp_path: Path) -> None:
    path = tmp_path / "claims.jsonl"
    _write_jsonl(
        path,
        [
            _valid_record(persisted_scope="production"),
            _valid_record(candidate_id="c2", chunk_id="chunk-2", persisted=False),
        ],
    )

    result = load_retrieval_fixture(path)

    assert result.records == []
    assert result.counts["refused_unexpected_persisted_scope"] == 1
    assert result.counts["refused_not_persisted"] == 1


def test_load_retrieval_fixture_rejects_raw_text_and_embeddings(tmp_path: Path) -> None:
    path = tmp_path / "claims.jsonl"
    _write_jsonl(
        path,
        [
            _valid_record(text="raw claim text must not be loaded"),
            _valid_record(candidate_id="c2", chunk_id="chunk-2", embeddings=[0.1, 0.2]),
            _valid_record(candidate_id="c3", chunk_id="chunk-3", raw_text_included=True),
            _valid_record(candidate_id="c4", chunk_id="chunk-4", embeddings_included=True),
        ],
    )

    result = load_retrieval_fixture(path)

    assert result.records == []
    assert result.counts["refused_forbidden_field_text"] == 1
    assert result.counts["refused_forbidden_field_embeddings"] == 1
    assert result.counts["refused_raw_text_flag_not_false"] == 1
    assert result.counts["refused_embeddings_flag_not_false"] == 1


def test_load_retrieval_fixture_rejects_missing_provenance_fields(tmp_path: Path) -> None:
    path = tmp_path / "claims.jsonl"
    missing_source = _valid_record(source_artifact="")
    missing_candidate = _valid_record(candidate_id="")
    missing_findings = _valid_record(finding_codes=[])
    _write_jsonl(path, [missing_source, missing_candidate, missing_findings])

    result = load_retrieval_fixture(path)

    assert result.records == []
    assert result.counts["refused_missing_source_artifact"] == 1
    assert result.counts["refused_missing_candidate_id"] == 1
    assert result.counts["refused_missing_finding_codes"] == 1


def test_fixture_load_serialization_is_redacted(tmp_path: Path) -> None:
    path = tmp_path / "claims.jsonl"
    _write_jsonl(path, [_valid_record()])

    payload = fixture_load_to_dict(load_retrieval_fixture(path))
    serialized = json.dumps(payload)

    assert payload["schema_version"] == "s07-retrieval-fixture-load.v1"
    assert payload["raw_text_included"] is False
    assert payload["embeddings_included"] is False
    assert "raw claim text" not in serialized
    assert payload["records"][0]["raw_text_included"] is False


def test_run_retrieval_validation_exact_id_queries_cover_fixture_records(tmp_path: Path) -> None:
    path = tmp_path / "persisted-candidate-claims.jsonl"
    _write_jsonl(path, [_valid_record(), _valid_record(paper_id="p2", candidate_id="c2", chunk_id="chunk-2")])

    result = run_retrieval_validation(path)

    assert result.summary["record_count"] == 2
    assert result.summary["query_count"] == 8
    assert result.summary["hit_count"] == 8
    assert result.summary["exact_match_rate"] == 1.0
    assert result.summary["candidate_coverage"] == 1.0
    assert result.summary["chunk_coverage"] == 1.0
    assert result.summary["raw_text_included"] is False
    assert result.summary["embeddings_included"] is False
    assert result.summary["llm_used"] is False
    assert result.summary["ladybugdb_used"] is False
    assert {event["query_type"] for event in result.events} == {"paper_id", "candidate_id", "chunk_id", "source_artifact"}
    assert all(event["hit"] for event in result.events)
    assert all(event["raw_text_included"] is False for event in result.events)


def test_run_retrieval_validation_writes_results_and_events(tmp_path: Path) -> None:
    path = tmp_path / "persisted-candidate-claims.jsonl"
    out = tmp_path / "out"
    _write_jsonl(path, [_valid_record()])

    result = run_retrieval_validation(path, output_dir=out)

    assert result.summary["record_count"] == 1
    results_path = out / "retrieval-validation-results.json"
    events_path = out / "retrieval-validation-events.jsonl"
    assert results_path.exists()
    assert events_path.exists()
    summary = json.loads(results_path.read_text(encoding="utf-8"))
    assert summary["schema_version"] == "s07-retrieval-validation-results.v1"
    events_text = events_path.read_text(encoding="utf-8")
    assert "retrieval_validation.query" in events_text
    assert "raw claim text" not in events_text


def test_run_retrieval_validation_blocks_when_loader_refuses_records(tmp_path: Path) -> None:
    path = tmp_path / "claims.jsonl"
    _write_jsonl(path, [_valid_record(source_artifact="")])

    result = run_retrieval_validation(path)

    assert result.records == []
    assert result.events == []
    assert result.summary["record_count"] == 0
    assert result.summary["load_refusals"] == 1
    assert result.summary["exact_match_rate"] == 0.0


def _write_refusals(path: Path, refusals_by_reason: dict[str, list[dict[str, object]]]) -> None:
    path.write_text(
        json.dumps(
            {
                "schema_version": "s06-persistence-refusals.v1",
                "refusals_by_reason": refusals_by_reason,
                "raw_text_included": False,
                "embeddings_included": False,
            }
        ),
        encoding="utf-8",
    )


def test_run_exclusion_checks_pass_when_refused_entries_are_absent(tmp_path: Path) -> None:
    claims_path = tmp_path / "claims.jsonl"
    refusals_path = tmp_path / "refusals.json"
    output_path = tmp_path / "exclusions.json"
    _write_jsonl(claims_path, [_valid_record(candidate_id="allowed", chunk_id="allowed-chunk")])
    _write_refusals(
        refusals_path,
        {
            "final_eligibility_repair_required": [
                {"paper_id": "p1", "candidate_id": "blocked", "chunk_id": "blocked-chunk"}
            ]
        },
    )

    payload = run_exclusion_checks(claims_path=claims_path, refusals_path=refusals_path, output_path=output_path)

    assert output_path.exists()
    assert payload["passed"] is True
    assert payload["forbidden_hit_count"] == 0
    assert payload["persisted_record_count"] == 1
    assert payload["refused_entry_count"] == 1
    assert payload["refusal_counts"] == {"final_eligibility_repair_required": 1}
    assert payload["raw_text_included"] is False


def test_run_exclusion_checks_fails_when_refused_candidate_is_persisted(tmp_path: Path) -> None:
    claims_path = tmp_path / "claims.jsonl"
    refusals_path = tmp_path / "refusals.json"
    _write_jsonl(claims_path, [_valid_record(candidate_id="blocked", chunk_id="allowed-chunk")])
    _write_refusals(
        refusals_path,
        {
            "final_eligibility_repair_required": [
                {"paper_id": "p1", "candidate_id": "blocked", "chunk_id": "blocked-chunk"}
            ]
        },
    )

    payload = run_exclusion_checks(claims_path=claims_path, refusals_path=refusals_path)

    assert payload["passed"] is False
    assert payload["forbidden_hit_count"] == 1
    assert payload["forbidden_hits"][0]["match_type"] == "candidate_id"


def test_run_exclusion_checks_allows_refused_sibling_candidate_from_same_chunk(tmp_path: Path) -> None:
    claims_path = tmp_path / "claims.jsonl"
    refusals_path = tmp_path / "refusals.json"
    _write_jsonl(claims_path, [_valid_record(candidate_id="allowed", chunk_id="shared-chunk")])
    _write_refusals(
        refusals_path,
        {
            "final_eligibility_repair_required": [
                {"paper_id": "p1", "candidate_id": "blocked", "chunk_id": "shared-chunk"}
            ]
        },
    )

    payload = run_exclusion_checks(claims_path=claims_path, refusals_path=refusals_path)

    assert payload["passed"] is True
    assert payload["forbidden_hit_count"] == 0
