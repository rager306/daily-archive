"""Compatibility shim for structure-aware chunking.

Implementation now lives under :mod:`arxiv_archive.chunking` so chunk formation,
table/figure unit classification, and diagnostics have explicit module
boundaries while legacy callers keep importing this module unchanged.
"""

from __future__ import annotations

from arxiv_archive.chunking.chunker import *  # noqa: F401,F403
from arxiv_archive.chunking.chunker import main

if __name__ == "__main__":
    raise SystemExit(main())
