from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path

FIXTURE_DIR = Path(__file__).parent / "fixtures" / "article_artifacts"
INPUT_STRUCTURE = FIXTURE_DIR / "basic_article_structure.json"


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["uv", "run", "python", "-m", "arxiv_archive", *args],
        check=False,
        capture_output=True,
        text=True,
    )


def test_article_artifacts_help_lists_contract_and_detect() -> None:
    result = _run_cli("article-artifacts", "--help")

    assert result.returncode == 0
    output = result.stdout.lower()
    assert "contract" in output
    assert "detect" in output
    assert "fixture-only" in output
    assert "no production kg import" in output


def test_article_artifacts_contract_json_reports_no_import_boundary() -> None:
    result = _run_cli("article-artifacts", "contract", "--json")

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["status"] == "contract_only"
    assert payload["schema_version"] == "m023-article-artifacts.v1"
    assert payload["run_schema_version"] == "m023-article-artifact-run.v1"
    assert payload["detector_mode"] == "deterministic_fixture_only"
    assert payload["production_import_attempted"] is False
    assert payload["ladybugdb_written"] is False
    assert payload["trusted_kg_import_allowed"] is False
    assert payload["raw_text_included"] is False
    assert payload["model_outputs_included"] is False


def test_article_artifacts_detect_writes_redacted_manifest_and_summary(tmp_path: Path) -> None:
    result = _run_cli(
        "article-artifacts",
        "detect",
        "--input-structure",
        str(INPUT_STRUCTURE),
        "--output-dir",
        str(tmp_path),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["status"] == "detected"
    assert payload["paper_id"] == "fixture-paper-0001"
    assert payload["artifact_count"] == 5
    assert payload["candidate_link_count"] == 6
    assert payload["diagnostic_count"] == 0
    assert payload["missing_span_count"] == 0
    assert payload["diagnostic_summary"]["artifact_counts_by_type"] == {
        "equation": 1,
        "figure": 1,
        "reference": 1,
        "section": 2,
    }
    assert payload["production_import_attempted"] is False
    assert payload["ladybugdb_written"] is False
    assert payload["trusted_kg_import_allowed"] is False
    assert payload["import_eligible_count"] == 0
    assert payload["promoted_to_fact_count"] == 0
    assert payload["provenance_hints"]["detector"] == "redacted_fixture_v1"

    manifest_path = Path(payload["manifest_path"])
    summary_path = Path(payload["run_summary_path"])
    diagnostics_path = Path(payload["diagnostics_path"])
    assert manifest_path.exists()
    assert summary_path.exists()
    assert diagnostics_path.exists()
    assert payload["output_paths"] == {
        "manifest": str(manifest_path),
        "run_summary": str(summary_path),
        "diagnostics": str(diagnostics_path),
    }
    expected_input_sha = hashlib.sha256(INPUT_STRUCTURE.read_bytes()).hexdigest()
    assert payload["input_hashes"] == {"input_structure_sha256": expected_input_sha}

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    diagnostics = json.loads(diagnostics_path.read_text(encoding="utf-8"))
    assert manifest["schema_version"] == "m023-article-artifacts.v1"
    assert manifest["summary"]["artifact_counts_by_type"] == {
        "equation": 1,
        "figure": 1,
        "reference": 1,
        "section": 2,
    }
    assert manifest["summary"]["candidate_link_type_counts"] == {
        "cites": 1,
        "contains": 3,
        "located_in": 1,
        "supports": 1,
    }
    assert manifest["summary"]["missing_span_count"] == 0
    assert manifest["safety_flags"]["raw_text_included"] is False
    assert manifest["safety_flags"]["model_outputs_included"] is False
    assert manifest["production_import_attempted"] is False
    assert manifest["ladybugdb_written"] is False
    assert summary["schema_version"] == "m023-article-artifact-run.v1"
    assert summary["manifest_schema_version"] == "m023-article-artifacts.v1"
    assert summary["diagnostics_schema_version"] == "m023-article-artifact-diagnostics.v1"
    assert summary["artifact_count"] == 5
    assert summary["input_hashes"] == {"input_structure_sha256": expected_input_sha}
    assert summary["output_paths"] == payload["output_paths"]
    assert summary["production_import_attempted"] is False
    assert summary["ladybugdb_written"] is False
    assert summary["trusted_kg_import_allowed"] is False
    assert diagnostics["schema_version"] == "m023-article-artifact-diagnostics.v1"
    assert diagnostics["run_schema_version"] == "m023-article-artifact-run.v1"
    assert diagnostics["manifest_schema_version"] == "m023-article-artifacts.v1"
    assert diagnostics["diagnostic_count"] == 0
    assert diagnostics["diagnostic_codes"] == []
    assert diagnostics["input_hashes"] == {"input_structure_sha256": expected_input_sha}
    assert diagnostics["output_paths"] == payload["output_paths"]
    assert diagnostics["manifest_diagnostic_summaries"]["fixture-paper-0001"]["missing_span_count"] == 0
    assert diagnostics["production_import_attempted"] is False
    assert diagnostics["ladybugdb_written"] is False
    assert diagnostics["trusted_kg_import_allowed"] is False

    serialized = json.dumps({"manifest": manifest, "summary": summary, "diagnostics": diagnostics})
    for forbidden_fragment in (
        "raw paper text",
        '"text":',
        '"caption_text":',
        '"raw_model_output":',
        '"embedding":',
        '"vector":',
        '"secret":',
        '"source_of_truth":',
        '"trusted_kg_import_allowed": true',
        '"ladybugdb_written": true',
        '"production_import_attempted": true',
        '"model_outputs_included": true',
    ):
        assert forbidden_fragment not in serialized
def test_article_artifacts_detect_persists_stable_diagnostics_artifact(tmp_path: Path) -> None:
    diagnostic_input = tmp_path / "diagnostic_structure.json"
    structure = json.loads(INPUT_STRUCTURE.read_text(encoding="utf-8"))
    structure["artifact_placeholders"][0]["span_id"] = "fixture-paper-0001:span:missing"
    diagnostic_input.write_text(json.dumps(structure), encoding="utf-8")

    result = _run_cli(
        "article-artifacts",
        "detect",
        "--input-structure",
        str(diagnostic_input),
        "--output-dir",
        str(tmp_path / "out"),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    diagnostics = json.loads(Path(payload["diagnostics_path"]).read_text(encoding="utf-8"))
    summary = json.loads(Path(payload["run_summary_path"]).read_text(encoding="utf-8"))

    assert payload["diagnostic_codes"] == ["missing_span"]
    assert diagnostics["diagnostic_codes"] == ["missing_span"]
    assert diagnostics["diagnostic_counts_by_code"] == {"missing_span": 1}
    assert diagnostics["diagnostics"][0]["json_path"] == "/safe_spans"
    assert summary["diagnostic_codes"] == ["missing_span"]
    assert summary["diagnostic_count"] == 1
    assert diagnostics["trusted_kg_import_allowed"] is False
    assert diagnostics["safety_flags"]["raw_text_included"] is False
    assert "raw paper text" not in json.dumps(diagnostics)


    result = _run_cli(
        "article-artifacts",
        "detect",
        "--input-structure",
        str(tmp_path / "missing.json"),
        "--output-dir",
        str(tmp_path / "out"),
        "--json",
    )

    assert result.returncode != 0
    assert "input structure could not be read" in result.stderr.lower()


def test_article_artifacts_detect_rejects_malformed_json(tmp_path: Path) -> None:
    bad_input = tmp_path / "bad_structure.json"
    bad_input.write_text("{not json", encoding="utf-8")

    result = _run_cli(
        "article-artifacts",
        "detect",
        "--input-structure",
        str(bad_input),
        "--output-dir",
        str(tmp_path / "out"),
        "--json",
    )

    assert result.returncode != 0
    assert "input structure must be json" in result.stderr.lower()


    bad_input = tmp_path / "bad_structure.json"
    bad_input.write_text(json.dumps({"schema_version": "wrong", "paper_id": "p1"}), encoding="utf-8")

    result = _run_cli(
        "article-artifacts",
        "detect",
        "--input-structure",
        str(bad_input),
        "--output-dir",
        str(tmp_path / "out"),
        "--json",
    )

    assert result.returncode != 0
    assert "m023-redacted-article-structure.v1" in result.stderr



def test_article_artifacts_detect_does_not_require_adjacent_expected_manifest(tmp_path: Path) -> None:
    standalone_input = tmp_path / "standalone_structure.json"
    standalone_input.write_text(INPUT_STRUCTURE.read_text(encoding="utf-8"), encoding="utf-8")

    result = _run_cli(
        "article-artifacts",
        "detect",
        "--input-structure",
        str(standalone_input),
        "--output-dir",
        str(tmp_path / "out"),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["artifact_count"] == 5
    assert payload["candidate_link_count"] == 6
    assert Path(payload["manifest_path"]).exists()
