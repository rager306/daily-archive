from __future__ import annotations

import importlib.util
import io
import json
import tarfile
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


def test_materialize_local_pdf_reports_skip_when_canonical_asset_exists(tmp_path: Path) -> None:
    source = tmp_path / "versioned.pdf"
    destination = tmp_path / "canonical.pdf"
    payload = b"%PDF-1.7\n" + b"x" * 1200
    source.write_bytes(payload)
    destination.write_bytes(payload)
    result = MODULE.materialize_local_pdf(source, destination)
    assert result["status"] == "skip"


def _write_tar(path: Path, members: list[tuple[str, bytes, str]]) -> None:
    with tarfile.open(path, "w:gz") as archive:
        for name, payload, kind in members:
            info = tarfile.TarInfo(name)
            if kind == "symlink":
                info.type = tarfile.SYMTYPE
                info.linkname = "target"
                archive.addfile(info)
            else:
                info.size = len(payload)
                archive.addfile(info, io.BytesIO(payload))


def test_inspect_tex_source_streams_safe_inventory(tmp_path: Path) -> None:
    source = tmp_path / "source.tar.gz"
    _write_tar(source, [("main.tex", b"\\section{Safe}", "file"), ("refs.bib", b"@article{x,}", "file")])
    result = MODULE.inspect_tex_source(source)
    assert result["member_count"] == 2
    assert result["file_count"] == 2
    assert result["tex_file_count"] == 1
    assert result["bib_file_count"] == 1
    assert result["expanded_bytes"] == len(b"\\section{Safe}") + len(b"@article{x,}")
    assert [item["path"] for item in result["files"]] == ["main.tex", "refs.bib"]


@pytest.mark.parametrize("name", ["../escape.tex", "/absolute.tex", "safe/../../escape.tex"])
def test_inspect_tex_source_rejects_traversal(tmp_path: Path, name: str) -> None:
    source = tmp_path / "unsafe.tar.gz"
    _write_tar(source, [(name, b"bad", "file")])
    with pytest.raises(ValueError, match="unsafe TeX archive member"):
        MODULE.inspect_tex_source(source)


def test_inspect_tex_source_rejects_duplicate_and_noncanonical_names(tmp_path: Path) -> None:
    duplicate = tmp_path / "duplicate.tar.gz"
    _write_tar(duplicate, [("main.tex", b"first", "file"), ("main.tex", b"second", "file")])
    with pytest.raises(ValueError, match="duplicate TeX archive member"):
        MODULE.inspect_tex_source(duplicate)

    for name in (
        "./main.tex",
        "a//b.tex",
        "a/./b.tex",
        "a\\b.tex",
        "-option.tex",
        "line\nbreak.tex",
    ):
        source = tmp_path / (name.replace("/", "_").replace("\\", "_") + ".tar.gz")
        _write_tar(source, [(name, b"bad", "file")])
        with pytest.raises(ValueError, match="unsafe TeX archive member"):
            MODULE.inspect_tex_source(source)

    unicode_source = tmp_path / "unicode.tar.gz"
    _write_tar(unicode_source, [("sections/café.tex", b"ok", "file")])
    report = MODULE.inspect_tex_source(unicode_source)
    assert report["files"][0]["path"] == "sections/café.tex"


def test_inspect_tex_source_rejects_links_and_caps(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    source = tmp_path / "link.tar.gz"
    _write_tar(source, [("link.tex", b"", "symlink")])
    with pytest.raises(ValueError, match="unsafe TeX archive member"):
        MODULE.inspect_tex_source(source)

    oversized = tmp_path / "oversized.tar.gz"
    _write_tar(oversized, [("large.tex", b"x" * 16, "file")])
    monkeypatch.setattr(MODULE, "MAX_TEX_MEMBER_BYTES", 8)
    with pytest.raises(ValueError, match="member too large"):
        MODULE.inspect_tex_source(oversized)


def test_download_tex_source_existing_archive_is_network_free_and_atomic(tmp_path: Path) -> None:
    source = tmp_path / "2606.06036.tar.gz"
    inventory = tmp_path / "2606.06036.tex-inventory.json"
    _write_tar(source, [("main.tex", b"content", "file")])
    result = MODULE.download_tex_source(
        "2606.06036", source, inventory, allow_network=False
    )
    assert result["status"] == "skip"
    assert inventory.is_file()
    assert result["inventory_sha256"] == MODULE.sha256_file(inventory)


def test_metadata_backfill_is_checksum_bound_and_idempotent(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    catalog = Path("data/article_catalog/article_catalog/arxiv")
    root = catalog / "cs-ai" / "2501.00001"
    pdf = root / "source" / "2501.00001.pdf"
    html = root / "article.html"
    pdf.parent.mkdir(parents=True, exist_ok=True)
    pdf.write_bytes(b"%PDF-1.7\n" + b"x" * 1200)
    html.parent.mkdir(parents=True, exist_ok=True)
    html.write_text("<html><body>" + "content " * 100 + "</body></html>")
    result = {
        "id": "2501.00001", "category": "cs-ai", "title": "An Agent Paper", "status": "ok",
        "representations": {
            "pdf": {"status": "ok", "path": str(pdf), "sha256": MODULE.sha256_file(pdf), "byte_size": pdf.stat().st_size, "source_url": "https://arxiv.org/pdf/2501.00001"},
            "html": {"status": "ok", "path": str(html), "sha256": MODULE.sha256_file(html), "byte_size": html.stat().st_size, "source_url": "https://ar5iv.labs.arxiv.org/html/2501.00001"},
        },
    }
    report = Path("download.json")
    report.write_text(json.dumps({"results": [result]}))
    assert MODULE.backfill_metadata(report, catalog) == {"ok": 1, "skip": 0, "fail": 0}
    metadata = json.loads((root / "article.json").read_text())
    assert metadata["article_key"] == "2501.00001"
    assert metadata["safety_defaults"]["production_import_authorized"] is False
    assert metadata["acquisition"]["import_eligible"] is False
    assert [item["source_role"] for item in metadata["source_variants"]] == ["arxiv_pdf", "arxiv_html"]
    assert MODULE.backfill_metadata(report, catalog) == {"ok": 0, "skip": 1, "fail": 0}


def test_metadata_backfill_rejects_checksum_mismatch(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    catalog = Path("catalog")
    pdf = catalog / "cs-ai" / "2501.00002" / "source" / "2501.00002.pdf"
    pdf.parent.mkdir(parents=True, exist_ok=True)
    pdf.write_bytes(b"%PDF-1.7\n" + b"x" * 1200)
    result = {"id": "2501.00002", "category": "cs-ai", "status": "ok", "representations": {"pdf": {"status": "ok", "path": str(pdf), "sha256": "0" * 64}}}
    report = Path("download.json")
    report.write_text(json.dumps({"results": [result]}))
    assert MODULE.backfill_metadata(report, catalog) == {"ok": 0, "skip": 0, "fail": 1}
    assert not (catalog / "cs-ai" / "2501.00002" / "article.json").exists()


def test_normalize_category_is_filesystem_canonical() -> None:
    assert MODULE.normalize_category("cs.CV") == "cs-cv"
