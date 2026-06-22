#!/usr/bin/env python3
"""R024 20-doc corpus selection verifier (fail-closed)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
OUT_DIR = REPO_ROOT / "data" / "r024-20-document-corpus-v1"
OUT_SELECTION = OUT_DIR / "selection.json"

CATALOG_ROOT = REPO_ROOT / "data" / "article_catalog" / "article_catalog"


def main() -> int:
    sel = json.loads(OUT_SELECTION.read_text())
    articles = sel["articles"]
    n = len(articles)

    # 1. 20 articles
    assert n == 20, f"Expected 20, got {n}"

    # 2. unique keys
    keys = [a["article_key"] for a in articles]
    assert len(set(keys)) == n, "Duplicate keys found"

    # 3. all local sources
    missing = []
    for a in articles:
        art = CATALOG_ROOT / a["article_ref"] / "article.json"
        if not art.exists():
            missing.append(a["article_ref"])
            continue
        sd = CATALOG_ROOT / a["article_ref"] / "source"
        if not sd.exists() or not (
            list(sd.glob("*.html")) or list(sd.glob("*.md")) or list(sd.glob("*.txt"))
        ):
            missing.append(a["article_ref"])
    assert not missing, f"Missing local sources for: {missing}"

    # 4. fail-closed
    assert sel.get("network_policy") == "test_phase_must_not_fetch"
    assert sel.get("graph_import_allowed") is False
    assert sel.get("ladybugdb_written") is False
    assert sel.get("production_import_attempted") is False

    # 5. baseline count
    baseline = sum(1 for a in articles if "m116" in a.get("selection_source", ""))
    extension = sum(1 for a in articles if "m117" in a.get("selection_source", ""))
    assert baseline == 10, f"Expected 10 baseline, got {baseline}"
    assert extension == 10, f"Expected 10 extension, got {extension}"

    print(
        f"OK: 20 articles, {baseline} baseline + {extension} extension, all unique, all local sources, fail-closed"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
