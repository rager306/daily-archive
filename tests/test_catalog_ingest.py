"""Tests for research_graph.infrastructure.corpus.ingestion.catalog_ingest.

Migrated from scripts/m061_ingest_to_canonical_catalog.py (M061 S04, 2026-06-13).
"""

from __future__ import annotations

import dataclasses
import json
import sys
import types
from pathlib import Path

import pytest

from research_graph.infrastructure.corpus.ingestion import (
    IngestOptions as IngestOptionsPkg,
)
from research_graph.infrastructure.corpus.ingestion import (
    SafetyOverride as SafetyOverridePkg,
)
from research_graph.infrastructure.corpus.ingestion.catalog_ingest import (
    ARXIV_API_URL,
    CATALOG_SAFETY_FLAGS,
    FALLBACK_CATEGORY,
    SAFETY_DEFAULTS,
    SAFETY_OVERRIDE_M061_INGEST,
    ApiMetrics,
    IngestOptions,
    RequestPacer,
    SafetyOverride,
    _atomic_write_text,
    arxiv_query_url,
    build_article_record,
    catalog_pdf_count,
    existing_catalog_pdf,
    fetch_arxiv_metadata,
    ingest_catalog,
    invert_anchor_membership,
    load_pdf_paths,
    load_selected_ids,
    normalize_arxiv_id,
    normalize_category,
    parse_retry_after,
    report_bucket,
    update_index_if_exists,
    write_article_record,
)

# ---------------------------------------------------------------------------
# SafetyOverride
# ---------------------------------------------------------------------------


def test_safety_override_is_frozen() -> None:
    so = SafetyOverride(
        external_network_authorized=True,
        reason="test",
        scope="unit test",
    )
    with pytest.raises(dataclasses.FrozenInstanceError):
        so.external_network_authorized = False  # type: ignore[misc]
    assert so.external_network_authorized is True


def test_safety_defaults_all_false() -> None:
    for k, v in SAFETY_DEFAULTS.items():
        assert v is False, f"{k} must be False by default"


def test_catalog_safety_flags_all_false() -> None:
    for k, v in CATALOG_SAFETY_FLAGS.items():
        assert v is False, f"{k} must be False by default"


def test_safety_override_m061_ingest_scope() -> None:
    assert SAFETY_OVERRIDE_M061_INGEST.external_network_authorized is True
    assert (
        "M064" in SAFETY_OVERRIDE_M061_INGEST.scope or "M061" in SAFETY_OVERRIDE_M061_INGEST.scope
    )
    # Reason mentions arxiv API + rate limit (the only network operation allowed)
    assert "arxiv" in SAFETY_OVERRIDE_M061_INGEST.reason.lower()
    assert "rate limit" in SAFETY_OVERRIDE_M061_INGEST.reason.lower()


# ---------------------------------------------------------------------------
# IngestOptions
# ---------------------------------------------------------------------------


def test_ingest_options_defaults() -> None:
    opts = IngestOptions()
    assert opts.m061_root == Path("artifacts/m061-2hop")
    assert opts.arxiv_root == Path("data/article_catalog/article_catalog/arxiv")
    assert opts.update_index is True
    assert opts.safety_override is SAFETY_OVERRIDE_M061_INGEST


def test_ingest_options_custom() -> None:
    custom_safety = SafetyOverride(False, "test reason", "unit test scope")
    opts = IngestOptions(
        m061_root=Path("/tmp/test"),
        arxiv_root=Path("/tmp/test-catalog"),
        safety_override=custom_safety,
        update_index=False,
    )
    assert opts.m061_root == Path("/tmp/test")
    assert opts.arxiv_root == Path("/tmp/test-catalog")
    assert opts.safety_override.external_network_authorized is False
    assert opts.update_index is False


def test_ingest_options_pkg_alias() -> None:
    """Same dataclass via package public API."""
    assert IngestOptions is IngestOptionsPkg
    assert SafetyOverride is SafetyOverridePkg


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def test_normalize_arxiv_id_strips_pdf() -> None:
    assert normalize_arxiv_id("1203.2295.pdf") == "1203.2295"
    assert normalize_arxiv_id("  1203.2295  ") == "1203.2295"
    assert normalize_arxiv_id("1203.2295") == "1203.2295"


def test_normalize_category_basic() -> None:
    assert normalize_category("cs.CL") == "cs-cl"
    assert normalize_category("cs.AI") == "cs-ai"
    assert normalize_category("cs_lg") == "cs-lg"


def test_normalize_category_fallback() -> None:
    assert normalize_category(None) == FALLBACK_CATEGORY
    assert normalize_category("") == FALLBACK_CATEGORY
    assert normalize_category("   ") == FALLBACK_CATEGORY


def test_report_bucket_known() -> None:
    assert report_bucket("cs-cl") == "cs-cl"
    assert report_bucket("unknown-cat") == "other"
    assert report_bucket("mixed-source") == "mixed-source"


