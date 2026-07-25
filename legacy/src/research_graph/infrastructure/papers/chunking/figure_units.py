"""Figure and equation retrieval-unit classification helpers.

This module owns deterministic figure/equation block detection so multimodal
retrieval-unit decisions are explicit handoffs from the general chunker.


Formerly: src/arxiv_archive/chunking/figure_units.py"""

from __future__ import annotations

import re

FIGURE_RE = re.compile(
    r"^\s*(?:!\[[^\]]*\]\([^)]*\)|(?:fig(?:ure)?\.?\s*\d*[:.]).*)", re.IGNORECASE
)
EQUATION_RE = re.compile(
    r"^\s*(?:\$\$|\\\[|\\begin\{(?:equation|align|gather|multline)\}|[A-Za-z0-9_{}^\\]+\s*=\s*.+)"
)


def is_figure_block(text: str) -> bool:
    """Return True when a Markdown block looks like a figure/caption unit."""
    return bool(FIGURE_RE.match(text.strip()))


def is_equation_block(text: str) -> bool:
    """Return True when a Markdown block looks like an equation unit."""
    return bool(EQUATION_RE.match(text.strip()))
