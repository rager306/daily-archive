from __future__ import annotations

import json
from pathlib import Path

from research_graph.repair.chunk_baseline_measurement import (
    build_baseline_package,
    measure_manifest,
    write_baseline_run,
    write_review_samples,
)
from research_graph.repair.chunk_import_contract import validate_import_ready_package


def _paper(tmp_path: Path, *, paper_id: str = "p1", full_text: str | None = None) -> dict[str, object]:
    paper_dir = tmp_path / paper_id
    paper_dir.mkdir()
    required_paths: list[str] = [str(paper_dir / "full_text.md")]
    if full_text is not None:
        (paper_dir / "full_text.md").write_text(full_text, encoding="utf-8")
    return {
        "paper_id": paper_id,
        "title": "Example Paper",
        "categories": ["cs.AI"],
        "source_artifacts": [f"normalized_markdown:{paper_id}"],
        "required_paths": required_paths,
        "hard_case_tags": ["fixture"],
    }


def _manifest(tmp_path: Path, papers: list[dict[str, object]]) -> Path:
    path = tmp_path / "manifest.json"
    path.write_text(
        json.dumps(
            {
                "schema_version": "m005-gold-corpus-manifest.v1",
                "milestone": "M005-test",
                "broad_corpus_run": False,
                "papers": papers,
            }
        ),
        encoding="utf-8",
    )
    return path


def test_build_baseline_package_marks_current_chunks_retrieval_only(tmp_path: Path) -> None:
    paper = _paper(
        tmp_path,
        full_text="# Title\n\n## Introduction\n\nThis paper introduces a fixture result.\n\n## References\n\n[1] Example.",
    )

    package = build_baseline_package(paper, run_id="run-1")
    validation = validate_import_ready_package(package)

    assert validation.valid_package is True
    assert validation.import_ready is False
    assert validation.import_eligible_chunk_count == 0
    assert validation.refused_chunk_count == 2
    assert package["diagnostics"]["package_state"] == "ok_for_retrieval_only"
    assert package["diagnostics"]["counts_by_route"] == {"retrieval_only": 2}
    assert package["diagnostics"]["refusal_counts"] == {"baseline_retrieval_only_not_import_ready": 2}
    assert all(chunk["route"] == "retrieval_only" for chunk in package["chunks"])
    assert all(chunk["redaction"]["chunk_text_included"] is False for chunk in package["chunks"])


def test_build_baseline_package_reports_missing_full_text_as_reject(tmp_path: Path) -> None:
    paper = _paper(tmp_path, full_text=None)

    package = build_baseline_package(paper, run_id="run-1")
    validation = validate_import_ready_package(package)

    assert validation.valid_package is True
    assert validation.import_ready is False
    assert package["diagnostics"]["package_state"] == "reject"
    assert package["diagnostics"]["refusal_counts"] == {"missing_full_text_artifact": 1}
    assert package["chunks"] == []
    assert package["conversion"]["raw_text_included"] is False
    assert package["conversion"]["embeddings_included"] is False


def test_measure_manifest_aggregates_redacted_baseline(tmp_path: Path) -> None:
    manifest = _manifest(
        tmp_path,
        [
            _paper(tmp_path, paper_id="p1", full_text="# Title\n\n## Body\n\nSubstantive body."),
            _paper(tmp_path, paper_id="p2", full_text=None),
        ],
    )

    result = measure_manifest(manifest)

    assert result.summary["paper_count"] == 2
    assert result.summary["valid_package_count"] == 2
    assert result.summary["import_ready_count"] == 0
    assert result.summary["raw_text_included"] is False
    assert result.summary["embeddings_included"] is False
    assert result.summary["ladybugdb_written"] is False
    assert result.summary["production_import_attempted"] is False
    assert "missing_full_text_artifact" in result.summary["refusal_counts"]
    assert "baseline_retrieval_only_not_import_ready" in result.summary["refusal_counts"]
    assert result.summary["counts_by_route"] == {"retrieval_only": 1}


def test_write_baseline_run_outputs_summary_and_jsonl(tmp_path: Path) -> None:
    manifest = _manifest(tmp_path, [_paper(tmp_path, full_text="# Title\n\n## Body\n\nSubstantive body.")])
    result = measure_manifest(manifest)
    out = tmp_path / "out"

    write_baseline_run(result, out)

    summary = json.loads((out / "baseline-summary.json").read_text(encoding="utf-8"))
    lines = (out / "baseline-package-diagnostics.jsonl").read_text(encoding="utf-8").splitlines()
    assert summary["schema_version"] == "m005-baseline-chunk-measurement.v1"
    assert len(lines) == 1
    record = json.loads(lines[0])
    assert record["schema_version"] == "m005-baseline-package-diagnostic.v1"
    assert record["raw_text_included"] is False
    assert record["embeddings_included"] is False
    assert record["refusal_counts"] == {"baseline_retrieval_only_not_import_ready": 1}
    assert "Substantive body" not in json.dumps(record)


def test_write_review_samples_separates_markdown_snippets_from_machine_index(tmp_path: Path) -> None:
    paper = _paper(
        tmp_path,
        paper_id="p1",
        full_text="# Title\n\n## Body\n\nThis bounded snippet should appear only in markdown review samples.",
    )
    manifest = _manifest(tmp_path, [paper])
    manifest_payload = json.loads(manifest.read_text(encoding="utf-8"))
    manifest_payload["inner_review_minimum"] = ["p1", "p2"]
    manifest.write_text(json.dumps(manifest_payload), encoding="utf-8")
    result = measure_manifest(manifest)
    review_path = tmp_path / "review.md"
    index_path = tmp_path / "index.json"

    write_review_samples(result, manifest, review_path=review_path, index_path=index_path)

    review_text = review_path.read_text(encoding="utf-8")
    index = json.loads(index_path.read_text(encoding="utf-8"))
    serialized_index = json.dumps(index)
    assert "This bounded snippet" in review_text
    assert "This bounded snippet" not in serialized_index
    assert index["schema_version"] == "m005-baseline-review-sample-index.v1"
    assert index["raw_text_in_machine_logs"] is False
    assert index["records"][0]["status"] == "sampled"
    assert index["records"][1]["status"] == "blocked"
