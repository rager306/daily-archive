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
    build_candidate_locator_batch_from_targets,
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


def test_builds_batch_from_m011_style_targets(tmp_path: Path) -> None:
    source_one, digest_one = _write_source(tmp_path, "Abstract\n" + "method result claim\n" * 12)
    source_two = tmp_path / "paper-two.md"
    source_two.write_text("No matching terms here.\n", encoding="utf-8")
    digest_two = hashlib.sha256(source_two.read_bytes()).hexdigest()
    targets = [
        {
            "paper_id": "paper-1",
            "target_id": "target-1",
            "source": {"path": str(source_one), "sha256": digest_one},
            "review_metadata": {"counts_by_route": {"claim_extraction": 1, "method_extraction": 1}},
        },
        {
            "paper_id": "paper-2",
            "target_id": "target-2",
            "source": {"path": str(source_two), "sha256": digest_two},
            "review_metadata": {"counts_by_route": {"claim_extraction": 1}},
        },
    ]

    batch = build_candidate_locator_batch_from_targets(
        run_id="batch-test",
        targets=targets,
        route_specs=DEFAULT_ROUTE_SPECS[:2],
        broad_match_threshold=3,
    )

    assert batch["schema_version"] == CANDIDATE_LOCATOR_PROTOCOL_VERSION
    assert batch["paper_id"] == "bounded-target-batch"
    assert batch["summary"]["paper_count"] == 2
    assert batch["summary"]["source_count"] == 2
    assert batch["summary"]["locator_count"] == 3
    assert batch["summary"]["ambiguous_span_count"] >= 1
    assert batch["summary"]["missing_span_count"] >= 1
    assert batch["summary"]["import_eligible_count"] == 0
    assert batch["summary"]["promoted_to_fact_count"] == 0
    assert len(batch["per_paper_summary"]) == 2
    assert find_forbidden_payload_keys(batch) == []
    assert validate_candidate_locator_artifact(batch) == []


def test_batch_routes_follow_m011_route_metadata(tmp_path: Path) -> None:
    source_path, digest = _write_source(tmp_path, "Abstract\nmethod result claim\n")
    targets = [
        {
            "paper_id": "paper-1",
            "source": {"path": str(source_path), "sha256": digest},
            "review_metadata": {"counts_by_route": {"method_extraction": 1}},
        }
    ]

    batch = build_candidate_locator_batch_from_targets(
        run_id="batch-routes",
        targets=targets,
        route_specs=DEFAULT_ROUTE_SPECS[:2],
    )

    assert batch["summary"]["locator_count"] == 1
    assert batch["locators"][0]["route"] == "method_location"
    assert batch["locators"][0]["candidate_type"] == "method_candidate"


def test_span_hash_is_stable_across_source_paths(tmp_path: Path) -> None:
    content = "Abstract\nA method produces a result.\n"
    first_dir = tmp_path / "first"
    second_dir = tmp_path / "second"
    first_dir.mkdir()
    second_dir.mkdir()
    first_path = first_dir / "paper.md"
    second_path = second_dir / "paper.md"
    first_path.write_text(content, encoding="utf-8")
    second_path.write_text(content, encoding="utf-8")
    digest = hashlib.sha256(first_path.read_bytes()).hexdigest()

    first = build_candidate_locator_artifact(
        run_id="stable-one",
        paper_id="paper-1",
        sources=[LocatorSource(source_id="stable-source", paper_id="paper-1", source_path=first_path, expected_sha256=digest)],
        route_specs=(DEFAULT_ROUTE_SPECS[1],),
    )
    second = build_candidate_locator_artifact(
        run_id="stable-two",
        paper_id="paper-1",
        sources=[LocatorSource(source_id="stable-source", paper_id="paper-1", source_path=second_path, expected_sha256=digest)],
        route_specs=(DEFAULT_ROUTE_SPECS[1],),
    )

    assert first["locators"][0]["source_spans"][0]["span_hash"] == second["locators"][0]["source_spans"][0]["span_hash"]


def test_overlapping_signal_windows_are_diagnosed(tmp_path: Path) -> None:
    source_path, digest = _write_source(tmp_path, "Abstract\nA method result appears in one compact sentence.\n")

    artifact = build_candidate_locator_artifact(
        run_id="overlap",
        paper_id="paper-1",
        sources=[LocatorSource(source_id="source-1", paper_id="paper-1", source_path=source_path, expected_sha256=digest)],
        route_specs=DEFAULT_ROUTE_SPECS[:2],
    )

    assert artifact["summary"]["ambiguous_span_count"] == 2
    for locator in artifact["locators"]:
        assert locator["state"] == "ambiguous_span"
        assert "overlapping_signal_window" in locator["diagnostic_codes"]
        assert "overlapping_signal_window" in locator["source_spans"][0]["ambiguity_diagnostics"]
