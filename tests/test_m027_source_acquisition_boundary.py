from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

# pyrefly: ignore [missing-import]
from capture_m027_mixed_source_sources import (  # noqa: E402  # ty:ignore[unresolved-import]
    FAIL_CLOSED_SAFETY_FLAGS,
    FetchResponse,
    capture_selection,
    capture_variant,
    main,
    safe_catalog_path,
    selected_article_paths,
    sha256_bytes,
)

# pyrefly: ignore [missing-import]
from verify_m027_source_acquisition_boundary import (  # ty: ignore[unresolved-import]
    main as verify_replay_main,  # noqa: E402  # ty:ignore[unresolved-import]
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
    "fixture arxiv abstract page",
    "fixture nature article page",
    "%PDF-1.4",
    "base64,",
}


def _article(article_ref: str = "arxiv/mixed-source/2605.20897") -> dict[str, Any]:
    article_key = article_ref.rsplit("/", 1)[-1]
    return {
        "schema_version": "article.v00.01",
        "article_key": article_key,
        "catalog_path": article_ref,
        "source_variants": [],
        "safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def _variant(
    role: str, *, url: str | None = "https://example.test/source", path: str | None = None
) -> dict[str, Any]:
    return {
        "variant_id": f"test:source:{role}",
        "source_role": role,
        "source_format": "pdf" if role.endswith("pdf") else "html_metadata",
        "path": path,
        "url": url,
        "media_type": "application/pdf" if role.endswith("pdf") else "text/html",
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "network_fetch_attempted": False,
    }


def _article_path(tmp_path: Path, article_ref: str = "arxiv/mixed-source/2605.20897") -> Path:
    path = tmp_path / "article_catalog" / article_ref / "article.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(_article(article_ref)), encoding="utf-8")
    return path


def _assert_metadata_only(result: dict[str, Any]) -> None:
    serialized = json.dumps(result, sort_keys=True)
    assert not (set(result) & FORBIDDEN_KEYS)
    for forbidden in FORBIDDEN_SNIPPETS:
        assert forbidden not in serialized
    assert result["raw_text_embedded"] is False
    assert result["raw_binary_embedded"] is False
    assert result["raw_payload_embedded_in_metadata"] is False
    safety = result["fail_closed_safety_flags"]
    assert safety["graph_import_allowed"] is False
    assert safety["production_ladybugdb_write_allowed"] is False
    assert safety["trusted_kg_import_allowed"] is False
    assert safety["production_import_attempted"] is False
    assert safety["ladybugdb_written"] is False


def test_arxiv_abs_html_capture_writes_metadata_only_record(tmp_path: Path) -> None:
    article_path = _article_path(tmp_path)
    html = b"<html><body>fixture arxiv abstract page</body></html>"

    result = capture_variant(
        article_path,
        _article(),
        _variant("arxiv_abs_page", url="https://arxiv.org/abs/2605.20897"),
        fetcher=lambda url: FetchResponse(html, media_type="text/html"),
    )

    assert result["status"] == "captured"
    assert result["diagnostic_code"] == "captured_source_artifact"
    assert result["local_path"] == "source/abs.html"
    assert result["sha256"] == sha256_bytes(html)
    assert result["byte_size"] == len(html)
    assert result["media_type"] == "text/html"
    assert result["network_fetch_attempted"] is True
    assert (article_path.parent / "source" / "abs.html").read_bytes() == html
    _assert_metadata_only(result)


def test_arxiv_pdf_capture_requires_pdf_signature(tmp_path: Path) -> None:
    article_path = _article_path(tmp_path)
    pdf = b"%PDF-1.4\n% fixture bytes\n%%EOF\n"

    result = capture_variant(
        article_path,
        _article(),
        _variant("arxiv_pdf", url="https://arxiv.org/pdf/2605.20897"),
        fetcher=lambda url: pdf,
    )

    assert result["status"] == "captured"
    assert result["local_path"] == "source/original.pdf"
    assert result["sha256"] == sha256_bytes(pdf)
    assert result["byte_size"] == len(pdf)
    assert result["media_type"] == "application/pdf"
    assert (article_path.parent / "source" / "original.pdf").read_bytes() == pdf
    _assert_metadata_only(result)


