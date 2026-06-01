from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from capture_m027_mixed_source_sources import (  # noqa: E402
    FAIL_CLOSED_SAFETY_FLAGS,
    FetchResponse,
    capture_selection,
    capture_variant,
    safe_catalog_path,
    selected_article_paths,
    sha256_bytes,
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


def _variant(role: str, *, url: str | None = "https://example.test/source", path: str | None = None) -> dict[str, Any]:
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


def test_selection_loads_articles_through_index_and_captures_offline_fixtures(tmp_path: Path) -> None:
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
    selection_path.write_text(json.dumps({"articles": [{"article_ref": article_ref}]}), encoding="utf-8")

    responses = {
        "https://arxiv.org/abs/2605.20897": b"<html>fixture arxiv abstract page</html>",
        "https://arxiv.org/pdf/2605.20897": b"%PDF-1.4\n%%EOF\n",
    }

    paths = selected_article_paths(catalog_root, json.loads(index_path.read_text()), json.loads(selection_path.read_text()))
    assert paths == [article_path.resolve()]

    results = capture_selection(
        catalog_root=catalog_root,
        index_path=index_path,
        selection_path=selection_path,
        fetcher=lambda url: responses[url],
    )

    assert [result["status"] for result in results] == ["captured", "captured"]
    assert {result["local_path"] for result in results} == {"source/abs.html", "source/original.pdf"}
    assert (article_path.parent / "source" / "abs.html").exists()
    assert (article_path.parent / "source" / "original.pdf").exists()
    for result in results:
        _assert_metadata_only(result)
