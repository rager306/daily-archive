"""Port substitutability tests for the M104 S02 FullTextProviderPort seam (D088).

Pins that any object implementing
:class:`~research_graph.domain.ports.FullTextProviderPort` is interchangeable at
the call site. ``MDConverterAdapter`` (production, delegates to MDConverter) and
``FakeFullTextProvider`` (in-memory test double) both satisfy the Port
structurally, so the application layer can swap them — and a future second
full-text provider (e.g. a direct GROBID adapter) implements the same Port.

Also verifies that D088 removed the premature ``PDFParserPort`` (single
``parse_article`` implementation) — that was an over-engineered Port.
"""

from __future__ import annotations

from unittest.mock import MagicMock

from research_graph.domain.ports import ConversionResult, FullTextProviderPort
from research_graph.infrastructure.sources import MDConverterAdapter


class FakeFullTextProvider:
    """In-memory test double implementing :class:`FullTextProviderPort` (D088)."""

    def __init__(self, *, result: ConversionResult | None = None) -> None:
        self.default = result or ConversionResult(markdown="# fake", method="fake", error=None)
        self.calls: list[str] = []

    def convert_sync(self, arxiv_id: str) -> ConversionResult:
        self.calls.append(arxiv_id)
        return self.default


class FailingFullTextProvider:
    """Port-compliant double whose ``convert_sync`` returns an error result (fail-closed)."""

    def __init__(self) -> None:
        self.calls: list[str] = []

    def convert_sync(self, arxiv_id: str) -> ConversionResult:
        self.calls.append(arxiv_id)
        return ConversionResult(markdown=None, method="error", error="simulated conversion failure")


def _consume(provider: FullTextProviderPort, arxiv_id: str) -> ConversionResult:
    """Application-layer helper that depends on the Port, not the adapter."""
    return provider.convert_sync(arxiv_id)


# ── D088: PDFParserPort removed (single parse_article impl — Ponytail rule) ──


class TestPortSurfaceMatchesD088:
    def test_pdf_parser_port_removed(self) -> None:
        import research_graph.domain.ports as ports

        assert not hasattr(ports, "PDFParserPort"), "D088: PDFParserPort must be removed"

    def test_fulltext_provider_port_present(self) -> None:
        assert FullTextProviderPort is not None


# ── Port satisfaction ────────────────────────────────────────────────────────


class TestPortSatisfaction:
    def test_md_converter_adapter_satisfies_port(self) -> None:
        assert isinstance(MDConverterAdapter(), FullTextProviderPort)

    def test_fake_provider_satisfies_port(self) -> None:
        assert isinstance(FakeFullTextProvider(), FullTextProviderPort)

    def test_failing_provider_satisfies_port(self) -> None:
        assert isinstance(FailingFullTextProvider(), FullTextProviderPort)


# ── Substitutability: both providers usable through the Port type ────────────


class TestSubstitutability:
    def test_fake_provider_returns_recorded_result(self) -> None:
        fake = FakeFullTextProvider()
        result = _consume(fake, "2605.18747")
        assert fake.calls == ["2605.18747"]
        assert result.method == "fake"
        assert result.markdown == "# fake"

    def test_md_converter_adapter_delegates_through_port(self) -> None:
        mock_converter = MagicMock()
        mock_converter.convert_sync.return_value = ConversionResult(
            markdown="# real", method="arxiv2md", error=None
        )
        adapter = MDConverterAdapter(converter=mock_converter)
        result = _consume(adapter, "2605.18747")
        mock_converter.convert_sync.assert_called_once_with("2605.18747")
        assert result.method == "arxiv2md"

    def test_failing_provider_fail_closed_through_port(self) -> None:
        failing = FailingFullTextProvider()
        result = _consume(failing, "0000.00000")
        # Fail-closed: returns an error result, does not raise
        assert result.markdown is None
        assert result.error == "simulated conversion failure"
        assert result.method == "error"


# ── Adapter construction ────────────────────────────────────────────────────


class TestMDConverterAdapterConstruction:
    def test_default_creates_real_converter(self) -> None:
        adapter = MDConverterAdapter()
        assert adapter.converter is not None
        assert isinstance(adapter, FullTextProviderPort)

    def test_injected_converter_used(self) -> None:
        mock_converter = MagicMock()
        mock_converter.convert_sync.return_value = ConversionResult(
            markdown="x", method="marker", error=None
        )
        adapter = MDConverterAdapter(converter=mock_converter)
        adapter.convert_sync("1234.5678")
        mock_converter.convert_sync.assert_called_once_with("1234.5678")
