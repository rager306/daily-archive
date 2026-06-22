# Formerly: repair contracts and bounded chunk repair workflows placeholder
"""Repair contracts and bounded chunk repair workflows.

The repair package exposes the bounded chunk repair, candidate locator shim,
and chunking benchmark modules. Concrete functions are importable via the
submodules directly (e.g. ``research_graph.repair.bounded_chunk_repair``).
"""

from __future__ import annotations

from research_graph.repair import bounded_chunk_repair, candidate_locators, chunking_benchmark

__all__ = [
    "bounded_chunk_repair",
    "candidate_locators",
    "chunking_benchmark",
]
