"""Compatibility shim for source asset manifest helpers.

Implementation now lives under :mod:`arxiv_archive.assets` so preservation,
asset registry, and provenance concerns have explicit module boundaries while
legacy callers keep importing this module unchanged.
"""

from __future__ import annotations

from arxiv_archive.assets.registry import *  # noqa: F401,F403
