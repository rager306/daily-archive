"""Compatibility shim for negative import-boundary rehearsal helpers.

Implementation ownership moved to :mod:`arxiv_archive.staging.import_boundary`.
Keep this module so existing public imports continue to work.
"""

from __future__ import annotations

from arxiv_archive.staging.import_boundary import *
from arxiv_archive.staging.import_boundary import __all__
