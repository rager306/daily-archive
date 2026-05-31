"""Compatibility shim for candidate locator staging helpers.

Implementation ownership moved to :mod:`arxiv_archive.staging.graph_candidates`.
Keep this module so existing public imports continue to work.
"""

from __future__ import annotations

from arxiv_archive.staging.graph_candidates import *
from arxiv_archive.staging.graph_candidates import __all__