def test_nature_html_capture_uses_article_html_target(tmp_path: Path) -> None:
    article_ref = "nature/mixed-source/s44387-025-00019-5"
    article_path = _article_path(tmp_path, article_ref)
    html = b"<html><body>fixture nature article page</body></html>"

    result = capture_variant(
        article_path,
        _article(article_ref),
        _variant("nature_html", url="https://www.nature.com/articles/s44387-025-00019-5"),
        fetcher=lambda url: FetchResponse(html, media_type="text/html"),
    )

    assert result["status"] == "captured"
    assert result["article_ref"] == article_ref
    assert result["local_path"] == "source/article.html"
    assert (article_path.parent / "source" / "article.html").read_bytes() == html
    _assert_metadata_only(result)


def test_missing_url_blocks_without_network_or_file_write(tmp_path: Path) -> None:
    article_path = _article_path(tmp_path)

    result = capture_variant(
        article_path,
        _article(),
        _variant("arxiv_abs_page", url=None),
        fetcher=lambda url: pytest.fail("fetcher must not be called for missing URL"),
    )

    assert result["status"] == "blocked"
    assert result["diagnostic_code"] == "missing_source_url"
    assert result["network_fetch_attempted"] is False
    assert not (article_path.parent / "source" / "abs.html").exists()
    _assert_metadata_only(result)


def test_empty_response_fails_without_fallback_artifact(tmp_path: Path) -> None:
    article_path = _article_path(tmp_path)

    result = capture_variant(
        article_path,
        _article(),
        _variant("arxiv_abs_page"),
        fetcher=lambda url: b"",
    )

    assert result["status"] == "failed"
    assert result["diagnostic_code"] == "empty_response"
    assert result["byte_size"] == 0
    assert result["sha256"] is None
    assert result["network_fetch_attempted"] is True
    assert not (article_path.parent / "source" / "abs.html").exists()
    _assert_metadata_only(result)


def test_bad_pdf_signature_fails_without_writing_html_as_pdf(tmp_path: Path) -> None:
    article_path = _article_path(tmp_path)

    result = capture_variant(
        article_path,
        _article(),
        _variant("arxiv_pdf"),
        fetcher=lambda url: b"<html>not a pdf</html>",
    )

    assert result["status"] == "failed"
    assert result["diagnostic_code"] == "bad_pdf_signature"
    assert result["local_path"] == "source/original.pdf"
    assert not (article_path.parent / "source" / "original.pdf").exists()
    _assert_metadata_only(result)


@pytest.mark.parametrize(
    ("bad_path", "expected_code"),
    [
        ("../outside.html", "unsafe_catalog_relative_path"),
        ("/tmp/outside.html", "unsafe_catalog_relative_path"),
        ("https://example.test/source.html", "url_not_allowed_as_local_path"),
        ("source/not-abs.html", "unexpected_source_path_for_role"),
    ],
)
def test_unsafe_or_unexpected_variant_paths_are_blocked(
    tmp_path: Path, bad_path: str, expected_code: str
) -> None:
    article_path = _article_path(tmp_path)

    result = capture_variant(
        article_path,
        _article(),
        _variant("arxiv_abs_page", path=bad_path),
        fetcher=lambda url: pytest.fail("fetcher must not be called for unsafe path"),
    )

    assert result["status"] == "blocked"
    assert result["diagnostic_code"] == expected_code
    assert not (article_path.parent / "source" / "abs.html").exists()
    _assert_metadata_only(result)


