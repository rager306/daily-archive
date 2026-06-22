from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

# pyrefly: ignore [missing-import]
import fitz  # ty:ignore[unresolved-import]
import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

# pyrefly: ignore [missing-import]
from convert_m027_source_quality_boundary import (  # noqa: E402  # ty:ignore[unresolved-import]
    CONVERTED_TEXT_DIR,
    FAIL_CLOSED_SAFETY_FLAGS,
    main,
    sha256_file,
)

# pyrefly: ignore [missing-import]
from verify_m027_conversion_quality_boundary import (
    main as verify_main,  # noqa: E402  # ty:ignore[unresolved-import]
)

FORBIDDEN_KEYS = {
    "text",
    "raw_text",
    "html",
    "pdf",
    "binary",
    "bytes",
    "base64",
    "payload",
    "content",
    "body",
}
FORBIDDEN_SNIPPETS = {
    "RAW_ARXIV_ABS_SECRET",
    "RAW_NATURE_BODY_SECRET",
    "RAW_PDF_SECRET",
    "%PDF-",
    "base64,",
}


def _write(path: Path, data: str | bytes) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, bytes):
        path.write_bytes(data)
    else:
        path.write_text(data, encoding="utf-8")
    return path


def _pdf_bytes(text: str) -> bytes:
    document = fitz.open()
    page = document.new_page()
    page.insert_textbox(fitz.Rect(72, 72, 520, 760), text, fontsize=11)
    data = document.tobytes()
    document.close()
    return data