def test_parse_retry_after_numeric() -> None:
    assert parse_retry_after("5") == 5.0
    assert parse_retry_after("0") == 0.0


def test_parse_retry_after_invalid() -> None:
    assert parse_retry_after(None) is None
    assert parse_retry_after("") is None
    assert parse_retry_after("not-a-date-or-number") is None


def test_catalog_pdf_count_missing_dir(tmp_path: Path) -> None:
    assert catalog_pdf_count(tmp_path / "nonexistent") == 0


# ---------------------------------------------------------------------------
# Atomic catalog JSON writes
# ---------------------------------------------------------------------------


def test_atomic_write_text_preserves_existing_file_if_replace_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    target = tmp_path / "article.json"
    target.write_text('{"old": true}\n', encoding="utf-8")

    def fail_replace(self: Path, target_path: Path) -> Path:
        raise OSError("simulated replace failure")

    monkeypatch.setattr(Path, "replace", fail_replace)

    with pytest.raises(OSError, match="simulated replace failure"):
        _atomic_write_text(target, '{"new": true}\n')

    assert target.read_text(encoding="utf-8") == '{"old": true}\n'
    assert not list(tmp_path.glob(".article.json.*.tmp"))


def test_write_article_record_uses_atomic_text_helper(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    calls: list[tuple[Path, str]] = []

    def fake_atomic_write(path: Path, text: str, *, encoding: str = "utf-8") -> None:
        calls.append((path, text))
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding=encoding)

    monkeypatch.setattr(
        "research_graph.infrastructure.corpus.ingestion.catalog_ingest._atomic_write_text",
        fake_atomic_write,
    )

    article_path = tmp_path / "catalog" / "article.json"
    write_article_record(article_path, {"article_key": "2605.18747"})

    assert calls == [(article_path, '{\n  "article_key": "2605.18747"\n}\n')]
    assert json.loads(article_path.read_text(encoding="utf-8")) == {"article_key": "2605.18747"}


def test_update_index_if_exists_uses_atomic_text_helper(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    catalog_root = tmp_path / "catalog"
    index_path = catalog_root / "article_catalog" / "index.json"
    index_path.parent.mkdir(parents=True)
    index_path.write_text('{"articles": []}\n', encoding="utf-8")
    (catalog_root / "catalog.json").write_text("{}\n", encoding="utf-8")

    module = types.ModuleType("verify_m025_article_catalog")

    def rebuild_index_from_articles(catalog_manifest_path: Path, existing: dict) -> tuple[dict, list]:
        assert catalog_manifest_path == catalog_root / "catalog.json"
        assert existing == {"articles": []}
        return {"articles": [{"article_key": "2605.18747"}]}, []

    module.rebuild_index_from_articles = rebuild_index_from_articles  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "verify_m025_article_catalog", module)

    calls: list[Path] = []

    def fake_atomic_write(path: Path, text: str, *, encoding: str = "utf-8") -> None:
        calls.append(path)
        path.write_text(text, encoding=encoding)

    monkeypatch.setattr(
        "research_graph.infrastructure.corpus.ingestion.catalog_ingest._atomic_write_text",
        fake_atomic_write,
    )

    updated, entries, diagnostics = update_index_if_exists(catalog_root)

    assert updated is True
    assert entries == 1
    assert diagnostics == []
    assert calls == [index_path]
    assert json.loads(index_path.read_text(encoding="utf-8")) == {
        "articles": [{"article_key": "2605.18747"}]
    }


# ---------------------------------------------------------------------------
# RequestPacer
# ---------------------------------------------------------------------------


def test_request_pacer_no_wait_first_call() -> None:
    pacer = RequestPacer(min_interval_seconds=1.0, sleep=lambda _: None)
    pacer.wait()  # first call should not sleep
    pacer.mark_request_started()


def test_request_pacer_waits() -> None:
    sleeps: list[float] = []

    def fake_sleep(s: float) -> None:
        sleeps.append(s)

    pacer = RequestPacer(min_interval_seconds=1.0, sleep=fake_sleep)
    pacer.mark_request_started()
    pacer.wait()
    assert sleeps, "pacer should sleep when elapsed < min_interval"


# ---------------------------------------------------------------------------
# fetch_arxiv_metadata (fail-closed)
# ---------------------------------------------------------------------------


def test_fetch_arxiv_metadata_fail_closed_no_network() -> None:
    """If safety_override.external_network_authorized=False, no network call."""
    safety = SafetyOverride(False, "no-network test", "unit test")
    pacer = RequestPacer(sleep=lambda _: None)
    metrics = ApiMetrics()
    metadata = fetch_arxiv_metadata(
        "2605.18747",
        pacer=pacer,
        metrics=metrics,
        sleep=lambda _: None,
        safety_override=safety,
    )
    assert metadata.fallback is True
    assert metadata.source == "fallback"
    assert metadata.error == "external_network_authorized=False"
    assert metrics.requests_made == 0
    assert metrics.failures == 1


