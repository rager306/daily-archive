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
from verify_m029_unified_conversion_quality_boundary import (  # ty: ignore[unresolved-import]
    main as verify_main,  # noqa: E402
)

# pyrefly: ignore [missing-import]
from verify_m029_unified_conversion_quality_boundary import (  # ty: ignore[unresolved-import]
    sha256_file,  # noqa: E402  # pyrefly: ignore [missing-import]
)


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


def _source_row(
    source_root: Path, article_ref: str, role: str, local_path: str, data: str | bytes
) -> dict[str, Any]:
    artifact = _write(source_root / local_path, data)
    article_key = article_ref.rsplit("/", 1)[-1]
    return {
        "schema_version": "m029-source-acquisition.v1",
        "milestone_id": "M029-eb0ljz",
        "slice_id": "S02",
        "selection_id": "m029-unified-corpus-v1",
        "article_ref": article_ref,
        "article_key": article_key,
        "identity_key": f"fixture:{article_key}",
        "variant_id": f"{article_key}:source:{role}",
        "source_role": role,
        "status": "captured",
        "terminal_state": "captured",
        "diagnostic_code": "captured_local_source_artifact",
        "failure_reason": None,
        "local_path": local_path,
        "sha256": sha256_file(artifact),
        "byte_size": artifact.stat().st_size,
        "media_type": "application/pdf" if role.endswith("pdf") else "text/html",
        "network_fetch_attempted": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": {
            "metadata_manifests_embed_raw_text": False,
            "metadata_manifests_embed_raw_binary": False,
            "graph_import_allowed": False,
            "production_ladybugdb_write_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "raw_text_embedded_in_metadata": False,
            "raw_binary_embedded_in_metadata": False,
        },
        "graph_import_allowed": False,
        "production_ladybugdb_write_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }


def _write_inputs(tmp_path: Path) -> tuple[Path, Path, Path, Path, Path, Path]:
    corpus = tmp_path / "corpus"
    source_root = corpus / "source"
    selection = corpus / "selection.json"
    source_summary = corpus / "source-acquisition-summary.json"
    conversion_summary = corpus / "conversion-quality-summary.json"
    conversion_diagnostics = corpus / "conversion-quality-diagnostics.jsonl"
    conversion_report = corpus / "conversion-quality-report.md"
    rows = [
        _source_row(
            source_root,
            "arxiv/mixed-source/2605.20897",
            "arxiv_abs_page",
            "arxiv/mixed-source/2605.20897/source/abs.html",
            "<html><body><nav>links only</nav><h1>Fixture title</h1>"
            "<blockquote class='abstract'>Abstract metadata, no substantive body.</blockquote></body></html>",
        ),
        _source_row(
            source_root,
            "arxiv/mixed-source/2605.20897",
            "arxiv_pdf",
            "arxiv/mixed-source/2605.20897/source/original.pdf",
            _pdf_bytes(
                "This fixture PDF contains enough local converted text for parser readiness. "
                "It has multiple scientific-style sentences so the verifier can prove the "
                "arXiv abstract/navigation source falls back to a substantive PDF artifact."
            ),
        ),
        _source_row(
            source_root,
            "vendor/blog/short",
            "web_article_html",
            "vendor/blog/short/source/article.html",
            "<html><body><article><p>Too short.</p></article></body></html>",
        ),
    ]
    _write_json(
        selection,
        {
            "schema_version": "article-corpus-selection.v00.01",
            "selection_id": "m029-unified-corpus-v1",
            "articles": [
                {
                    "article_ref": "arxiv/mixed-source/2605.20897",
                    "seed_url": "https://arxiv.org/abs/2605.20897",
                },
                {"article_ref": "vendor/blog/short", "seed_url": "https://example.test/short"},
            ],
        },
    )
    _write_json(
        source_summary,
        {
            "schema_version": "m029-source-acquisition-verify.v1",
            "milestone_id": "M029-eb0ljz",
            "slice_id": "S02",
            "selection_id": "m029-unified-corpus-v1",
            "status": "passed",
            "article_count": 2,
            "variant_count": len(rows),
            "counts": {"captured": len(rows), "blocked": 0, "failed": 0},
            "results": rows,
        },
    )
    return (
        selection,
        source_summary,
        conversion_summary,
        conversion_diagnostics,
        conversion_report,
        source_root,
    )


