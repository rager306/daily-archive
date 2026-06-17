# Formerly: src/arxiv_archive/candidate_locators.py

"""Compatibility shim for candidate locator staging helpers.

Implementation ownership moved to :mod:`research_graph.staging.graph_candidates`.
Keep this module so existing public imports continue to work.
"""

from __future__ import annotations

from research_graph.staging.graph_candidates import *
from research_graph.staging.graph_candidates import __all__
