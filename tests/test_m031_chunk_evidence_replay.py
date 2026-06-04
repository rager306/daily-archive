from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from replay_m031_chunk_evidence import main, sha256_file  # noqa: E402


def _write_json(path: Path, payload: dict[str, Any]) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return path


def _write_text(path: Path, text: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


def _converted_markdown() -> str:
    return "\n".join(
        [
            "# Local Parser Ready Paper",
            "",
            "## Abstract",
            "",
            "This local parser ready artifact describes bounded replay evidence, deterministic structure parsing, and review only chunk routing.",
            "",
            "## Method",
            "",
            "The method keeps provenance identifiers, source spans, section hierarchy, and conservative route blockers without graph writes.",
            "",
            "## Results",
            "",
            "The result is a metadata only package with chunk diagnostics and explicit refusal states for import readiness review.",
        ]
    )


def _row(
    *,
    identity: str,
    role: str,
    status: str,
    parser_ready: bool = False,
    converted_path: Path | str | None = None,
    converted_sha256: str | None = None,
    converted_size: int = 0,
    diagnostic_code: str | None = None,
) -> dict[str, Any]:
    return {
        "identity": identity,
        "article_ref": "arxiv/cs-cl/2507.19457" if identity == "arxiv:2507.19457" else identity.replace(":", "/"),
        "article_key": identity.rsplit(":", 1)[-1],
        "variant_id": f"{identity}:source:{role}",
        "source_role": role,
        "status": status,
        "terminal_state": status,
        "diagnostic_code": diagnostic_code or ("parser_ready_converted_text" if parser_ready else f"{status}_refused"),
        "refusal_code": None if parser_ready else diagnostic_code or f"{status}_refused",
        "failure_reason": None if parser_ready else "fixture refusal",
        "converted_text_path": converted_path.as_posix() if isinstance(converted_path, Path) else converted_path,
        "converted_text_sha256": converted_sha256,
        "converted_text_byte_size": converted_size,
        "parser_ready": parser_ready,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "fail_closed_safety_flags": {
            "network_fetch_attempted": False,
            "graph_import_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }


def _fixture(tmp_path: Path) -> tuple[Path, Path, Path, Path, Path, dict[str, Any], dict[str, Any]]:
    project = tmp_path / "project"
    corpus = project / "data" / "article_corpora" / "m031-catalog-backed-replay-v1"
    converted = _write_text(corpus / "conversion-quality" / "converted-text" / "arxiv_cs-cl_2507.19457" / "arxiv_pdf.txt", _converted_markdown())
    converted_hash = sha256_file(converted)
    rows = [
        _row(identity="arxiv:2507.19457", role="arxiv_html", status="low_quality", diagnostic_code="converted_text_low_quality"),
        _row(
            identity="arxiv:2507.19457",
            role="arxiv_pdf",
            status="converted",
            parser_ready=True,
            converted_path=converted,
            converted_sha256=converted_hash,
            converted_size=converted.stat().st_size,
        ),
        _row(identity="arxiv:2507.19457", role="arxiv_abs_page", status="metadata_only", diagnostic_code="metadata_only_refused"),
        _row(identity="stanford:cs224n:gradient-notes", role="external_pdf", status="blocked", diagnostic_code="catalog_pdf_missing"),
        _row(identity="arxiv:2605.29548", role="arxiv_abs_page", status="metadata_only", diagnostic_code="metadata_only_refused"),
        _row(identity="arxiv:2605.29548", role="arxiv_pdf", status="blocked", diagnostic_code="missing_source_artifact"),
        _row(identity="arxiv:2605.26099", role="arxiv_abs_url", status="blocked", diagnostic_code="catalog_placeholder_pruned_no_article_record"),
    ]
    selection = _write_json(
        corpus / "selection.json",
        {
            "schema_version": "m031-catalog-backed-replay-selection.v1",
            "selection_id": "m031-catalog-backed-replay-v1",
            "milestone_id": "M031-vwpd8e",
        },
    )
    conversion_summary = {
        "schema_version": "m031-parser-conversion-replay.v1",
        "milestone_id": "M031-vwpd8e",
        "slice_id": "S03",
        "selection_id": "m031-catalog-backed-replay-v1",
        "row_count": 7,
        "parser_ready_count": 1,
        "counts": {"blocked": 3, "converted": 1, "low_quality": 1, "metadata_only": 2},
        "results": rows,
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }
    conversion_summary_path = _write_json(corpus / "conversion-quality" / "conversion-quality-summary.json", conversion_summary)
    closeout = {
        "schema_version": "m031-parser-conversion-closeout-verifier.v1",
        "milestone_id": "M031-vwpd8e",
        "slice_id": "S03",
        "selection_id": "m031-catalog-backed-replay-v1",
        "status": "passed",
        "failure_count": 0,
        "row_count": 7,
        "parser_ready_count": 1,
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "fail_closed_safety_flags": {
            "network_fetch_attempted": False,
            "graph_import_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }
    closeout_path = _write_json(corpus / "parser-conversion-closeout-summary.json", closeout)
    output = corpus / "chunk-evidence"
    return project, selection, conversion_summary_path, closeout_path, output, conversion_summary, closeout


def _run(project: Path, selection: Path, conversion_summary: Path, closeout: Path, output: Path) -> int:
    return main(
        [
            "--selection",
            selection.as_posix(),
            "--conversion-summary",
            conversion_summary.as_posix(),
            "--closeout-summary",
            closeout.as_posix(),
            "--output-dir",
            output.as_posix(),
            "--project-root",
            project.as_posix(),
        ]
    )


def test_chunk_evidence_replay_writes_one_package_and_six_zero_chunk_refusals(tmp_path: Path) -> None:
    project, selection, conversion_summary, closeout, output, _summary_payload, _closeout_payload = _fixture(tmp_path)

    assert _run(project, selection, conversion_summary, closeout, output) == 0

    summary = json.loads((output / "chunk-evidence-summary.json").read_text(encoding="utf-8"))
    diagnostics = [json.loads(line) for line in (output / "chunk-evidence-diagnostics.jsonl").read_text(encoding="utf-8").splitlines()]
    report = (output / "chunk-evidence-report.md").read_text(encoding="utf-8")
    structure_package = json.loads((output / "packages" / "arxiv_cs-cl_2507.19457_arxiv_pdf" / "structure-aware-package.json").read_text(encoding="utf-8"))
    graph_package = json.loads((output / "packages" / "arxiv_cs-cl_2507.19457_arxiv_pdf" / "graph-readiness-package.json").read_text(encoding="utf-8"))

    assert summary["row_count"] == 7
    assert summary["chunked_parser_ready_row_count"] == 1
    assert summary["zero_chunk_refusal_count"] == 6
    assert summary["counts"] == {"chunked": 1, "zero_chunk_refused": 6}
    assert summary["package_count"] == 1
    assert summary["graph_readiness_package_count"] == 1
    assert summary["pending_graph_readiness_review_count"] == 1
    assert summary["import_eligible_chunk_count"] == 0
    assert summary["graph_import_allowed"] is False
    assert summary["trusted_kg_import_allowed"] is False
    assert summary["production_import_attempted"] is False
    assert summary["ladybugdb_written"] is False
    assert all(row["chunk_count"] == 0 for row in diagnostics if row["status"] == "zero_chunk_refused")
    assert [row for row in diagnostics if row["status"] == "chunked"][0]["chunk_count"] > 0
    assert structure_package["paper_id"] == "arxiv_cs-cl_2507.19457_arxiv_pdf"
    assert structure_package["diagnostics"]["counts_by_state"]
    assert graph_package["review_state"] == "pending_independent_graph_readiness_review"
    assert graph_package["output_contract_completed"] is False
    assert graph_package["validation"]["import_ready"] is False
    assert "Local Parser Ready Paper" not in json.dumps(summary)
    assert "Local Parser Ready Paper" not in json.dumps(diagnostics)
    assert "Local Parser Ready Paper" not in json.dumps(structure_package)
    assert "Local Parser Ready Paper" not in json.dumps(graph_package)
    assert "metadata-only" in report


@pytest.mark.parametrize(
    ("mutate", "expected_code"),
    [
        ("hash", "converted_text_sha256_mismatch"),
        ("outside_path", "converted_text_path_outside_project"),
        ("html_parser_ready", "non_pdf_parser_ready_refused"),
        ("stale_closeout", "s03_closeout_not_passed"),
        ("missing_converted", "missing_converted_text_artifact"),
        ("payload_marker", "metadata_payload_snippet_leakage"),
    ],
)
def test_chunk_evidence_replay_negative_fail_closed_cases(tmp_path: Path, capsys: pytest.CaptureFixture[str], mutate: str, expected_code: str) -> None:
    project, selection, conversion_summary_path, closeout_path, output, conversion_summary, closeout = _fixture(tmp_path)
    rows = conversion_summary["results"]
    parser_row = rows[1]
    if mutate == "hash":
        parser_row["converted_text_sha256"] = "0" * 64
        _write_json(conversion_summary_path, conversion_summary)
    elif mutate == "outside_path":
        outside = _write_text(tmp_path / "outside.txt", _converted_markdown())
        parser_row["converted_text_path"] = outside.as_posix()
        parser_row["converted_text_sha256"] = sha256_file(outside)
        parser_row["converted_text_byte_size"] = outside.stat().st_size
        _write_json(conversion_summary_path, conversion_summary)
    elif mutate == "html_parser_ready":
        rows[0]["status"] = "converted"
        rows[0]["parser_ready"] = True
        rows[0]["converted_text_path"] = parser_row["converted_text_path"]
        rows[0]["converted_text_sha256"] = parser_row["converted_text_sha256"]
        rows[0]["converted_text_byte_size"] = parser_row["converted_text_byte_size"]
        conversion_summary["parser_ready_count"] = 2
        closeout["parser_ready_count"] = 2
        _write_json(conversion_summary_path, conversion_summary)
        _write_json(closeout_path, closeout)
    elif mutate == "stale_closeout":
        closeout["status"] = "failed"
        closeout["failure_count"] = 1
        _write_json(closeout_path, closeout)
    elif mutate == "missing_converted":
        Path(parser_row["converted_text_path"]).unlink()
    elif mutate == "payload_marker":
        parser_row["identity"] = "deterministic fallback capture"
        _write_json(conversion_summary_path, conversion_summary)

    assert _run(project, selection, conversion_summary_path, closeout_path, output) == 2
    captured = capsys.readouterr()
    assert expected_code in captured.err
    assert not (output / "packages" / "arxiv_cs-cl_2507.19457_arxiv_pdf" / "structure-aware-package.json").exists()


def test_chunk_evidence_replay_rejects_permissive_closeout_flags(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    project, selection, conversion_summary_path, closeout_path, output, _conversion_summary, closeout = _fixture(tmp_path)
    closeout["fail_closed_safety_flags"]["graph_import_allowed"] = True
    _write_json(closeout_path, closeout)

    assert _run(project, selection, conversion_summary_path, closeout_path, output) == 2

    captured = capsys.readouterr()
    assert "unsafe_closeout_flag" in captured.err
    assert not (output / "chunk-evidence-summary.json").exists()
