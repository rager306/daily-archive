from __future__ import annotations

import copy
import json
import subprocess
import sys
from pathlib import Path

from research_graph.infrastructure.staging.import_boundary import (
    TRUSTED_IMPORT_USE,
    build_import_boundary_rehearsal,
    validate_import_boundary_rehearsal,
)

CORPUS_DIR = Path("data/article_corpora/m031-catalog-backed-replay-v1")
CHUNK_EVIDENCE_DIR = CORPUS_DIR / "chunk-evidence"
SUMMARY_PATH = CHUNK_EVIDENCE_DIR / "chunk-evidence-summary.json"
CLOSEOUT_PATH = CORPUS_DIR / "chunk-evidence-closeout-summary.json"
GRAPH_PACKAGE_PATH = (
    CHUNK_EVIDENCE_DIR
    / "packages"
    / "arxiv_cs-cl_2507.19457_arxiv_pdf"
    / "graph-readiness-package.json"
)
REVIEW_EVENTS_PATH = CHUNK_EVIDENCE_DIR / "independent-review-events.jsonl"


def _m031_contract() -> dict[str, object]:
    return build_import_boundary_rehearsal(
        summary_path=SUMMARY_PATH,
        closeout_summary_path=CLOSEOUT_PATH,
        graph_readiness_package_paths=[GRAPH_PACKAGE_PATH],
        independent_review_events_path=REVIEW_EVENTS_PATH,
    )


def test_build_import_boundary_rehearsal_rejects_parser_ready_and_zero_chunk_rows() -> None:
    contract = _m031_contract()

    validation = validate_import_boundary_rehearsal(contract)

    assert validation.valid_rehearsal is True
    assert contract["rehearsal_id"] == "import-boundary-refusal-only.v1"
    assert contract["source_benchmark_id"] == "m031-catalog-backed-replay-v1"
    assert contract["candidate_count"] == 7
    assert contract["accepted_count"] == 0
    assert contract["rejected_count"] == 7
    assert contract["trusted_kg_import_allowed"] is False
    assert contract["graph_import_allowed"] is False
    assert contract["production_ladybugdb_write_allowed"] is False
    assert contract["production_import_attempted"] is False
    assert contract["ladybugdb_written"] is False
    assert contract["refusal_counts"] == {
        "completed_independent_graph_readiness_review_required": 1,
        "non_parser_ready_zero_chunk_refusal:catalog_placeholder_pruned_no_article_record": 1,
        "non_parser_ready_zero_chunk_refusal:converted_text_low_quality": 1,
        "non_parser_ready_zero_chunk_refusal:metadata_only_refused": 2,
        "non_parser_ready_zero_chunk_refusal:missing_local_source_path": 2,
    }


def test_build_import_boundary_rehearsal_treats_positive_structural_labels_as_refused() -> (
    None
):
    contract = _m031_contract()

    parser_ready = next(
        candidate
        # pyrefly: ignore [not-iterable]
        for candidate in contract["candidates"]  # ty:ignore[not-iterable]
        if candidate["package_id"] == "arxiv_cs-cl_2507.19457_arxiv_pdf"
    )

    assert parser_ready["route"] == "retrieval_only"
    assert parser_ready["state"] == "ok_for_retrieval_only"
    assert parser_ready["accepted"] is False
    assert parser_ready["rejected"] is True
    assert parser_ready["import_eligible"] is False
    assert parser_ready["trusted_kg_import_allowed"] is False
    assert parser_ready["kg_readiness_claimed"] is False
    assert parser_ready["refusal_reasons"] == [
        "completed_independent_graph_readiness_review_required"
    ]
    assert "independent_graph_readiness_review_required" in parser_ready["remediation_hints"]
    assert TRUSTED_IMPORT_USE not in parser_ready["allowed_uses"]
    assert TRUSTED_IMPORT_USE in parser_ready["excluded_uses"]


def test_build_import_boundary_rehearsal_is_metadata_only_and_has_consistent_counts() -> None:
    contract = _m031_contract()

    forbidden_payload_fragments = (
        "do not expose me",
        "token-value",
        "normalized_markdown",
        "char_start",
        "char_end",
    )
    rendered = repr(contract)

    assert all(fragment not in rendered for fragment in forbidden_payload_fragments)
    # pyrefly: ignore [bad-argument-type]
    assert contract["candidate_count"] == len(contract["candidates"])  # ty:ignore[invalid-argument-type]
    assert contract["accepted_count"] == sum(
        1
        for c in contract["candidates"]  # ty:ignore[not-iterable]  # pyrefly: ignore [not-iterable]
        if c["accepted"] is True  # pyrefly: ignore [not-iterable]
    )
    assert contract["rejected_count"] == sum(
        1
        for c in contract["candidates"]  # ty:ignore[not-iterable]  # pyrefly: ignore [not-iterable]
        if c["rejected"] is True  # pyrefly: ignore [not-iterable]
    )
    # pyrefly: ignore [not-iterable]
    assert all(candidate["raw_text_included"] is False for candidate in contract["candidates"])  # ty:ignore[not-iterable]
    # pyrefly: ignore [not-iterable]
    assert all(candidate["chunk_text_included"] is False for candidate in contract["candidates"])  # ty:ignore[not-iterable]
    # pyrefly: ignore [not-iterable]
    assert all(candidate["embeddings_included"] is False for candidate in contract["candidates"])  # ty:ignore[not-iterable]
    # pyrefly: ignore [not-iterable]
    assert all(candidate["vectors_included"] is False for candidate in contract["candidates"])  # ty:ignore[not-iterable]
    assert all(
        # pyrefly: ignore [not-iterable]
        candidate["production_import_attempted"] is False
        # pyrefly: ignore [not-iterable]
        for candidate in contract["candidates"]  # ty:ignore[not-iterable]
    )
    # pyrefly: ignore [not-iterable]
    assert all(candidate["ladybugdb_written"] is False for candidate in contract["candidates"])  # ty:ignore[not-iterable]


