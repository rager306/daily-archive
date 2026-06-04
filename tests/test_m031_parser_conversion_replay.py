from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

fitz = pytest.importorskip("fitz")

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from replay_m031_parser_conversion import main, sha256_file  # noqa: E402
from verify_m031_parser_conversion_replay import main as verify_main, verify  # noqa: E402


def _write(path: Path, value: str | bytes) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(value, bytes):
        path.write_bytes(value)
    else:
        path.write_text(value, encoding="utf-8")
    return path


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _pdf_bytes(text: str) -> bytes:
    document = fitz.open()
    page = document.new_page()
    page.insert_textbox(fitz.Rect(72, 72, 520, 760), text, fontsize=11)
    payload = document.tobytes()
    document.close()
    return payload


def _loader_row(
    source_root: Path,
    *,
    identity: str,
    article_ref: str | None,
    role: str,
    local_path: str | None,
    data: str | bytes | None,
    status: str = "loaded",
    diagnostic_code: str = "loader_loaded",
    is_metadata_only: bool = False,
    media_type: str | None = None,
) -> dict[str, Any]:
    sha256: str | None = None
    byte_size: int | None = None
    if local_path is not None and data is not None:
        artifact = _write(source_root / local_path, data)
        sha256 = sha256_file(artifact)
        byte_size = artifact.stat().st_size
    return {
        "schema_version": "m031-catalog-backed-loader-evidence.v1",
        "milestone_id": "M031-vwpd8e",
        "slice_id": "S02",
        "selection_id": "m031-catalog-backed-replay-v1",
        "identity": identity,
        "article_ref": article_ref,
        "article_key": article_ref.rsplit("/", 1)[-1] if article_ref else None,
        "variant_id": f"{identity}:{role}",
        "source_role": role,
        "status": status,
        "terminal_state": status,
        "diagnostic_code": diagnostic_code,
        "blocker_code": diagnostic_code if status == "blocked" else None,
        "failure_reason": None if status != "blocked" else "fixture blocker",
        "local_path": local_path,
        "safe_local_paths": [local_path] if local_path else [],
        "media_type": media_type or ("application/pdf" if role.endswith("pdf") else "text/html"),
        "sha256": sha256,
        "byte_size": byte_size,
        "is_metadata_only": is_metadata_only,
        "requires_conversion": not is_metadata_only,
        "loader_attempted": status != "blocked",
        "text_present": status == "loaded",
        "network_fetch_attempted": False,
        "graph_import_allowed": False,
        "production_ladybugdb_write_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }


def _write_inputs(tmp_path: Path) -> tuple[Path, Path, Path, Path]:
    corpus = tmp_path / "corpus"
    source_root = corpus / "source"
    selection = corpus / "selection.json"
    loader_summary = corpus / "loader-evidence-summary.json"
    output_dir = corpus / "conversion-quality"
    long_pdf_text = (
        "This fixture PDF contains enough local scientific prose for parser conversion replay. "
        "It includes several sentences about evaluation boundaries, local-only extraction, and "
        "stable hash evidence so the parser-ready branch writes a converted text artifact. "
        "No network fetches or graph writes should be needed for this conversion contract."
    )
    raw_html = "<html><body><article><p>tiny fallback stub</p></article></body></html>"
    rows = [
        _loader_row(
            source_root,
            identity="arxiv:2507.19457",
            article_ref="arxiv/cs-cl/2507.19457",
            role="arxiv_pdf",
            local_path="arxiv/cs-cl/2507.19457/source/original.pdf",
            data=_pdf_bytes(long_pdf_text),
            status="loaded_metadata_only",
            diagnostic_code="loader_loaded_metadata_only",
        ),
        _loader_row(
            source_root,
            identity="arxiv:2507.19457",
            article_ref="arxiv/cs-cl/2507.19457",
            role="arxiv_html",
            local_path="arxiv/cs-cl/2507.19457/source/article.html",
            data=raw_html,
        ),
        _loader_row(
            source_root,
            identity="arxiv:2605.29548",
            article_ref="arxiv/mixed-source/2605.29548",
            role="arxiv_abs_page",
            local_path=None,
            data=None,
            status="blocked",
            diagnostic_code="missing_local_source_path",
            is_metadata_only=True,
        ),
        _loader_row(
            source_root,
            identity="arxiv:2605.26099",
            article_ref=None,
            role="arxiv_abs_url",
            local_path=None,
            data=None,
            status="blocked",
            diagnostic_code="catalog_placeholder_pruned_no_article_record",
        ),
        _loader_row(
            source_root,
            identity="unsafe:path",
            article_ref="unsafe/path",
            role="arxiv_pdf",
            local_path="../escape.pdf",
            data=None,
            status="loaded",
            diagnostic_code="loader_loaded",
            media_type="application/pdf",
        ),
        _loader_row(
            source_root,
            identity="missing:source",
            article_ref="missing/source",
            role="arxiv_pdf",
            local_path="missing/source/original.pdf",
            data=None,
            status="loaded",
            diagnostic_code="loader_loaded",
            media_type="application/pdf",
        ),
    ]
    _write_json(
        selection,
        {
            "schema_version": "m031-catalog-backed-replay-selection.v1",
            "selection_id": "m031-catalog-backed-replay-v1",
            "milestone_id": "M031-vwpd8e",
            "slice_id": "S02",
            "requested_refs": [],
            "articles": [],
            "catalog_blockers": [],
        },
    )
    _write_json(
        loader_summary,
        {
            "schema_version": "m031-catalog-backed-loader-evidence.v1",
            "milestone_id": "M031-vwpd8e",
            "slice_id": "S02",
            "selection_id": "m031-catalog-backed-replay-v1",
            "status": "completed_with_diagnostics",
            "results": rows,
        },
    )
    return selection, loader_summary, source_root, output_dir


def _materialize(tmp_path: Path) -> tuple[Path, Path, Path, Path]:
    selection, loader_summary, source_root, output_dir = _write_inputs(tmp_path)
    assert main(["--selection", str(selection), "--loader-summary", str(loader_summary), "--source-dir", str(source_root), "--output-dir", str(output_dir)]) == 0
    return selection, loader_summary, source_root, output_dir


def _run(tmp_path: Path) -> tuple[dict[str, Any], list[dict[str, Any]], str, Path]:
    _selection, _loader_summary, _source_root, output_dir = _materialize(tmp_path)
    summary = json.loads((output_dir / "conversion-quality-summary.json").read_text(encoding="utf-8"))
    diagnostics = [json.loads(line) for line in (output_dir / "conversion-quality-diagnostics.jsonl").read_text(encoding="utf-8").splitlines()]
    report = (output_dir / "conversion-quality-report.md").read_text(encoding="utf-8")
    return summary, diagnostics, report, output_dir


def _verify_args(selection: Path, loader_summary: Path, output_dir: Path, corpus: Path) -> list[str]:
    return [
        "--selection",
        str(selection),
        "--loader-summary",
        str(loader_summary),
        "--conversion-summary",
        str(output_dir / "conversion-quality-summary.json"),
        "--conversion-diagnostics",
        str(output_dir / "conversion-quality-diagnostics.jsonl"),
        "--conversion-report",
        str(output_dir / "conversion-quality-report.md"),
        "--converted-text-dir",
        str(output_dir / "converted-text"),
        "--write-summary",
        str(corpus / "parser-conversion-closeout-summary.json"),
        "--write-diagnostics",
        str(corpus / "parser-conversion-closeout-diagnostics.jsonl"),
        "--write-report",
        str(corpus / "parser-conversion-closeout-report.md"),
    ]


def _diagnostic_codes(findings: list[dict[str, Any]]) -> set[str]:
    return {str(row.get("diagnostic_code") or row.get("code")) for row in findings}


def _row(summary: dict[str, Any], role: str, identity: str | None = None) -> dict[str, Any]:
    matches = [row for row in summary["results"] if row["source_role"] == role and (identity is None or row["identity"] == identity)]
    assert len(matches) == 1
    return matches[0]


def test_parser_conversion_replay_writes_hashed_converted_text_and_fail_closed_metadata(tmp_path: Path) -> None:
    summary, diagnostics, report, output_dir = _run(tmp_path)

    pdf_row = _row(summary, "arxiv_pdf", "arxiv:2507.19457")
    assert pdf_row["status"] == "converted"
    assert pdf_row["parser_ready"] is True
    converted_path = Path(pdf_row["converted_text_path"])
    assert converted_path.exists()
    assert converted_path.is_relative_to(output_dir)
    assert pdf_row["converted_text_sha256"] == sha256_file(converted_path)
    assert pdf_row["converted_text_byte_size"] == converted_path.stat().st_size
    assert pdf_row["source_sha256_verified"] is True
    assert pdf_row["source_byte_size_verified"] is True

    assert summary["counts"] == {"blocked": 3, "converted": 1, "low_quality": 1, "metadata_only": 1}
    assert summary["parser_ready_count"] == 1
    assert summary["network_fetch_attempted"] is False
    assert summary["graph_import_allowed"] is False
    assert summary["trusted_kg_import_allowed"] is False
    assert summary["production_import_attempted"] is False
    assert summary["ladybugdb_written"] is False
    assert all(value is False for value in summary["fail_closed_safety_flags"].values())
    assert all(value is False for row in summary["results"] for value in row["fail_closed_safety_flags"].values())
    assert {diag["code"] for diag in diagnostics} >= {
        "parser_ready_converted_text",
        "converted_text_low_quality",
        "metadata_only_refused",
        "catalog_placeholder_pruned_no_article_record",
        "unsafe_relative_path",
        "missing_source_artifact",
    }
    for diag in diagnostics:
        assert {"code", "severity", "json_path", "identity", "article_ref", "source_role", "safe_path"} <= set(diag)
    assert "## Failure Modes" in report
    assert "## Load Profile" in report
    assert "## Negative Tests" in report


def test_parser_conversion_replay_refuses_low_quality_metadata_blocked_unsafe_and_missing_rows(tmp_path: Path) -> None:
    summary, _diagnostics, _report, _output_dir = _run(tmp_path)

    low_quality = _row(summary, "arxiv_html")
    assert low_quality["status"] == "low_quality"
    assert low_quality["parser_ready"] is False
    assert low_quality["diagnostic_code"] == "converted_text_low_quality"
    assert low_quality["converted_text_path"] is None

    metadata_only = _row(summary, "arxiv_abs_page")
    assert metadata_only["status"] == "metadata_only"
    assert metadata_only["refusal_code"] == "metadata_only_refused"
    assert metadata_only["parser_ready"] is False

    blocker = _row(summary, "arxiv_abs_url")
    assert blocker["status"] == "blocked"
    assert blocker["refusal_code"] == "catalog_placeholder_pruned_no_article_record"
    assert blocker["parser_ready"] is False

    unsafe = _row(summary, "arxiv_pdf", "unsafe:path")
    assert unsafe["status"] == "blocked"
    assert unsafe["diagnostic_code"] == "unsafe_relative_path"
    assert unsafe["safe_path"] is None

    missing = _row(summary, "arxiv_pdf", "missing:source")
    assert missing["status"] == "blocked"
    assert missing["diagnostic_code"] == "missing_source_artifact"
    assert missing["safe_path"] == "missing/source/original.pdf"


def test_parser_conversion_replay_refuses_real_fallback_html_stub_and_removes_stale_converted_text(tmp_path: Path) -> None:
    selection, loader_summary, source_root, output_dir = _write_inputs(tmp_path)
    fallback_path = source_root / "arxiv/cs-cl/2507.19457/source/article.html"
    fallback_path.write_text(
        "<html><body><h1>GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning</h1>"
        "<p>Deterministic fallback capture for arxiv_html; live acquisition failed with HTTPError.</p>"
        "<p>Canonical URL: https://arxiv.org/abs/2507.19457</p>"
        "<p>This local source preserves the smoke-corpus loader contract without embedding payload text in metadata.</p>"
        "</body></html>",
        encoding="utf-8",
    )
    loader_payload = json.loads(loader_summary.read_text(encoding="utf-8"))
    html_row = next(row for row in loader_payload["results"] if row["source_role"] == "arxiv_html")
    html_row["sha256"] = sha256_file(fallback_path)
    html_row["byte_size"] = fallback_path.stat().st_size
    _write_json(loader_summary, loader_payload)
    stale = output_dir / "converted-text" / "arxiv_cs-cl_2507.19457" / "arxiv_html.txt"
    _write(stale, "stale parser-ready html should be removed")

    assert main(["--selection", str(selection), "--loader-summary", str(loader_summary), "--source-dir", str(source_root), "--output-dir", str(output_dir)]) == 0
    summary = json.loads((output_dir / "conversion-quality-summary.json").read_text(encoding="utf-8"))
    html = _row(summary, "arxiv_html")

    assert html["status"] == "low_quality"
    assert html["diagnostic_code"] == "converted_text_low_quality"
    assert html["parser_ready"] is False
    assert html["bounded_extraction"]["fallback_stub_detected"] == 1
    assert not stale.exists()


def test_parser_conversion_replay_metadata_and_reports_do_not_embed_raw_payloads(tmp_path: Path) -> None:
    summary, _diagnostics, report, output_dir = _run(tmp_path)
    metadata = (output_dir / "conversion-quality-summary.json").read_text(encoding="utf-8")
    metadata += (output_dir / "conversion-quality-diagnostics.jsonl").read_text(encoding="utf-8")
    metadata += report

    assert "tiny fallback stub" not in metadata
    assert "This fixture PDF contains enough local scientific prose" not in metadata
    assert "<html" not in metadata.lower()
    assert "</html" not in metadata.lower()
    assert "%PDF-" not in metadata
    assert "base64," not in metadata.lower()
    assert all(row["converted_text_path"] is None for row in summary["results"] if row["parser_ready"] is not True)


def test_parser_conversion_replay_absent_pymupdf_is_diagnostic_not_success(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    import replay_m031_parser_conversion as replay

    selection, loader_summary, source_root, output_dir = _write_inputs(tmp_path)
    monkeypatch.setattr(replay, "fitz", None)

    assert replay.main(["--selection", str(selection), "--loader-summary", str(loader_summary), "--source-dir", str(source_root), "--output-dir", str(output_dir)]) == 0
    summary = json.loads((output_dir / "conversion-quality-summary.json").read_text(encoding="utf-8"))
    pdf_row = _row(summary, "arxiv_pdf", "arxiv:2507.19457")
    assert pdf_row["status"] == "blocked"
    assert pdf_row["diagnostic_code"] == "pymupdf_unavailable"
    assert pdf_row["parser_ready"] is False


def test_parser_conversion_replay_malformed_json_returns_cli_diagnostic(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    selection = tmp_path / "selection.json"
    loader_summary = tmp_path / "loader.json"
    source_root = tmp_path / "source"
    source_root.mkdir()
    selection.write_text("{not json", encoding="utf-8")
    _write_json(loader_summary, {"results": []})

    exit_code = main(["--selection", str(selection), "--loader-summary", str(loader_summary), "--source-dir", str(source_root), "--output-dir", str(tmp_path / "out")])

    assert exit_code == 2
    stderr = capsys.readouterr().err
    assert "malformed_json" in stderr


def test_parser_conversion_closeout_verifier_writes_metadata_only_audit_surface(tmp_path: Path) -> None:
    selection, loader_summary, _source_root, output_dir = _materialize(tmp_path)
    corpus = tmp_path / "corpus"

    summary, findings = verify(_verify_args(selection, loader_summary, output_dir, corpus))

    assert findings == []
    assert summary["status"] == "passed"
    assert summary["failure_count"] == 0
    assert summary["row_count"] == 6
    assert summary["parser_ready_count"] == 1
    assert (corpus / "parser-conversion-closeout-summary.json").exists()
    assert (corpus / "parser-conversion-closeout-diagnostics.jsonl").exists()
    report = (corpus / "parser-conversion-closeout-report.md").read_text(encoding="utf-8")
    assert "## Failure Modes" in report
    assert "## Load Profile" in report
    assert "## Negative Tests" in report
    closeout_metadata = (corpus / "parser-conversion-closeout-summary.json").read_text(encoding="utf-8") + report
    assert "This fixture PDF contains enough local scientific prose" not in closeout_metadata
    assert "<html" not in closeout_metadata.lower()
    assert "%PDF-" not in closeout_metadata


def test_parser_conversion_closeout_verifier_cli_fails_on_findings(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    selection, loader_summary, _source_root, output_dir = _materialize(tmp_path)
    summary = json.loads((output_dir / "conversion-quality-summary.json").read_text(encoding="utf-8"))
    summary["network_fetch_attempted"] = True
    _write_json(output_dir / "conversion-quality-summary.json", summary)

    exit_code = verify_main(_verify_args(selection, loader_summary, output_dir, tmp_path / "corpus"))

    assert exit_code == 1
    assert "failed" in capsys.readouterr().out


def test_parser_conversion_closeout_verifier_flags_mutated_converted_hash_and_missing_text(tmp_path: Path) -> None:
    selection, loader_summary, _source_root, output_dir = _materialize(tmp_path)
    summary = json.loads((output_dir / "conversion-quality-summary.json").read_text(encoding="utf-8"))
    converted = Path(next(row for row in summary["results"] if row["parser_ready"] is True)["converted_text_path"])
    converted.write_text("tampered converted text", encoding="utf-8")

    _summary, findings = verify(_verify_args(selection, loader_summary, output_dir, tmp_path / "corpus"))
    assert {"converted_text_sha256_mismatch", "converted_text_byte_size_mismatch"} <= _diagnostic_codes(findings)

    converted.unlink()
    _summary, findings = verify(_verify_args(selection, loader_summary, output_dir, tmp_path / "corpus2"))
    assert "missing_converted_text_artifact" in _diagnostic_codes(findings)


def test_parser_conversion_closeout_verifier_flags_unsafe_path_and_payload_leakage(tmp_path: Path) -> None:
    selection, loader_summary, _source_root, output_dir = _materialize(tmp_path)
    summary_path = output_dir / "conversion-quality-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["results"][0]["safe_path"] = "../escape.html"
    summary["results"][0]["raw_text"] = "RAW_PDF_SECRET <html"
    _write_json(summary_path, summary)
    report_path = output_dir / "conversion-quality-report.md"
    report_path.write_text(report_path.read_text(encoding="utf-8") + "\nbase64,RAW\n", encoding="utf-8")

    _summary, findings = verify(_verify_args(selection, loader_summary, output_dir, tmp_path / "corpus"))

    codes = _diagnostic_codes(findings)
    assert "unsafe_safe_path" in codes
    assert "metadata_payload_key_leakage" in codes
    assert "metadata_payload_snippet_leakage" in codes


def test_parser_conversion_closeout_verifier_flags_invalid_parser_ready_promotions_and_graph_flags(tmp_path: Path) -> None:
    selection, loader_summary, _source_root, output_dir = _materialize(tmp_path)
    summary_path = output_dir / "conversion-quality-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    html = next(row for row in summary["results"] if row["source_role"] == "arxiv_html")
    html["status"] = "converted"
    html["parser_ready"] = True
    html["diagnostic_code"] = "parser_ready_converted_text"
    abs_row = next(row for row in summary["results"] if row["source_role"] == "arxiv_abs_page")
    abs_row["status"] = "converted"
    abs_row["parser_ready"] = True
    abs_row["diagnostic_code"] = "parser_ready_converted_text"
    summary["graph_import_allowed"] = True
    summary["results"][0]["fail_closed_safety_flags"]["ladybugdb_written"] = True
    _write_json(summary_path, summary)

    _summary, findings = verify(_verify_args(selection, loader_summary, output_dir, tmp_path / "corpus"))

    codes = _diagnostic_codes(findings)
    assert "low_quality_html_parser_ready_claim" in codes
    assert "metadata_only_parser_ready_claim" in codes
    assert "unsafe_safety_flag_true" in codes