def test_fetch_error_blocks_without_fallback_payload_success(tmp_path: Path) -> None:
    article_path = _article_path(tmp_path)

    def failing_fetcher(url: str) -> bytes:
        raise TimeoutError("offline fixture timeout")

    result = capture_variant(
        article_path,
        _article(),
        _variant("arxiv_pdf"),
        fetcher=failing_fetcher,
    )

    assert result["status"] == "blocked"
    assert result["diagnostic_code"] == "fetch_timeout"
    assert result["sha256"] is None
    assert result["byte_size"] == 0
    assert result["network_fetch_attempted"] is True
    assert not (article_path.parent / "source" / "original.pdf").exists()
    _assert_metadata_only(result)


def test_safe_catalog_path_rejects_traversal_absolute_and_url_like_paths(tmp_path: Path) -> None:
    assert safe_catalog_path(tmp_path, "article_catalog/arxiv/x/article.json").is_relative_to(
        tmp_path.resolve()
    )
    for rel_path in ("../escape", "/tmp/escape", "https://example.test/file"):
        with pytest.raises(ValueError):
            safe_catalog_path(tmp_path, rel_path)


def test_selection_loads_articles_through_index_and_captures_offline_fixtures(
    tmp_path: Path,
) -> None:
    catalog_root = tmp_path / "catalog"
    article_ref = "arxiv/mixed-source/2605.20897"
    article_path = catalog_root / "article_catalog" / article_ref / "article.json"
    article_path.parent.mkdir(parents=True, exist_ok=True)
    article = _article(article_ref)
    article["source_variants"] = [
        _variant("arxiv_abs_page", url="https://arxiv.org/abs/2605.20897"),
        _variant("arxiv_pdf", url="https://arxiv.org/pdf/2605.20897"),
    ]
    article_path.write_text(json.dumps(article), encoding="utf-8")
    index_path = catalog_root / "index.json"
    index_path.write_text(
        json.dumps(
            {
                "articles": [
                    {
                        "article_ref": article_ref,
                        "article_path": f"article_catalog/{article_ref}/article.json",
                    }
                ]
            }
        ),
        encoding="utf-8",
    )
    selection_path = tmp_path / "selection.json"
    selection_path.write_text(
        json.dumps({"articles": [{"article_ref": article_ref}]}), encoding="utf-8"
    )

    responses = {
        "https://arxiv.org/abs/2605.20897": b"<html>fixture arxiv abstract page</html>",
        "https://arxiv.org/pdf/2605.20897": b"%PDF-1.4\n%%EOF\n",
    }

    paths = selected_article_paths(
        catalog_root, json.loads(index_path.read_text()), json.loads(selection_path.read_text())
    )
    assert paths == [article_path.resolve()]

    results = capture_selection(
        catalog_root=catalog_root,
        index_path=index_path,
        selection_path=selection_path,
        fetcher=lambda url: responses[url],
    )

    assert [result["status"] for result in results] == ["captured", "captured"]
    assert {result["local_path"] for result in results} == {
        "source/abs.html",
        "source/original.pdf",
    }
    assert (article_path.parent / "source" / "abs.html").exists()
    assert (article_path.parent / "source" / "original.pdf").exists()
    for result in results:
        _assert_metadata_only(result)


M027_REFS = [
    "arxiv/mixed-source/2605.20897",
    "arxiv/mixed-source/2605.21401",
    "nature/mixed-source/s44387-025-00019-5",
    "arxiv/mixed-source/2605.25522",
    "arxiv/mixed-source/2603.04448",
    "arxiv/mixed-source/2604.18478",
]


def _m027_variants(article_ref: str) -> list[dict[str, Any]]:
    key = article_ref.rsplit("/", 1)[-1]
    if article_ref.startswith("nature/"):
        return [
            _variant("nature_html", url="https://www.nature.com/articles/s44387-025-00019-5"),
            _variant("citation_metadata", url="https://www.nature.com/articles/s44387-025-00019-5"),
        ]
    return [
        _variant("arxiv_abs_page", url=f"https://arxiv.org/abs/{key}"),
        _variant("arxiv_pdf", url=f"https://arxiv.org/pdf/{key}"),
    ]


