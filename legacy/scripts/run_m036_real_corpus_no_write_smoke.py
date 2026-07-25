#!/usr/bin/env python3
"""Thin compatibility wrapper; implementation lives in :mod:`research_graph.workflows.universal_kb.smoke_runner`."""

from research_graph.workflows.universal_kb.smoke_runner import *  # noqa: F403
from research_graph.workflows.universal_kb.smoke_runner import main

if __name__ == "__main__":  # pragma: no cover - wrapper only
    raise SystemExit(main())
