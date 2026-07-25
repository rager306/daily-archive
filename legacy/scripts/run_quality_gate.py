#!/usr/bin/env python3
"""Thin compatibility wrapper; implementation lives in :mod:`research_graph.infrastructure.quality.gate`."""

from research_graph.infrastructure.quality.gate import *  # noqa: F403
from research_graph.infrastructure.quality.gate import main

if __name__ == "__main__":  # pragma: no cover - wrapper only
    raise SystemExit(main())
