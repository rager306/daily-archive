from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from arxiv_archive.candidate_locators import (
    CANDIDATE_LOCATOR_PROTOCOL_VERSION,
    DEFAULT_ROUTE_SPECS,
    LocatorSource,
    build_candidate_locator_artifact,
    find_forbidden_payload_keys,
    validate_candidate_locator_artifact,
    write_candidate_locator_artifact,
)


def _write_source(tmp_path: Path, text: str) -> tuple[Path, str]:
    source_path = tmp_path / "paper.md"
    source_path.write_text(text, encoding="utf-8")
    return source_path, hashlib.sha256(source_path.read_bytes()).hexdigest()


def test_builds_import_disabled_locator_artifact_without_raw_text(tmp_path: Path) -> None:
    source_path, digest = _write_source(
        tmp_path,
        "Abstract\nWe introduce a deterministic method for graph retrieval. "
        "The result shows improved provenance review.\n",
    )

    artifact = build_candidate_locator_artifact(
        run_id="test-run",
        paper_id="paper-1",
        sources=[LocatorSource(source_id="source-1", paper_id="paper-1", source_path=source_path, expected_sha256=digest)],
        route_specs=DEFAULT_ROUTE_SPECS,
    )

    assert artifact["schema_version"] == CANDIDATE_LOCATOR_PROTOCOL_VERSION
    assert artifact["summary"]["source_count"] == 1
    assert artifact["summary"]["locator_count"] >= 2
    assert artifact["summary"]["import_eligible_count"] == 0
    assert artifact["summary"]["promoted_to_fact_count"] == 0
    assert artifact["safety_flags"]["production_import_attempted"] is False
    assert artifact["safety_flags"]["ladybugdb_written"] is False
    assert artifact["safety_flags"]["raw_text_included"] is False
    assert find_forbidden_payload_keys(artifact) == []

    for locator in artifact["locators"]:
        assert locator["import_eligible"] is False
        assert locator["promoted_to_fact"] is False
        assert locator["minimax_source_of_truth"] is False
        assert "trusted_kg_import" in locator["excluded_uses"]
        assert locator["source_spans"]
        for span in locator["source_spans"]:
            assert span["raw_text_embedded"] is False
            if span["coordinate_space"] != "artifact_record":
                assert span["char_end"] > span["char_start"] >= 0

    diagnostics = validate_candidate_locator_artifact(artifact)
    assert diagnostics == []


def test_source_hash_mismatch_blocks_source_and_marks_missing_span(tmp_path: Path) -> None:
    source_path, _digest = _write_source(tmp_path, "Abstract\nA method appears here.\n")

    artifact = build_candidate_locator_artifact(
        run_id="hash-mismatch",
        paper_id="paper-1",
        sources=[LocatorSource(source_id="source-1", paper_id="paper-1", source_path=source_path, expected_sha256="0" * 64)],
        route_specs=DEFAULT_ROUTE_SPECS[:1],
    )

    assert artifact["source_ledger"][0]["conversion_status"] == "blocked"
    assert artifact["summary"]["missing_span_count"] == 1
    locator = artifact["locators"][0]
    assert locator["state"] == "missing_span"
    assert locator["review_queue_reason"] == "span_missing"
    assert "source_hash_mismatch" in locator["diagnostic_codes"]
    assert validate_candidate_locator_artifact(artifact) == []


def test_broad_repeated_signal_is_ambiguous(tmp_path: Path) -> None:
    source_path, digest = _write_source(tmp_path, "\n".join(["method result claim"] * 20))

    artifact = build_candidate_locator_artifact(
        run_id="ambiguous",
        paper_id="paper-1",
        sources=[LocatorSource(source_id="source-1", paper_id="paper-1", source_path=source_path, expected_sha256=digest)],
        route_specs=DEFAULT_ROUTE_SPECS[:1],
        broad_match_threshold=3,
    )

    locator = artifact["locators"][0]
    assert locator["state"] == "ambiguous_span"
    assert locator["review_queue_reason"] == "span_ambiguous"
    assert "broad_signal_many_matches" in locator["diagnostic_codes"]
    assert artifact["summary"]["ambiguous_span_count"] == 1


def test_missing_signal_records_missing_span(tmp_path: Path) -> None:
    source_path, digest = _write_source(tmp_path, "No relevant route marker is present.\n")

    artifact = build_candidate_locator_artifact(
        run_id="missing-signal",
        paper_id="paper-1",
        sources=[LocatorSource(source_id="source-1", paper_id="paper-1", source_path=source_path, expected_sha256=digest)],
        route_specs=DEFAULT_ROUTE_SPECS[:1],
    )

    locator = artifact["locators"][0]
    assert locator["state"] == "missing_span"
    assert locator["support_level"] == "insufficient"
    assert locator["review_queue_reason"] == "span_missing"
    assert "signal_missing" in locator["diagnostic_codes"]


def test_validate_rejects_forbidden_payload_key_and_unsafe_flags(tmp_path: Path) -> None:
    source_path, digest = _write_source(tmp_path, "Abstract\nA method appears here.\n")
    artifact = build_candidate_locator_artifact(
        run_id="unsafe",
        paper_id="paper-1",
        sources=[LocatorSource(source_id="source-1", paper_id="paper-1", source_path=source_path, expected_sha256=digest)],
        route_specs=DEFAULT_ROUTE_SPECS[:1],
    )
    artifact["locators"][0]["import_eligible"] = True
    artifact["locators"][0]["source_spans"][0]["chunk_text"] = "must not persist"
    artifact["safety_flags"]["ladybugdb_written"] = True

    diagnostics = validate_candidate_locator_artifact(artifact)

    assert "locator_import_eligible_true:m021-paper-1-claim-001" in diagnostics
    assert "safety_flag_true:ladybugdb_written" in diagnostics
    assert any(item.endswith("/chunk_text") for item in diagnostics)


def test_validate_rejects_invalid_coordinates(tmp_path: Path) -> None:
    source_path, digest = _write_source(tmp_path, "Abstract\nA method appears here.\n")
    artifact = build_candidate_locator_artifact(
        run_id="bad-coordinates",
        paper_id="paper-1",
        sources=[LocatorSource(source_id="source-1", paper_id="paper-1", source_path=source_path, expected_sha256=digest)],
        route_specs=(DEFAULT_ROUTE_SPECS[1],),
    )
    artifact["locators"][0]["source_spans"][0]["char_end"] = artifact["locators"][0]["source_spans"][0]["char_start"]

    diagnostics = validate_candidate_locator_artifact(artifact)

    assert "invalid_span_coordinates:m021-paper-1-method-001" in diagnostics


def test_writer_persists_safe_json_without_forbidden_payload_keys(tmp_path: Path) -> None:
    source_path, digest = _write_source(tmp_path, "Abstract\nA method appears here.\n")
    artifact = build_candidate_locator_artifact(
        run_id="writer",
        paper_id="paper-1",
        sources=[LocatorSource(source_id="source-1", paper_id="paper-1", source_path=source_path, expected_sha256=digest)],
        route_specs=DEFAULT_ROUTE_SPECS[:1],
    )

    output_path = write_candidate_locator_artifact(artifact, tmp_path / "locator.json")
    persisted = json.loads(output_path.read_text(encoding="utf-8"))

    assert output_path.exists()
    assert find_forbidden_payload_keys(persisted) == []
    assert validate_candidate_locator_artifact(persisted) == []


def test_writer_refuses_invalid_artifact(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="candidate locator artifact failed validation"):
        write_candidate_locator_artifact({"schema_version": "bad", "text": "unsafe"}, tmp_path / "bad.json")