def _captured_row(
    root: Path, article_ref: str, role: str, local_path: str, data: str | bytes
) -> dict[str, Any]:
    artifact = _write(root / article_ref / local_path, data)
    return {
        "schema_version": "m027-source-acquisition.v1",
        "milestone_id": "M027-aakeky",
        "slice_id": "S02",
        "selection_id": "m027-mixed-source-corpus-v1",
        "article_ref": article_ref,
        "variant_id": f"{article_ref.rsplit('/', 1)[-1]}:source:{role}",
        "source_role": role,
        "status": "captured",
        "diagnostic_code": "captured_source_artifact",
        "failure_reason": None,
        "local_path": local_path,
        "sha256": sha256_file(artifact),
        "byte_size": artifact.stat().st_size,
        "media_type": "application/pdf" if role.endswith("pdf") else "text/html",
        "network_fetch_attempted": True,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def _write_summary(path: Path, rows: list[dict[str, Any]]) -> Path:
    payload = {
        "schema_version": "m027-source-acquisition.v1",
        "milestone_id": "M027-aakeky",
        "slice_id": "S02",
        "selection_id": "m027-mixed-source-corpus-v1",
        "status": "captured",
        "results": rows,
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return path


def _run_boundary(
    tmp_path: Path, rows: list[dict[str, Any]]
) -> tuple[dict[str, Any], list[dict[str, Any]], str]:
    source_root = tmp_path / "corpus"
    summary_path = _write_summary(source_root / "source-acquisition-summary.json", rows)
    output_summary = source_root / "conversion-quality-summary.json"
    output_diagnostics = source_root / "conversion-quality-diagnostics.jsonl"
    output_report = source_root / "conversion-quality-report.md"
    exit_code = main(
        [
            "convert_m027_source_quality_boundary.py",
            "--source-summary",
            str(summary_path),
            "--source-root",
            str(source_root),
            "--converted-text-dir",
            str(source_root / "converted-text"),
            "--output-summary",
            str(output_summary),
            "--output-diagnostics",
            str(output_diagnostics),
            "--output-report",
            str(output_report),
        ]
    )
    assert exit_code == 0
    summary = json.loads(output_summary.read_text(encoding="utf-8"))
    diagnostics = [
        json.loads(line) for line in output_diagnostics.read_text(encoding="utf-8").splitlines()
    ]
    report = output_report.read_text(encoding="utf-8")
    return summary, diagnostics, report


def _verify_boundary(tmp_path: Path, *, article_count: int, variant_count: int) -> int:
    source_root = tmp_path / "corpus"
    return verify_main(
        [
            "verify_m027_conversion_quality_boundary.py",
            "--source-summary",
            str(source_root / "source-acquisition-summary.json"),
            "--summary",
            str(source_root / "conversion-quality-summary.json"),
            "--diagnostics",
            str(source_root / "conversion-quality-diagnostics.jsonl"),
            "--report",
            str(source_root / "conversion-quality-report.md"),
            "--source-root",
            str(source_root),
            "--corpus-dir",
            str(source_root),
            "--expected-article-count",
            str(article_count),
            "--expected-variant-count",
            str(variant_count),
        ]
    )


def _assert_metadata_redacted(*artifacts: Any) -> None:
    serialized = json.dumps(artifacts, sort_keys=True)
    for key in FORBIDDEN_KEYS:
        assert f'"{key}"' not in serialized
    for snippet in FORBIDDEN_SNIPPETS:
        assert snippet not in serialized
    assert '"network_fetch_attempted": true' not in serialized.lower()
    assert '"production_import_attempted": true' not in serialized.lower()
    assert '"ladybugdb_written": true' not in serialized.lower()


def _by_role(rows: list[dict[str, Any]], role: str) -> dict[str, Any]:
    matches = [row for row in rows if row.get("source_role") == role]
    assert len(matches) == 1
    return matches[0]


def test_default_payload_directory_matches_s03_handoff_contract() -> None:
    assert CONVERTED_TEXT_DIR.name == "conversion-quality"


def test_local_conversion_classifies_abs_pdf_fallback_and_nature_body(tmp_path: Path) -> None:
    source_root = tmp_path / "corpus"
    article_ref = "arxiv/mixed-source/2605.20897"
    nature_ref = "nature/mixed-source/s44387-025-00019-5"
    rows = [
        _captured_row(
            source_root,
            article_ref,
            "arxiv_abs_page",
            "source/abs.html",
            "<html><head><title>RAW_ARXIV_ABS_SECRET</title></head>"
            "<body><h1>Title</h1><blockquote class='abstract'>Only abstract metadata.</blockquote></body></html>",
        ),
        _captured_row(
            source_root,
            article_ref,
            "arxiv_pdf",
            "source/original.pdf",
            _pdf_bytes(
                "RAW_PDF_SECRET converted fallback text with enough local content for parser readiness. "
                "This bounded PyMuPDF fixture includes multiple scientific article sentences so the "
                "quality threshold represents substantive local conversion rather than a title-only page."
            ),
        ),
        _captured_row(
            source_root,
            nature_ref,
            "nature_html",
            "source/article.html",
            "<html><body><article><h1>Nature title</h1><p>RAW_NATURE_BODY_SECRET paragraph one "
            "with enough article content for local parser readiness diagnostics.</p><p>Second paragraph.</p>"
            "</article></body></html>",
        ),
    ]

    summary, diagnostics, report = _run_boundary(tmp_path, rows)

    abs_row = _by_role(diagnostics, "arxiv_abs_page")
    assert abs_row["status"] == "metadata_only"
    assert abs_row["diagnostic_code"] == "arxiv_abs_html_metadata_only"
    assert abs_row["parser_ready"] is False
    assert abs_row["converted_text_path"] is None
    assert abs_row["structure_counts"]["abstract_like_count"] == 1

    pdf_row = _by_role(diagnostics, "arxiv_pdf")
    assert pdf_row["status"] == "converted"
    assert pdf_row["diagnostic_code"] == "parser_ready_converted_text"
    assert pdf_row["parser_ready"] is True
    pdf_payload = Path(pdf_row["converted_text_path"])
    assert pdf_payload.exists()
    assert "RAW_PDF_SECRET" in pdf_payload.read_text(encoding="utf-8")
    assert pdf_row["converted_text_sha256"] == sha256_file(pdf_payload)
    assert pdf_row["structure_counts"]["page_count"] == 1

    nature_row = _by_role(diagnostics, "nature_html")
    assert nature_row["status"] == "converted"
    assert nature_row["diagnostic_code"] == "parser_ready_converted_text"
    assert nature_row["parser_ready"] is True
    nature_payload = Path(nature_row["converted_text_path"])
    assert "RAW_NATURE_BODY_SECRET" in nature_payload.read_text(encoding="utf-8")
    assert nature_row["structure_counts"]["article_tag_count"] == 1
    assert nature_row["structure_counts"]["paragraph_count"] == 2

    assert summary["parser_ready_count"] == 2
    assert summary["counts"] == {"converted": 2, "metadata_only": 1}
    assert summary["provenance"]["milestone_id"] == "M027-aakeky"
    assert summary["provenance"]["slice_id"] == "S03"
    assert summary["provenance"]["input_hashes"]
    assert summary["provenance"]["output_hashes"]
    assert "Failure Modes" in report
    assert "Load Profile" in report
    assert "Negative Tests" in report
    assert _verify_boundary(tmp_path, article_count=2, variant_count=3) == 0
    _assert_metadata_redacted(summary, diagnostics, report)


@pytest.mark.parametrize(
    ("field", "value", "expected_code"),
    [
        ("article_ref", "../escape", "unsafe_article_ref"),
        ("article_ref", "https://example.test/article", "url_not_allowed_as_article_ref"),
        ("local_path", "../source.html", "unsafe_local_path"),
        ("local_path", "https://example.test/source.html", "url_not_allowed_as_local_path"),
    ],
)
def test_unsafe_article_ref_or_local_path_is_blocked_without_conversion(
    tmp_path: Path, field: str, value: str, expected_code: str
) -> None:
    source_root = tmp_path / "corpus"
    row = _captured_row(
        source_root,
        "arxiv/mixed-source/2605.20897",
        "arxiv_pdf",
        "source/original.pdf",
        _pdf_bytes("safe fixture text that should not be opened for unsafe locator rows"),
    )
    row[field] = value

    summary, diagnostics, _report = _run_boundary(tmp_path, [row])

    assert summary["counts"] == {"blocked": 1}
    assert diagnostics[0]["status"] == "blocked"
    assert diagnostics[0]["diagnostic_code"] == expected_code
    assert diagnostics[0]["parser_ready"] is False
    assert diagnostics[0]["converted_text_path"] is None
    assert diagnostics[0]["network_fetch_attempted"] is False
    _assert_metadata_redacted(summary, diagnostics)


def test_missing_hash_mismatch_and_non_captured_rows_fail_closed(tmp_path: Path) -> None:
    source_root = tmp_path / "corpus"
    missing = {
        "article_ref": "arxiv/mixed-source/missing",
        "variant_id": "missing:source:arxiv-pdf",
        "source_role": "arxiv_pdf",
        "status": "captured",
        "local_path": "source/original.pdf",
        "sha256": "0" * 64,
        "byte_size": 10,
        "media_type": "application/pdf",
    }
    mismatch = _captured_row(
        source_root,
        "arxiv/mixed-source/mismatch",
        "arxiv_pdf",
        "source/original.pdf",
        _pdf_bytes("RAW_PDF_SECRET this file exists but recorded hash will be wrong"),
    )
    mismatch["sha256"] = "f" * 64
    blocked = dict(mismatch)
    blocked["article_ref"] = "arxiv/mixed-source/blocked"
    blocked["variant_id"] = "blocked:source:arxiv-pdf"
    blocked["status"] = "blocked"

    summary, diagnostics, _report = _run_boundary(tmp_path, [missing, mismatch, blocked])

    codes = {row["diagnostic_code"] for row in diagnostics}
    assert codes == {"missing_source_artifact", "source_sha256_mismatch", "source_not_captured"}
    assert summary["parser_ready_count"] == 0
    assert summary["counts"] == {"blocked": 3}
    for row in diagnostics:
        assert row["parser_ready"] is False
        assert row["converted_text_path"] is None
        assert row["fail_closed_safety_flags"]["graph_import_allowed"] is False
        assert row["fail_closed_safety_flags"]["production_ladybugdb_write_allowed"] is False
    _assert_metadata_redacted(summary, diagnostics)


def _write_conversion_artifacts(source_root: Path, summary: dict[str, Any]) -> None:
    (source_root / "conversion-quality-summary.json").write_text(
        json.dumps(summary, indent=2), encoding="utf-8"
    )
    (source_root / "conversion-quality-diagnostics.jsonl").write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in summary["results"]),
        encoding="utf-8",
    )


