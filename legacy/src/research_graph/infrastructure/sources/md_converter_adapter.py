"""MDConverterAdapter — wraps :class:`~research_graph.infrastructure.corpus.sources
.markdown_converter.MDConverter` behind
:class:`~research_graph.domain.ports.FullTextProviderPort` (D088).

This is a THIN adapter (Ponytail): it delegates ``convert_sync`` to the existing
MDConverter WITHOUT changing its fallback routing (arxiv2md → marker → docling,
gated by ``_needs_marker_fallback``). The routing logic stays in MDConverter —
making it an injectable strategy here would be speculative until a second
routing policy exists (Ponytail: no abstraction for one implementation).

Structural typing: MDConverterAdapter (and a plain MDConverter, since it already
exposes ``convert_sync``) satisfies :class:`FullTextProviderPort`. Callers that
depend on the Port can swap in a :class:`FakeFullTextProvider` for tests.
"""

from __future__ import annotations

from research_graph.domain.ports import ConversionResult
from research_graph.infrastructure.corpus.sources.markdown_converter import MDConverter


class MDConverterAdapter:
    """FullTextProviderPort adapter over the existing MDConverter (D088).

    Construct with an optional ``converter`` (a configured :class:`MDConverter`)
    or let the adapter create a default one. Delegates ``convert_sync``
    unchanged. A test fake can be injected via ``converter=`` (any object with a
    ``convert_sync`` method satisfies the Port).
    """

    def __init__(self, *, converter: MDConverter | None = None) -> None:
        self._converter: MDConverter = converter if converter is not None else MDConverter()

    @property
    def converter(self) -> MDConverter:
        """Expose the underlying MDConverter (Adapter-private; for tests/audit)."""
        return self._converter

    def convert_sync(self, arxiv_id: str) -> ConversionResult:
        """Delegate to MDConverter.convert_sync — fallback routing preserved.

        Fail-closed: MDConverter returns a ``ConversionResult`` with ``error``
        set on failure rather than raising (the arxiv2md error result is
        returned even when marker fallback also fails).
        """
        return self._converter.convert_sync(arxiv_id)


__all__ = ["MDConverterAdapter"]
