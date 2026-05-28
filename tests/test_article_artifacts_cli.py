from __future__ import annotations

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
    assert manifest_path.exists()
    assert summary_path.exists()

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
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
    assert summary["artifact_count"] == 5

    serialized = json.dumps({"manifest": manifest, "summary": summary})
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


def test_article_artifacts_detect_rejects_missing_input_structure(tmp_path: Path) -> None:
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


def test_article_artifacts_detect_rejects_non_fixture_schema(tmp_path: Path) -> None:
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
