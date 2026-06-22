"""Property-based tests for Markdown Converter using Hypothesis and Adaptix."""

from typing import Any

from hypothesis import given, settings
from hypothesis import strategies as st

from research_graph.infrastructure.corpus.sources.markdown_converter import (
    ConversionResult,
    MDConverter,
)
from tests.helpers.modular_fixtures import FIXTURE_MARKDOWN, MODULAR_RETORT, adaptix_dump


def dict_to_conversion_result(data: dict[str, Any]) -> ConversionResult:
    return MODULAR_RETORT.load(data, ConversionResult)


# --- Property: Adaptix roundtripping ---

@settings(max_examples=100)
@given(
    markdown=st.one_of(st.none(), st.just(FIXTURE_MARKDOWN), st.text()),
    method=st.sampled_from(["arxiv2md", "marker", "error", "unknown"]),
    error=st.one_of(st.none(), st.text()),
)
def test_conversion_result_adaptix_roundtrip(
    markdown: str | None, method: str, error: str | None
) -> None:
    """ConversionResult serialized to dict and back must preserve all fields."""
    original = ConversionResult(markdown=markdown, method=method, error=error)
    dumped = adaptix_dump(original)
    restored = dict_to_conversion_result(dumped)

    assert restored.markdown == original.markdown
    assert restored.method == original.method
    assert restored.error == original.error

# --- Property: normalize_id ---

@settings(max_examples=100)
@given(
    base_id=st.text(min_size=1).filter(lambda x: "arxiv:" not in x.lower()),
    spaces_before=st.integers(min_value=0, max_value=5),
    spaces_after=st.integers(min_value=0, max_value=5),
    include_prefix=st.booleans(),
)
def test_normalize_id_strips_prefix_and_whitespace(
    base_id: str, spaces_before: int, spaces_after: int, include_prefix: bool
) -> None:
    """_normalize_id must always strip surrounding whitespace and 'arxiv:' prefix."""
    converter = MDConverter()

    prefix = "arxiv:" if include_prefix else ""
    padded_id = (" " * spaces_before) + prefix + base_id + (" " * spaces_after)

    normalized = converter._normalize_id(padded_id)

    assert normalized == base_id.strip()

# --- Property: needs_marker_fallback ---

@settings(max_examples=200)
@given(
    year=st.integers(min_value=2007, max_value=2050),
    month=st.integers(min_value=1, max_value=12),
    seq=st.integers(min_value=0, max_value=99999),
    four_digit_year=st.booleans(),
)
def test_needs_marker_fallback_correctly_identifies_years(
    year: int, month: int, seq: int, four_digit_year: bool
) -> None:
    """_needs_marker_fallback must return True for year < 2020, False otherwise, given valid IDs."""
    converter = MDConverter()

    if four_digit_year:
        arxiv_id = f"{year:04d}{month:02d}.{seq:04d}"
    else:
        short_year = year % 100
        arxiv_id = f"{short_year:02d}{month:02d}.{seq:04d}"

    needs_fallback = converter._needs_marker_fallback(arxiv_id)

    if year < 2020:
        assert needs_fallback is True
    else:
        assert needs_fallback is False

@settings(max_examples=100)
@given(
    invalid_id=st.text().filter(lambda x: not __import__('re').match(r"^\d{2,4}\d{2}\.\d+$", x))
)
def test_needs_marker_fallback_invalid_id_returns_false(invalid_id: str) -> None:
    """_needs_marker_fallback must return False for unparseable IDs."""
    converter = MDConverter()
    assert converter._needs_marker_fallback(invalid_id) is False


@settings(max_examples=50)
@given(
    year=st.integers(min_value=2007, max_value=2050),
    month=st.sampled_from([0, 13, 99]),
    seq=st.integers(min_value=0, max_value=99999),
)
def test_needs_marker_fallback_invalid_month(year: int, month: int, seq: int) -> None:
    converter = MDConverter()
    arxiv_id = f"{year:04d}{month:02d}.{seq:04d}"
    assert converter._needs_marker_fallback(arxiv_id) is False
