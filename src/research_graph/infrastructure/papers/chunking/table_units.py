"""Table retrieval-unit classification helpers.

This module owns deterministic table-block detection so table retrieval units
are not hidden inside the general chunker implementation.


Formerly: src/arxiv_archive/chunking/table_units.py"""

from __future__ import annotations

import re

TABLE_RE = re.compile(r"^\s*\|.*\|\s*$")
TABLE_SEPARATOR_RE = re.compile(r"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$")


def is_table_block(text: str) -> bool:
    """Return True when a Markdown block contains pipe-table structure."""
    return any(TABLE_RE.match(line) for line in text.splitlines()) or any(
        TABLE_SEPARATOR_RE.match(line) for line in text.splitlines()
    )