def _write_response(response_dir: Path, url: str, payload: bytes) -> None:
    response_dir.mkdir(parents=True, exist_ok=True)
    (response_dir / f"{sha256_bytes(url.encode('utf-8'))}.bin").write_bytes(payload)


def _write_m027_catalog_fixture(tmp_path: Path) -> tuple[Path, Path, Path, Path, Path]:
    catalog_root = tmp_path / "catalog"
    corpus_dir = tmp_path / "corpus"
    fixture_dir = tmp_path / "fixtures"
    index_rows = []
    selection_rows = []
    for article_ref in M027_REFS:
        article_path = catalog_root / "article_catalog" / article_ref / "article.json"
        article_path.parent.mkdir(parents=True, exist_ok=True)
        article = _article(article_ref)
        article["source_variants"] = _m027_variants(article_ref)
        article_path.write_text(json.dumps(article), encoding="utf-8")
        rel = f"article_catalog/{article_ref}/article.json"
        index_rows.append({"article_ref": article_ref, "article_path": rel})
        selection_rows.append({"article_ref": article_ref})
        for variant in article["source_variants"]:
            if variant["source_role"] not in {"arxiv_abs_page", "arxiv_pdf", "nature_html"}:
                continue
            payload = (
                b"%PDF-1.4\n%%EOF\n"
                if variant["source_role"] == "arxiv_pdf"
                else b"<html><body>fixture arxiv abstract page</body></html>"
            )
            if variant["source_role"] == "nature_html":
                payload = b"<html><body>fixture nature article page</body></html>"
            _write_response(fixture_dir, variant["url"], payload)
    catalog_path = catalog_root / "catalog.json"
    catalog_path.write_text(json.dumps({"schema_version": "catalog.test"}), encoding="utf-8")
    index_path = catalog_root / "index.json"
    index_path.write_text(json.dumps({"articles": index_rows}), encoding="utf-8")
    selection_path = corpus_dir / "selection.json"
    selection_path.parent.mkdir(parents=True, exist_ok=True)
    selection_path.write_text(json.dumps({"articles": selection_rows}), encoding="utf-8")
    return catalog_root, catalog_path, index_path, selection_path, fixture_dir


def _artifact_texts(output_dir: Path) -> str:
    return "\n".join(
        [
            (output_dir / "source-acquisition-summary.json").read_text(encoding="utf-8"),
            (output_dir / "source-acquisition-diagnostics.jsonl").read_text(encoding="utf-8"),
            (output_dir / "source-acquisition-report.md").read_text(encoding="utf-8"),
        ]
    )


