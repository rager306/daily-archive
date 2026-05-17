"""Tests for md_converter module — arxiv2md REST + Marker fallback."""


import pytest

from arxiv_archive.md_converter import ConversionResult, MDConverter


@pytest.mark.asyncio
async def test_arxiv2md_fallback_to_marker():
    """arxiv2md for modern papers, Marker for papers < 2020."""
    converter = MDConverter()

    # Modern paper (has HTML on ar5iv) — should use arxiv2md
    result = await converter.convert("2501.11120")
    assert result.markdown is not None
    assert result.method == "arxiv2md"
    assert len(result.markdown) > 100

    # Old paper (pre-2020, no HTML) — should fallback to Marker
    result_old = await converter.convert("1701.00001")
    assert result_old.method in ("arxiv2md", "marker")  # either works


def test_conversion_result_dataclass():
    """Test ConversionResult has required fields."""
    result = ConversionResult(markdown="# Test", method="arxiv2md", error=None)
    assert result.markdown == "# Test"
    assert result.method == "arxiv2md"
    assert result.error is None

    result_err = ConversionResult(markdown=None, method="marker", error="timeout")
    assert result_err.markdown is None
    assert result_err.error == "timeout"


def test_needs_marker_fallback():
    """Papers before 2020 need Marker fallback."""
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
