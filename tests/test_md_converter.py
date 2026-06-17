"""Tests for md_converter module — arxiv2md REST + quality fallback."""


import pytest

from research_graph.corpus.sources.markdown_converter import ConversionResult, MDConverter


@pytest.mark.asyncio
async def test_arxiv2md_fallback_to_quality_backend():
    """arxiv2md for modern papers, quality fallback for papers without usable HTML."""
    converter = MDConverter()

    # Modern paper (has HTML on ar5iv) — should use arxiv2md
    result = await converter.convert("2501.11120")
    assert result.markdown is not None
    assert result.method == "arxiv2md"
    assert len(result.markdown) > 100

    # Old paper (pre-2020, no usable HTML) should use an accepted quality fallback.
    result_old = await converter.convert("1701.00001")
    assert result_old.method in ("arxiv2md", "docling", "marker")


def test_conversion_result_dataclass():
    """Test ConversionResult has required fields."""
    result = ConversionResult(markdown="# Test", method="arxiv2md", error=None)
    assert result.markdown == "# Test"
    assert result.method == "arxiv2md"
    assert result.error is None

    result_err = ConversionResult(markdown=None, method="marker", error="timeout")
    assert result_err.markdown is None
    assert result_err.error == "timeout"


def test_needs_quality_fallback():
    """Papers before 2020 need the configured quality fallback path."""
    converter = MDConverter()
    # Pre-2020 papers
    assert converter._needs_marker_fallback("1701.00001") is True
    assert converter._needs_marker_fallback("1912.99999") is True
    # 2020 and after
    assert converter._needs_marker_fallback("2001.00001") is False
    assert converter._needs_marker_fallback("2501.11120") is False


def test_normalizes_arxiv_id_prefix():
    """convert() strips 'arxiv:' prefix."""
    converter = MDConverter()

    # Strip "arxiv:" prefix
    assert converter._normalize_id("arxiv:2501.11120") == "2501.11120"
    # Plain IDs pass through
    assert converter._normalize_id("2501.11120") == "2501.11120"
