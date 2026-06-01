#!/usr/bin/env python3
"""Generic article catalog verifier entrypoint.

This delegates to the M025-origin verifier core without imposing an M025
selection_id, so milestone-specific corpus selections can reuse the same
index-only, no-network, path-safe validation behavior.
"""

from __future__ import annotations

import sys

from verify_m025_article_catalog import main


if __name__ == "__main__":
    raise SystemExit(
        main(
            sys.argv,
            default_expected_selection_id=None,
            label="article catalog",
            default_report_title="Article Catalog Readiness Report",
        )
    )
