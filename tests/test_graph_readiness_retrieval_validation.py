from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.graph_readiness_retrieval_validation import (
    fixture_load_to_dict,
    load_retrieval_fixture,
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