def test_cli_updates_all_six_selected_records_and_writes_metadata_only_artifacts(
    tmp_path: Path,
) -> None:
    catalog_root, catalog_path, index_path, selection_path, fixture_dir = (
        _write_m027_catalog_fixture(tmp_path)
    )
    output_dir = tmp_path / "corpus"

    exit_code = main(
        [
            "capture_m027_mixed_source_sources.py",
            "--catalog-root",
            str(catalog_root),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--selection",
            str(selection_path),
            "--output-dir",
            str(output_dir),
            "--fixture-response-dir",
            str(fixture_dir),
        ]
    )

    assert exit_code == 0
    summary = json.loads(
        (output_dir / "source-acquisition-summary.json").read_text(encoding="utf-8")
    )
    assert summary["milestone_id"] == "M027-aakeky"
    assert summary["slice_id"] == "S02"
    assert summary["capture_phase_network_allowed"] is True
    assert summary["replay_phase_network_allowed"] is False
    assert summary["graph_import_allowed"] is False
    assert summary["production_import_attempted"] is False
    assert summary["ladybugdb_written"] is False
    assert summary["variant_count"] == 11
    assert summary["counts"] == {"captured": 11, "blocked": 0, "failed": 0}
    assert set(summary["output_paths"]) == {"summary", "diagnostics", "report"}
    assert set(summary["output_hashes"]) == {"summary", "diagnostics", "report"}
    assert (output_dir / "source-acquisition-diagnostics.jsonl").read_text(encoding="utf-8").count(
        "\n"
    ) == 11

    for article_ref in M027_REFS:
        article_path = catalog_root / "article_catalog" / article_ref / "article.json"
        article = json.loads(article_path.read_text(encoding="utf-8"))
        assert article["capture_summary"]["selection_id"] == "m027-mixed-source-corpus-v1"
        assert article["capture_summary"]["failed_count"] == 0
        assert article["capture_summary"]["blocked_count"] == 0
        for variant in article["source_variants"]:
            if variant["source_role"] in {"arxiv_abs_page", "arxiv_pdf", "nature_html"}:
                assert variant["capture_status"] == "captured"
                assert variant["network_fetch_attempted"] is True
                assert variant["path"] in {
                    "source/abs.html",
                    "source/original.pdf",
                    "source/article.html",
                }
                assert variant["graph_import_allowed"] is False
                assert variant["production_ladybugdb_write_allowed"] is False

    serialized = _artifact_texts(output_dir)
    for forbidden in FORBIDDEN_SNIPPETS:
        assert forbidden not in serialized


def test_cli_missing_index_row_fails_before_artifact_promotion(tmp_path: Path) -> None:
    catalog_root, catalog_path, index_path, selection_path, fixture_dir = (
        _write_m027_catalog_fixture(tmp_path)
    )
    index_path.write_text(json.dumps({"articles": []}), encoding="utf-8")
    output_dir = tmp_path / "corpus"

    with pytest.raises(ValueError, match="selection article not present in index"):
        main(
            [
                "capture_m027_mixed_source_sources.py",
                "--catalog-root",
                str(catalog_root),
                "--catalog",
                str(catalog_path),
                "--index",
                str(index_path),
                "--selection",
                str(selection_path),
                "--output-dir",
                str(output_dir),
                "--fixture-response-dir",
                str(fixture_dir),
            ]
        )
    assert not (output_dir / "source-acquisition-summary.json").exists()


