#!/usr/bin/env python3
"""Deprecated compatibility shim for the M057 table embedding helper."""

from __future__ import annotations

# Deprecated as of M062 S01. Use src/arxiv_archive/embedder.py:Embedder instead.
from legacy.m057_table_embed import *  # noqa: F401,F403
from legacy.m057_table_embed import main


if __name__ == "__main__":
    raise SystemExit(main())
