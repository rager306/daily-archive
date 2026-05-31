from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

from arxiv_archive.validation_batch_state import (
    ScanArtifactPaths,
    SelectedPaper,
    SourceReadiness,
    ValidationBatchState,
    write_batch_state,
)

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_batch_validation"
FORBIDDEN_OUTPUT = (
    "FORBIDDEN_RAW_ARTICLE_TEXT_DO_NOT_ECHO",
    "FORBIDDEN_CHUNK_TEXT",
    "FORBIDDEN_TABLE_TEXT_DO_NOT_ECHO",
    "secret-token",
    "api_key=",
    "token=",
)


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["uv", "run", "python", "-m", "arxiv_archive", *args],
        check=False,
        capture_output=True,
        text=True,
    )


def _load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(payload, dict)
    return payload


def _assert_redacted(text: str) -> None:
    for forbidden in FORBIDDEN_OUTPUT:
        assert forbidden not in text


def test_article_report_help_exposes_safe_options() -> None:
    result = _run_cli("validation-batch", "article-report", "--help")

    assert result.returncode == 0
    assert "--manifest-path" in result.stdout
    assert "--state-path" in result.stdout
    assert "--output-dir" in result.stdout
    assert "--provenance-log" in result.stdout
    assert "import" not in result.stdout.lower() or "production" not in result.stdout.lower()


def test_article_report_writes_report_diagnostics_provenance_and_freshness(tmp_path: Path) -> None:
    output_dir = tmp_path / "article-report"
    manifest = FIXTURES_DIR / "ten_document_manifest.json"

    result = _run_cli(
        "validation-batch",
        "article-report",
        "--manifest-path",
        str(manifest),
        "--output-dir",
        str(output_dir),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    response = json.loads(result.stdout)
    assert response["status"] == "article_report_written"
    assert response["recommendation"] == "proceed_to_20_document_scale_review_only"
    assert response["ready_document_count"] == 10
    assert response["blocked_document_count"] == 0
    assert response["freshness_verdict"] == "fresh"
    assert response["production_import_attempted"] is False
    assert response["ladybugdb_written"] is False

    report = _load_json(Path(response["report_path"]))
    diagnostics = Path(response["diagnostics_path"]).read_text(encoding="utf-8")
    freshness = _load_json(Path(response["freshness_report_path"]))
    provenance_lines = Path(response["provenance_log_path"]).read_text(encoding="utf-8").splitlines()

    assert report["schema_version"] == "m024-article-batch-validation.v1"
    assert report["runner"]["command"] == "validation-batch article-report"
    assert report["safety_counters"]["graph_import_attempted_count"] == 0
    assert diagnostics == ""
    assert freshness["verdict"] == "fresh"
    assert len(provenance_lines) == 1
    provenance = json.loads(provenance_lines[0])
    assert provenance["command"] == "validation-batch article-report"
    assert provenance["real_source_acquisition_performed"] is False
    assert provenance["real_scan_performed"] is False
    _assert_redacted(result.stdout + json.dumps(report) + diagnostics + json.dumps(freshness) + json.dumps(provenance))


def test_article_report_adapts_validation_batch_state_metadata_only(tmp_path: Path) -> None:
    state = ValidationBatchState(
        batch_id="state-batch",
        phase="initialized",
        selected_papers=tuple(
            SelectedPaper(
                paper_id=f"2605.{index:05d}v1",
                selection_role="deterministic_expansion",
                rank=index,
                source_paths={
                    "research_full_text_md": f"/metadata-only/{index}.md",
                    "source_sha256": "a" * 64,
                },
            )
            for index in range(10)
        ),
        input_manifests=("manifest.json",),
        artifact_paths=ScanArtifactPaths(),
        source_readiness_by_paper={
            f"2605.{index:05d}v1": SourceReadiness(markdown_present=True, markdown_quality_accepted=True, ready_for_markdown_scan=True)
            for index in range(10)
        },
    )
    state_path = write_batch_state(state, tmp_path / "batch-state.json")

    result = _run_cli(
        "validation-batch",
        "article-report",
        "--state-path",
        str(state_path),
        "--output-dir",
        str(tmp_path / "out"),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    response = json.loads(result.stdout)
    report = _load_json(Path(response["report_path"]))
    assert response["ready_document_count"] == 10
    assert report["runner"]["source_kind"] == "state"
    assert {row["source_sha256"] for row in report["document_status_rows"]} == {"a" * 64}


def test_article_report_missing_manifest_fails_with_redacted_blocked_artifacts(tmp_path: Path) -> None:
    missing = tmp_path / "missing.json"
    result = _run_cli(
        "validation-batch",
        "article-report",
        "--manifest-path",
        str(missing),
        "--output-dir",
        str(tmp_path / "out"),
        "--json",
    )

    assert result.returncode == 1
    response = json.loads(result.stdout)
    assert response["status"] == "blocked_report_written"
    assert response["recommendation"] == "repeat_10_document_batch_after_repairs"
    report = _load_json(Path(response["report_path"]))
    assert "empty_batch" in {diagnostic["code"] for diagnostic in report["diagnostics"]}
    assert response["production_import_attempted"] is False
    assert response["ladybugdb_written"] is False
    _assert_redacted(result.stdout + result.stderr + json.dumps(report))


def test_article_report_malformed_json_fails_without_raw_payload_echo(tmp_path: Path) -> None:
    malformed = tmp_path / "malformed.json"
    malformed.write_text('{"documents": ["FORBIDDEN_RAW_ARTICLE_TEXT_DO_NOT_ECHO", ', encoding="utf-8")

    result = _run_cli(
        "validation-batch",
        "article-report",
        "--manifest-path",
        str(malformed),
        "--output-dir",
        str(tmp_path / "out"),
        "--json",
    )

    assert result.returncode == 1
    response = json.loads(result.stdout)
    report = _load_json(Path(response["report_path"]))
    assert response["status"] == "blocked_report_written"
    assert report["aggregate_diagnostics"]["blocked_document_count"] == 0
    _assert_redacted(result.stdout + result.stderr + json.dumps(report))


def test_article_report_unsafe_manifest_is_redacted_and_fail_closed(tmp_path: Path) -> None:
    result = _run_cli(
        "validation-batch",
        "article-report",
        "--manifest-path",
        str(FIXTURES_DIR / "unsafe_document_manifest.json"),
        "--output-dir",
        str(tmp_path / "out"),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    response = json.loads(result.stdout)
    report = _load_json(Path(response["report_path"]))
    diagnostics_text = Path(response["diagnostics_path"]).read_text(encoding="utf-8")
    assert response["recommendation"] == "stop_graph_import_unsafe_evidence"
    assert report["safety_counters"]["graph_import_attempted_count"] == 0
    assert report["safety_flags"]["trusted_kg_import_allowed"] is False
    assert "forbidden_payload_key:raw_text" in {diagnostic["code"] for diagnostic in report["diagnostics"]}
    _assert_redacted(result.stdout + result.stderr + json.dumps(report) + diagnostics_text)


def test_article_report_rejects_ambiguous_source_options(tmp_path: Path) -> None:
    result = _run_cli(
        "validation-batch",
        "article-report",
        "--manifest-path",
        str(FIXTURES_DIR / "ten_document_manifest.json"),
        "--state-path",
        str(tmp_path / "state.json"),
        "--output-dir",
        str(tmp_path / "out"),
        "--json",
    )

    assert result.returncode == 2
    payload = json.loads(result.stdout)
    assert payload["status"] == "invalid_article_report_request"
    assert payload["production_import_attempted"] is False
    assert payload["ladybugdb_written"] is False
