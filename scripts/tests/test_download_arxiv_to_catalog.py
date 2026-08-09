from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).parents[1] / "download_arxiv_to_catalog.py"
SPEC = importlib.util.spec_from_file_location("download_arxiv_to_catalog", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def test_normalize_arxiv_id_accepts_canonical_forms_and_strips_version() -> None:
    assert MODULE.normalize_arxiv_id("arXiv:2606.11190v2") == "2606.11190"
    assert MODULE.normalize_arxiv_id("https://arxiv.org/abs/2605.26525") == "2605.26525"
    assert MODULE.normalize_arxiv_id("hep-th/9901001v3") == "hep-th/9901001"


@pytest.mark.parametrize("value", ["", "../2606.11190", "2606.11190/../../x", "not-an-id"])
def test_normalize_arxiv_id_rejects_invalid_or_traversal_input(value: str) -> None:
    with pytest.raises(ValueError):
        MODULE.normalize_arxiv_id(value)


def test_inspect_pdf_returns_checksum_and_rejects_non_pdf(tmp_path: Path) -> None:
    pdf = tmp_path / "paper.pdf"
    pdf.write_bytes(b"%PDF-1.7\n" + b"x" * 1200)
    artifact = MODULE.inspect_pdf(pdf)
    assert artifact["byte_size"] == pdf.stat().st_size
    assert len(artifact["sha256"]) == 64

    invalid = tmp_path / "invalid.pdf"
    invalid.write_bytes(b"<html>not a pdf</html>" * 100)
    with pytest.raises(ValueError, match="PDF"):
        MODULE.inspect_pdf(invalid)


def test_download_pdf_existing_asset_is_checksum_bound_and_network_free(tmp_path: Path) -> None:
    dest = tmp_path / "2606.11190.pdf"
    dest.write_bytes(b"%PDF-1.7\n" + b"a" * 1200)
    result = MODULE.download_pdf("2606.11190", dest, allow_network=False)
    assert result["status"] == "skip"
    assert result["path"] == str(dest)
    assert len(result["sha256"]) == 64


def test_download_pdf_requires_explicit_network_authorization(tmp_path: Path) -> None:
    with pytest.raises(PermissionError, match="allow-network"):
        MODULE.download_pdf("2606.11190", tmp_path / "missing.pdf", allow_network=False)


def test_inspect_html_accepts_document_and_rejects_error_page(tmp_path: Path) -> None:
    html = tmp_path / "article.html"
    html.write_text("<!DOCTYPE html><html><body>" + "paper " * 300 + "</body></html>")
    artifact = MODULE.inspect_html(html)
    assert artifact["byte_size"] == html.stat().st_size
    assert len(artifact["sha256"]) == 64

    invalid = tmp_path / "error.html"
    invalid.write_text("service unavailable")
    with pytest.raises(ValueError, match="HTML"):
        MODULE.inspect_html(invalid)


def test_find_local_versioned_pdf_reuses_unique_catalog_asset(tmp_path: Path) -> None:
    source = tmp_path / "cs-lg/2606.11190v1/source/2606.11190v1.pdf"
    source.parent.mkdir(parents=True)
    source.write_bytes(b"%PDF-1.7\n" + b"x" * 1200)
    found = MODULE.find_local_versioned_pdf(tmp_path, "2606.11190")
    assert found == source


def test_normalize_category_is_filesystem_canonical() -> None:
    assert MODULE.normalize_category("cs.CV") == "cs-cv"
