"""Deterministic text normalization helpers for parser boundaries.

Formerly: src/arxiv_archive/parsing/normalization.py"""

from __future__ import annotations

import re

_SLUG_RE = re.compile(r"[^a-z0-9]+")


def strip_yaml_frontmatter(text: str) -> str:
    """Return text without leading YAML frontmatter when a closing fence exists."""
    normalized = text.strip()
    if not normalized.startswith("---"):
        return normalized

    lines = normalized.splitlines()
    for index, line in enumerate(lines[1:], start=1):
        if line.strip() == "---":
            return "\n".join(lines[index + 1 :]).strip()
    return normalized


def slugify(value: str) -> str:
    """Return the stable lowercase slug used by parser and PageIndex IDs."""
    slug = _SLUG_RE.sub("-", value.casefold()).strip("-")
    return slug or "section"


__all__ = ["slugify", "strip_yaml_frontmatter"]