def test_validate_m031_rehearsal_rejects_unsafe_graph_import_flags() -> None:
    contract = _m031_contract()
    unsafe = copy.deepcopy(contract)
    unsafe["trusted_kg_import_allowed"] = True
    unsafe["graph_import_allowed"] = True
    unsafe["production_ladybugdb_write_allowed"] = True
    unsafe["candidates"][0]["trusted_kg_import_allowed"] = True  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    unsafe["candidates"][0]["kg_readiness_claimed"] = True  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]

    validation = validate_import_boundary_rehearsal(unsafe)

    assert validation.valid_rehearsal is False
    assert validation.refusal_counts["unsafe_trusted_kg_import_allowed"] == 2
    assert validation.refusal_counts["unsafe_graph_import_allowed"] == 1
    assert validation.refusal_counts["unsafe_production_ladybugdb_write_allowed"] == 1
    assert validation.refusal_counts["unsafe_kg_readiness_claimed"] == 1


def test_build_import_boundary_rehearsal_requires_completed_review_absence_to_refuse() -> None:
    contract = _m031_contract()

    parser_ready = next(
        candidate
        # pyrefly: ignore [not-iterable]
        for candidate in contract["candidates"]  # ty:ignore[not-iterable]
        if candidate["candidate_type"] == "graph_readiness_package"
    )

    assert contract["source_import_boundary_summary"]["independent_review_completed_count"] == 0  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    assert contract["source_import_boundary_summary"]["pending_graph_readiness_review_count"] == 1  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    assert parser_ready["review_state"] == "pending_independent_graph_readiness_review"
    assert parser_ready["output_contract_completed"] is False
    assert parser_ready["independent_review_completed"] is False
    assert (
        "completed_independent_graph_readiness_review_required" in parser_ready["refusal_reasons"]
    )


def test_replay_m031_import_boundary_cli_writes_redacted_rehearsal_artifacts(
    tmp_path: Path,
) -> None:
    output_dir = tmp_path / "import-boundary-rehearsal"

    result = subprocess.run(
        [
            sys.executable,
            "scripts/replay_m031_import_boundary_rehearsal.py",
            "--output-dir",
            str(output_dir),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    summary = json.loads((output_dir / "import-boundary-summary.json").read_text(encoding="utf-8"))
    diagnostics = [
        json.loads(line)
        for line in (output_dir / "import-boundary-diagnostics.jsonl")
        .read_text(encoding="utf-8")
        .splitlines()
    ]
    report = (output_dir / "import-boundary-report.md").read_text(encoding="utf-8")

    assert "candidates" not in summary
    assert summary["candidate_count"] == 7
    assert summary["accepted_count"] == 0
    assert summary["import_eligible_count"] == 0
    assert summary["rejected_count"] == 7
    assert summary["valid_rehearsal"] is True
    assert summary["diagnostic_code_counts"] == {"M031_IMPORT_BOUNDARY_REFUSED": 7}
    assert summary["trusted_kg_import_allowed"] is False
    assert summary["graph_import_allowed"] is False
    assert summary["production_import_attempted"] is False
    assert summary["ladybugdb_written"] is False
    assert len(diagnostics) == 7
    assert all(
        record["diagnostic_code"] == "M031_IMPORT_BOUNDARY_REFUSED" for record in diagnostics
    )
    assert all(record["blocks_import"] is True for record in diagnostics)
    assert all(record["accepted"] is False for record in diagnostics)
    assert all(record["import_eligible"] is False for record in diagnostics)
    assert all(record["json_path"].startswith("$.candidates[") for record in diagnostics)
    assert "accepted/import-eligible candidates: 0" in report
    assert "LadybugDB writes: false" in report
    assert "normalized_markdown" not in repr(summary) + repr(diagnostics) + report


def test_replay_m031_import_boundary_cli_fails_closed_before_writes_when_closeout_is_not_passed(
    tmp_path: Path,
) -> None:
    closeout = json.loads(CLOSEOUT_PATH.read_text(encoding="utf-8"))
    closeout["status"] = "failed"
    bad_closeout_path = tmp_path / "bad-closeout-summary.json"
    bad_closeout_path.write_text(json.dumps(closeout), encoding="utf-8")
    output_dir = tmp_path / "import-boundary-rehearsal"

    result = subprocess.run(
        [
            sys.executable,
            "scripts/replay_m031_import_boundary_rehearsal.py",
            "--closeout-summary",
            str(bad_closeout_path),
            "--output-dir",
            str(output_dir),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode != 0
    assert "M031_CLOSEOUT_NOT_PASSED" in result.stderr
    assert not (output_dir / "import-boundary-summary.json").exists()
    assert not (output_dir / "import-boundary-diagnostics.jsonl").exists()
    assert not (output_dir / "import-boundary-report.md").exists()


def test_replay_m031_import_boundary_cli_fails_closed_when_review_events_are_absent(
    tmp_path: Path,
) -> None:
    empty_events_path = tmp_path / "independent-review-events.jsonl"
    empty_events_path.write_text("", encoding="utf-8")
    output_dir = tmp_path / "import-boundary-rehearsal"

    result = subprocess.run(
        [
            sys.executable,
            "scripts/replay_m031_import_boundary_rehearsal.py",
            "--independent-review-events",
            str(empty_events_path),
            "--output-dir",
            str(output_dir),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode != 0
    assert "M031_REVIEW_EVENTS_ABSENT" in result.stderr
    assert not output_dir.exists()