def test_arxiv_query_url_format() -> None:
    url = arxiv_query_url("2605.18747")
    assert ARXIV_API_URL in url
    assert "id_list=2605.18747" in url
    assert "max_results=1" in url


# ---------------------------------------------------------------------------
# Article builder
# ---------------------------------------------------------------------------


def test_build_article_record_basic(tmp_path: Path) -> None:
    pdf = tmp_path / "arxiv" / "cs-ai" / "1207.4167" / "source" / "1207.4167.pdf"
    pdf.parent.mkdir(parents=True)
    pdf.write_bytes(b"%PDF-1.4 fake content")
    catalog_root = tmp_path / "data" / "article_catalog"
    article = build_article_record("1207.4167", "cs-ai", "Test Title", pdf, catalog_root)
    assert article["schema_version"] == "article.v00.01"
    assert article["article_key"] == "1207.4167"
    assert article["coarse_topic_code"] == "cs-ai"
    assert article["identity"]["title"] == "Test Title"
    assert article["safety_flags"]["graph_import_allowed"] is False
    assert article["safety_flags"]["ladybugdb_written"] is False
    assert article["safety_override"]["external_network_authorized"] is True


def test_build_article_record_fail_closed_flags() -> None:
    pdf = Path("/tmp/nonexistent/test.pdf")
    article = build_article_record("9999.9999", "cs-ai", "Test", pdf, Path("/tmp/catalog"))
    for flag in (
        "graph_import_allowed",
        "production_ladybugdb_write_allowed",
        "trusted_kg_import_allowed",
        "production_import_attempted",
        "ladybugdb_written",
    ):
        assert article["safety_flags"][flag] is False, f"{flag} must be False"


# ---------------------------------------------------------------------------
# write_article_record
# ---------------------------------------------------------------------------


def test_write_article_record_roundtrip(tmp_path: Path) -> None:
    pdf = tmp_path / "source" / "1207.4167.pdf"
    pdf.parent.mkdir(parents=True)
    pdf.write_bytes(b"%PDF-1.4 fake")
    article = build_article_record("1207.4167", "cs-ai", "Title", pdf, tmp_path)
    article_path = tmp_path / "article.json"
    write_article_record(article_path, article)
    assert article_path.exists()
    loaded = json.loads(article_path.read_text())
    assert loaded["article_key"] == "1207.4167"


# ---------------------------------------------------------------------------
# existing_catalog_pdf + invert_anchor_membership
# ---------------------------------------------------------------------------


def test_existing_catalog_pdf_not_found(tmp_path: Path) -> None:
    assert existing_catalog_pdf(tmp_path, "9999.9999") is None


def test_invert_anchor_membership_basic() -> None:
    anchors = {"a1": ["x", "y"], "a2": ["y", "z"]}
    inv = invert_anchor_membership(anchors)
    assert inv == {"x": ["a1"], "y": ["a1", "a2"], "z": ["a2"]}


# ---------------------------------------------------------------------------
# Catalog ingest end-to-end (offline; safety_override False)
# ---------------------------------------------------------------------------


def test_ingest_catalog_fail_closed_no_network(tmp_path: Path) -> None:
    """Offline ingest: external_network_authorized=False → all fallback."""
    m061_root = tmp_path / "m061"
    anchor_dir = m061_root / "anchor-test"
    (anchor_dir / "acquisition").mkdir(parents=True)
    selected = anchor_dir / "acquisition" / "selected-2hop-papers.json"
    selected.write_text(json.dumps({"selected_arxiv_ids": ["2605.18747", "1703.00050"]}))
    pdf1 = anchor_dir / "acquisition" / "pdfs" / "2605.18747.pdf"
    pdf2 = anchor_dir / "acquisition" / "pdfs" / "1703.00050.pdf"
    pdf1.parent.mkdir(parents=True)
    pdf1.write_bytes(b"%PDF-1.4 fake1")
    pdf2.write_bytes(b"%PDF-1.4 fake2")

    catalog_root = tmp_path / "data" / "article_catalog"
    arxiv_root = catalog_root / "article_catalog" / "arxiv"
    arxiv_root.mkdir(parents=True)

    safety = SafetyOverride(False, "offline test", "unit test")
    options = IngestOptions(
        m061_root=m061_root,
        arxiv_root=arxiv_root,
        safety_override=safety,
        update_index=False,
    )
    result = ingest_catalog(options)
    assert result.unique_arxiv_ids == 2
    assert result.api_metrics.requests_made == 0
    assert result.api_metrics.failures == 2
    for record in result.records:
        assert record.fallback is True
        assert record.dest_pdf.exists()


# ---------------------------------------------------------------------------
# Module integration: load_selected_ids + load_pdf_paths
# ---------------------------------------------------------------------------


def test_load_selected_ids_not_found(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError):
        load_selected_ids(tmp_path)


def test_load_pdf_paths_not_found(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError):
        load_pdf_paths(tmp_path)