def _verify_args(tmp_path: Path) -> list[str]:
    (
        selection,
        source_summary,
        conversion_summary,
        conversion_diagnostics,
        conversion_report,
        source_root,
    ) = _write_inputs(tmp_path)
    return [
        "verify_m029_unified_conversion_quality_boundary.py",
        "--selection",
        str(selection),
        "--source-summary",
        str(source_summary),
        "--conversion-summary",
        str(conversion_summary),
        "--conversion-diagnostics",
        str(conversion_diagnostics),
        "--conversion-report",
        str(conversion_report),
        "--source-root",
        str(source_root),
        "--corpus-dir",
        str(tmp_path / "corpus"),
        "--converted-text-dir",
        str(tmp_path / "corpus" / "conversion-quality"),
        "--check-negative-cases",
        "--check-low-quality-sources",
        "--require-no-substantive-body-diagnostic",
    ]


def _load_summary(tmp_path: Path) -> dict[str, Any]:
    return json.loads(
        (tmp_path / "corpus" / "conversion-quality-summary.json").read_text(encoding="utf-8")
    )


def _write_conversion_artifacts(tmp_path: Path, summary: dict[str, Any]) -> None:
    corpus = tmp_path / "corpus"
    _write_json(corpus / "conversion-quality-summary.json", summary)
    (corpus / "conversion-quality-diagnostics.jsonl").write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in summary["results"]),
        encoding="utf-8",
    )


def _row_by_role(summary: dict[str, Any], role: str) -> dict[str, Any]:
    matches = [row for row in summary["results"] if row["source_role"] == role]
    assert len(matches) == 1
    return matches[0]


def test_verifier_builds_metadata_only_conversion_artifacts_and_no_substantive_body_diagnostic(
    tmp_path: Path,
) -> None:
    assert verify_main(_verify_args(tmp_path)) == 0

    summary = _load_summary(tmp_path)
    abs_row = _row_by_role(summary, "arxiv_abs_page")
    assert abs_row["status"] == "metadata_only"
    assert abs_row["parser_ready"] is False
    assert abs_row["fallback_reason"] == "no_substantive_body"
    assert abs_row["converted_text_path"] is None

    pdf_row = _row_by_role(summary, "arxiv_pdf")
    assert pdf_row["status"] == "converted"
    assert pdf_row["parser_ready"] is True
    assert Path(pdf_row["converted_text_path"]).exists()

    low_quality = _row_by_role(summary, "web_article_html")
    assert low_quality["status"] == "low_quality"
    assert low_quality["parser_ready"] is False
    assert low_quality["fallback_reason"] == "no_substantive_body"
    assert summary["counts"] == {"converted": 1, "low_quality": 1, "metadata_only": 1}


def test_verifier_fails_closed_on_unsafe_converted_text_path(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    assert verify_main(_verify_args(tmp_path)) == 0
    summary = _load_summary(tmp_path)
    _row_by_role(summary, "arxiv_pdf")["converted_text_path"] = "../escape.txt"
    _write_conversion_artifacts(tmp_path, summary)

    assert verify_main(_verify_args(tmp_path)) == 1
    assert "unsafe_converted_text_path" in capsys.readouterr().err


def test_verifier_fails_closed_when_source_artifact_hash_drifts(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    assert verify_main(_verify_args(tmp_path)) == 0
    (
        tmp_path / "corpus" / "source" / "arxiv/mixed-source/2605.20897/source/original.pdf"
    ).write_bytes(b"drifted source")

    assert verify_main(_verify_args(tmp_path)) == 1
    assert "source_sha256_mismatch" in capsys.readouterr().err


def test_verifier_fails_closed_when_converted_payload_hash_drifts(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    assert verify_main(_verify_args(tmp_path)) == 0
    summary = _load_summary(tmp_path)
    converted_path = Path(_row_by_role(summary, "arxiv_pdf")["converted_text_path"])
    converted_path.write_text(
        converted_path.read_text(encoding="utf-8") + "\nstale converted text", encoding="utf-8"
    )

    assert verify_main(_verify_args(tmp_path)) == 1
    assert "converted_text_sha256_mismatch" in capsys.readouterr().err


def test_verifier_fails_closed_on_unsafe_safety_flag(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    assert verify_main(_verify_args(tmp_path)) == 0
    summary = _load_summary(tmp_path)
    _row_by_role(summary, "arxiv_pdf")["fail_closed_safety_flags"]["graph_import_allowed"] = True
    _write_conversion_artifacts(tmp_path, summary)

    assert verify_main(_verify_args(tmp_path)) == 1
    stderr = capsys.readouterr().err
    assert "unsafe_safety_flag_true" in stderr
    assert "graph_import_allowed" in stderr
