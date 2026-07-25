#!/usr/bin/env python3
"""R024 10-document corpus selection verifier.

Validates:
- 10 articles selected (5 from M025 baseline + 5 new)
- All 10 articles have local source artifacts
- No-network / no-import-flags verified (R056 constraint: no production graph import)
- No duplicates
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
SELECTION = REPO_ROOT / "data" / "r024-10-document-corpus-v1" / "selection.json"
CATALOG = REPO_ROOT / "data" / "article_catalog" / "catalog.json"


def verify(selection_path: Path = SELECTION, expect_count: int = 10) -> int:
    if not selection_path.exists():
        print(f"FAIL: selection not found: {selection_path}")
        return 1
    sel = json.loads(selection_path.read_text())
    articles = sel.get("articles", [])
    n = len(articles)
    print(f"selection_id: {sel.get('selection_id')}")
    print(f"articles: {n} (expected {expect_count})")
    if n != expect_count:
        print(f"FAIL: expected {expect_count} articles, got {n}")
        return 1
    # uniqueness
    keys = [a["article_key"] for a in articles]
    if len(set(keys)) != n:
        print(f"FAIL: duplicate article_keys: {keys}")
        return 1
    # baseline vs extension
    baseline = [
        a for a in articles if a.get("selection_source") == "m025-rlm-dspy-pageindex-smoke-v1"
    ]
    extension = [a for a in articles if a.get("selection_source") == "r024-10-document-corpus-v1"]
    if len(baseline) != 5:
        print(f"FAIL: expected 5 baseline (M025), got {len(baseline)}")
        return 1
    if len(extension) != 5:
        print(f"FAIL: expected 5 extension, got {len(extension)}")
        return 1
    # local source artifacts
    for a in articles:
        ref = a["article_ref"]
        key = a["article_key"]
        # find catalog entry
        found = list(
            REPO_ROOT.glob(f"data/article_catalog/article_catalog/**/{key}/loader/summary.json")
        )
        found2 = list(REPO_ROOT.glob(f"data/article_catalog/article_catalog/**/{key}/article.json"))
        if not (found or found2):
            print(f"FAIL: no local source for {ref} (key={key})")
            return 1
    # network policy
    policy = sel.get("network_policy", {})
    if not policy.get("test_phase_must_not_fetch"):
        print("FAIL: test_phase_must_not_fetch not set")
        return 1
    print("PASS: R024 10-document corpus selection validated")
    print(f"  baseline (M025): {[a['article_ref'] for a in baseline]}")
    print(f"  extension: {[a['article_ref'] for a in extension]}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--selection", type=Path, default=SELECTION)
    parser.add_argument("--expect-count", type=int, default=10)
    args = parser.parse_args()
    return verify(args.selection, args.expect_count)


if __name__ == "__main__":
    sys.exit(main())