def _arxiv_abs_row(
    source_root: Path, article_ref: str = "arxiv/mixed-source/2605.20897"
) -> dict[str, Any]:
    return _captured_row(
        source_root,
        article_ref,
        "arxiv_abs_page",
        "source/abs.html",
        "<html><body><h1>Title</h1><blockquote class='abstract'>metadata only</blockquote></body></html>",
    )


def _arxiv_pdf_row(
    source_root: Path, article_ref: str = "arxiv/mixed-source/2605.20897"
) -> dict[str, Any]:
    return _captured_row(
        source_root,
        article_ref,
        "arxiv_pdf",
        "source/original.pdf",
        _pdf_bytes(
            "converted fallback text with enough local content for parser readiness. "
            "This fixture includes multiple scientific article sentences so quality passes."
        ),
    )


def test_conversion_verifier_fails_on_unsafe_converted_text_path(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    source_root = tmp_path / "corpus"
    summary, _diagnostics, _report = _run_boundary(
        tmp_path, [_arxiv_abs_row(source_root), _arxiv_pdf_row(source_root)]
    )
    pdf_row = _by_role(summary["results"], "arxiv_pdf")
    pdf_row["converted_text_path"] = "../escape.txt"
    _write_conversion_artifacts(source_root, summary)

    assert _verify_boundary(tmp_path, article_count=1, variant_count=2) == 1
    assert "unsafe_converted_text_path" in capsys.readouterr().err


def test_conversion_verifier_fails_on_stale_source_and_converted_hashes(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    source_root = tmp_path / "corpus"
    summary, _diagnostics, _report = _run_boundary(
        tmp_path, [_arxiv_abs_row(source_root), _arxiv_pdf_row(source_root)]
    )
    pdf_row = _by_role(summary["results"], "arxiv_pdf")
    Path(source_root / "arxiv/mixed-source/2605.20897/source/original.pdf").write_bytes(
        b"stale source bytes"
    )
    converted_path = Path(pdf_row["converted_text_path"])
    converted_path.write_text(
        converted_path.read_text(encoding="utf-8") + "\nstale converted text", encoding="utf-8"
    )

    assert _verify_boundary(tmp_path, article_count=1, variant_count=2) == 1
    stderr = capsys.readouterr().err
    assert "source_sha256_mismatch" in stderr
    assert "converted_text_sha256_mismatch" in stderr


def test_conversion_verifier_fails_on_metadata_payload_leakage(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    source_root = tmp_path / "corpus"
    summary, _diagnostics, _report = _run_boundary(
        tmp_path, [_arxiv_abs_row(source_root), _arxiv_pdf_row(source_root)]
    )
    summary["text"] = "RAW_PDF_SECRET must never appear in metadata"
    _write_conversion_artifacts(source_root, summary)

    assert _verify_boundary(tmp_path, article_count=1, variant_count=2) == 1
    stderr = capsys.readouterr().err
    assert "metadata_payload_key_leakage" in stderr
    assert "metadata_payload_snippet_leakage" in stderr


def test_conversion_verifier_fails_when_arxiv_pdf_fallback_is_missing(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    source_root = tmp_path / "corpus"
    _run_boundary(tmp_path, [_arxiv_abs_row(source_root)])

    assert _verify_boundary(tmp_path, article_count=1, variant_count=1) == 1
    stderr = capsys.readouterr().err
    assert "missing_arxiv_pdf_fallback" in stderr
    assert "article_without_parser_ready_fallback" in stderr


def test_conversion_verifier_fails_on_unsafe_safety_flags(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    source_root = tmp_path / "corpus"
    summary, _diagnostics, _report = _run_boundary(
        tmp_path, [_arxiv_abs_row(source_root), _arxiv_pdf_row(source_root)]
    )
    summary["fail_closed_safety_flags"]["graph_import_allowed"] = True
    pdf_row = _by_role(summary["results"], "arxiv_pdf")
    pdf_row["fail_closed_safety_flags"]["production_ladybugdb_write_allowed"] = True
    pdf_row["safety_flag_context"]["parser_readiness_claimed_without_conversion_quality"] = True
    _write_conversion_artifacts(source_root, summary)

    assert _verify_boundary(tmp_path, article_count=1, variant_count=2) == 1
    stderr = capsys.readouterr().err
    assert "unsafe_safety_flag_true" in stderr
    assert "graph_import_allowed" in stderr
    assert "production_ladybugdb_write_allowed" in stderr