def test_cli_duplicate_or_unsafe_index_path_is_rejected(tmp_path: Path) -> None:
    catalog_root, _catalog_path, index_path, selection_path, _fixture_dir = (
        _write_m027_catalog_fixture(tmp_path)
    )
    selection = {"articles": [{"article_ref": "arxiv/mixed-source/2605.20897"}]}
    selection_path.write_text(json.dumps(selection), encoding="utf-8")
    index_path.write_text(
        json.dumps(
            {
                "articles": [
                    {
                        "article_ref": "arxiv/mixed-source/2605.20897",
                        "article_path": "article_catalog/arxiv/mixed-source/2605.20897/article.json",
                    },
                    {
                        "article_ref": "arxiv/mixed-source/2605.20897",
                        "article_path": "article_catalog/arxiv/mixed-source/2605.20897/article.json",
                    },
                ]
            }
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="duplicate index row"):
        selected_article_paths(
            catalog_root, json.loads(index_path.read_text()), json.loads(selection_path.read_text())
        )

    index_path.write_text(
        json.dumps(
            {
                "articles": [
                    {
                        "article_ref": "arxiv/mixed-source/2605.20897",
                        "article_path": "../escape/article.json",
                    }
                ]
            }
        ),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="unsafe_catalog_relative_path"):
        selected_article_paths(
            catalog_root, json.loads(index_path.read_text()), json.loads(selection_path.read_text())
        )


def test_cli_fixture_failure_response_records_failed_diagnostic_without_fallback(
    tmp_path: Path,
) -> None:
    catalog_root, catalog_path, index_path, selection_path, fixture_dir = (
        _write_m027_catalog_fixture(tmp_path)
    )
    # Empty one selected response: the command completes artifact emission but returns non-zero style status.
    _write_response(fixture_dir, "https://arxiv.org/abs/2605.20897", b"")
    output_dir = tmp_path / "corpus"

    exit_code = main(
        [
            "capture_m027_mixed_source_sources.py",
            "--catalog-root",
            str(catalog_root),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--selection",
            str(selection_path),
            "--output-dir",
            str(output_dir),
            "--fixture-response-dir",
            str(fixture_dir),
        ]
    )

    assert exit_code == 1
    summary = json.loads(
        (output_dir / "source-acquisition-summary.json").read_text(encoding="utf-8")
    )
    assert summary["counts"]["failed"] == 1
    failed = [row for row in summary["results"] if row["status"] == "failed"]
    assert failed[0]["diagnostic_code"] == "empty_response"
    article = json.loads(
        (catalog_root / "article_catalog/arxiv/mixed-source/2605.20897/article.json").read_text(
            encoding="utf-8"
        )
    )
    failed_variant = [
        v for v in article["source_variants"] if v["source_role"] == "arxiv_abs_page"
    ][0]
    assert failed_variant["capture_status"] == "failed"
    assert not (
        catalog_root / "article_catalog/arxiv/mixed-source/2605.20897/source/abs.html"
    ).exists()


def test_cli_rejects_output_dir_traversal(tmp_path: Path) -> None:
    catalog_root, catalog_path, index_path, selection_path, fixture_dir = (
        _write_m027_catalog_fixture(tmp_path)
    )
    with pytest.raises(ValueError, match="unsafe_output_dir_traversal"):
        main(
            [
                "capture_m027_mixed_source_sources.py",
                "--catalog-root",
                str(catalog_root),
                "--catalog",
                str(catalog_path),
                "--index",
                str(index_path),
                "--selection",
                str(selection_path),
                "--output-dir",
                "../outside-corpus",
                "--fixture-response-dir",
                str(fixture_dir),
            ]
        )


def _run_capture_fixture(
    catalog_root: Path,
    catalog_path: Path,
    index_path: Path,
    selection_path: Path,
    fixture_dir: Path,
    output_dir: Path,
) -> None:
    exit_code = main(
        [
            "capture_m027_mixed_source_sources.py",
            "--catalog-root",
            str(catalog_root),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--selection",
            str(selection_path),
            "--output-dir",
            str(output_dir),
            "--fixture-response-dir",
            str(fixture_dir),
        ]
    )
    assert exit_code == 0


def _run_replay_verifier(
    catalog_root: Path, catalog_path: Path, index_path: Path, selection_path: Path, output_dir: Path
) -> int:
    return verify_replay_main(
        [
            "verify_m027_source_acquisition_boundary.py",
            "--catalog-root",
            str(catalog_root),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--selection",
            str(selection_path),
            "--summary",
            str(output_dir / "source-acquisition-summary.json"),
            "--diagnostics",
            str(output_dir / "source-acquisition-diagnostics.jsonl"),
            "--report",
            str(output_dir / "source-acquisition-report.md"),
        ]
    )


def _prepared_replay_fixture(tmp_path: Path) -> tuple[Path, Path, Path, Path, Path]:
    catalog_root, catalog_path, index_path, selection_path, fixture_dir = (
        _write_m027_catalog_fixture(tmp_path)
    )
    output_dir = tmp_path / "corpus"
    _run_capture_fixture(
        catalog_root, catalog_path, index_path, selection_path, fixture_dir, output_dir
    )
    return catalog_root, catalog_path, index_path, selection_path, output_dir


def _first_selected_variant(
    catalog_root: Path,
    article_ref: str = "arxiv/mixed-source/2605.20897",
    role: str = "arxiv_abs_page",
) -> tuple[Path, dict[str, Any], dict[str, Any]]:
    article_path = catalog_root / "article_catalog" / article_ref / "article.json"
    article = json.loads(article_path.read_text(encoding="utf-8"))
    variant = [v for v in article["source_variants"] if v.get("source_role") == role][0]
    return article_path, article, variant


def _write_article(article_path: Path, article: dict[str, Any]) -> None:
    article_path.write_text(json.dumps(article), encoding="utf-8")


def _diagnostic_text(output_dir: Path) -> str:
    return (output_dir / "source-acquisition-diagnostics.jsonl").read_text(encoding="utf-8")


def test_replay_verifier_passes_local_only_against_fixture_generated_records(
    tmp_path: Path,
) -> None:
    catalog_root, catalog_path, index_path, selection_path, output_dir = _prepared_replay_fixture(
        tmp_path
    )

    exit_code = _run_replay_verifier(
        catalog_root, catalog_path, index_path, selection_path, output_dir
    )

    assert exit_code == 0
    summary = json.loads(
        (output_dir / "source-acquisition-summary.json").read_text(encoding="utf-8")
    )
    replay = summary["local_only_replay_verification"]
    assert replay["status"] == "passed"
    assert replay["selected_article_count"] == 6
    assert replay["network_fetch_attempted"] is False
    assert replay["production_import_attempted"] is False
    assert replay["ladybugdb_written"] is False
    assert replay["trusted_kg_import_allowed"] is False
    assert replay["graph_import_allowed"] is False
    assert replay["provenance"]["validate_only"] is True
    assert replay["provenance"]["output_hashes"]
    report = (output_dir / "source-acquisition-report.md").read_text(encoding="utf-8")
    assert "Local-Only Replay Verification" in report


@pytest.mark.parametrize(
    ("bad_path", "expected_code"),
    [
        ("../escape.pdf", "unsafe_local_path"),
        ("/tmp/escape.pdf", "unsafe_local_path"),
        ("https://example.test/source.pdf", "url_not_allowed_as_local_path"),
    ],
)
def test_replay_verifier_rejects_unsafe_or_url_like_local_paths(
    tmp_path: Path, bad_path: str, expected_code: str
) -> None:
    catalog_root, catalog_path, index_path, selection_path, output_dir = _prepared_replay_fixture(
        tmp_path
    )
    article_path, article, variant = _first_selected_variant(catalog_root, role="arxiv_pdf")
    variant["path"] = bad_path
    _write_article(article_path, article)

    exit_code = _run_replay_verifier(
        catalog_root, catalog_path, index_path, selection_path, output_dir
    )

    assert exit_code == 1
    assert expected_code in _diagnostic_text(output_dir)


def test_replay_verifier_fails_on_hash_mismatch_after_captured_bytes_change(tmp_path: Path) -> None:
    catalog_root, catalog_path, index_path, selection_path, output_dir = _prepared_replay_fixture(
        tmp_path
    )
    article_path, _article_data, variant = _first_selected_variant(
        catalog_root, role="arxiv_abs_page"
    )
    (article_path.parent / variant["path"]).write_bytes(b"<html>changed local bytes</html>")

    exit_code = _run_replay_verifier(
        catalog_root, catalog_path, index_path, selection_path, output_dir
    )

    assert exit_code == 1
    diagnostics = _diagnostic_text(output_dir)
    assert "sha256_mismatch" in diagnostics
    assert "byte_size_mismatch" in diagnostics


def test_replay_verifier_fails_on_missing_captured_file(tmp_path: Path) -> None:
    catalog_root, catalog_path, index_path, selection_path, output_dir = _prepared_replay_fixture(
        tmp_path
    )
    article_path, _article_data, variant = _first_selected_variant(
        catalog_root, role="arxiv_abs_page"
    )
    (article_path.parent / variant["path"]).unlink()

    exit_code = _run_replay_verifier(
        catalog_root, catalog_path, index_path, selection_path, output_dir
    )

    assert exit_code == 1
    assert "missing_captured_file" in _diagnostic_text(output_dir)


def test_replay_verifier_fails_on_captured_pdf_without_pdf_signature(tmp_path: Path) -> None:
    catalog_root, catalog_path, index_path, selection_path, output_dir = _prepared_replay_fixture(
        tmp_path
    )
    article_path, article, variant = _first_selected_variant(catalog_root, role="arxiv_pdf")
    bad_pdf = b"not-a-pdf"
    (article_path.parent / variant["path"]).write_bytes(bad_pdf)
    variant["sha256"] = sha256_bytes(bad_pdf)
    variant["byte_size"] = len(bad_pdf)
    _write_article(article_path, article)

    exit_code = _run_replay_verifier(
        catalog_root, catalog_path, index_path, selection_path, output_dir
    )

    assert exit_code == 1
    assert "bad_pdf_signature" in _diagnostic_text(output_dir)


def test_replay_verifier_fails_on_blocked_variant_without_reason(tmp_path: Path) -> None:
    catalog_root, catalog_path, index_path, selection_path, output_dir = _prepared_replay_fixture(
        tmp_path
    )
    article_path, article, variant = _first_selected_variant(catalog_root, role="arxiv_abs_page")
    variant["capture_status"] = "blocked"
    variant["acquisition_status"] = "blocked"
    variant["diagnostic_code"] = "fixture_blocked"
    variant["failure_reason"] = ""
    _write_article(article_path, article)

    exit_code = _run_replay_verifier(
        catalog_root, catalog_path, index_path, selection_path, output_dir
    )

    assert exit_code == 1
    assert "missing_failure_reason" in _diagnostic_text(output_dir)


def test_replay_verifier_rejects_raw_payload_field_leakage(tmp_path: Path) -> None:
    catalog_root, catalog_path, index_path, selection_path, output_dir = _prepared_replay_fixture(
        tmp_path
    )
    article_path, article, variant = _first_selected_variant(catalog_root, role="arxiv_abs_page")
    variant["raw_text"] = "do not serialize article body text"
    _write_article(article_path, article)

    exit_code = _run_replay_verifier(
        catalog_root, catalog_path, index_path, selection_path, output_dir
    )

    assert exit_code == 1
    assert "raw_payload_key_leakage" in _diagnostic_text(output_dir)


def test_replay_verifier_rejects_raw_payload_snippet_in_metadata_artifact(tmp_path: Path) -> None:
    catalog_root, catalog_path, index_path, selection_path, output_dir = _prepared_replay_fixture(
        tmp_path
    )
    summary = json.loads(
        (output_dir / "source-acquisition-summary.json").read_text(encoding="utf-8")
    )
    summary["leaked_note"] = "base64,abcdef"
    (output_dir / "source-acquisition-summary.json").write_text(
        json.dumps(summary), encoding="utf-8"
    )

    exit_code = _run_replay_verifier(
        catalog_root, catalog_path, index_path, selection_path, output_dir
    )

    assert exit_code == 1
    assert "raw_payload_snippet_leakage" in _diagnostic_text(output_dir)


def test_replay_verifier_rejects_unsafe_graph_or_production_write_flags(tmp_path: Path) -> None:
    catalog_root, catalog_path, index_path, selection_path, output_dir = _prepared_replay_fixture(
        tmp_path
    )
    article_path, article, variant = _first_selected_variant(catalog_root, role="arxiv_abs_page")
    variant["production_import_attempted"] = True
    _write_article(article_path, article)

    exit_code = _run_replay_verifier(
        catalog_root, catalog_path, index_path, selection_path, output_dir
    )

    assert exit_code == 1
    diagnostics = _diagnostic_text(output_dir)
    assert "unsafe_true_safety_flag" in diagnostics
    assert "production_import_attempted" in diagnostics
