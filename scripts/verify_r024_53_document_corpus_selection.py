#!/usr/bin/env python3
"""R024 53-doc corpus selection verifier (fail-closed)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path("/root/daily-archive")
OUT_SELECTION = REPO_ROOT / "data" / "r024-53-document-corpus-v1" / "selection.json"

CATALOG_ROOT = REPO_ROOT / "data" / "article_catalog" / "article_catalog"


def main() -> int:
    sel = json.loads(OUT_SELECTION.read_text())
    articles = sel["articles"]
    n = len(articles)

    assert n == 53, f"Expected 53, got {n}"

    keys = [a["article_key"] for a in articles]
    assert len(set(keys)) == n, "Duplicate keys found"

    # all extractable (PDF or HTML)
    missing = []
    for a in articles:
        ref = a["article_ref"]
        sd = CATALOG_ROOT / ref / "source"
        if not sd.exists():
            missing.append(ref)
            continue
        source_kind = a.get("source_kind", "")
        if source_kind == "pdf_converted":
            pdf = sd / f"{a['article_key']}.pdf"
            if not pdf.exists():
                missing.append(f"{ref}: no PDF")
        else:
            has_text = bool(
                list(sd.glob("*.html")) + list(sd.glob("*.md")) + list(sd.glob("*.txt"))
            )
            if not has_text:
                missing.append(f"{ref}: no text")

    assert not missing, f"Missing sources: {missing[:5]}"

    # fail-closed
    assert sel.get("network_policy") == "test_phase_must_not_fetch"
    assert sel.get("graph_import_allowed") is False
    assert sel.get("ladybugdb_written") is False
    assert sel.get("production_import_attempted") is False

    # counts
    baseline = sum(1 for a in articles if "m117" in a.get("selection_source", ""))
    extension = sum(1 for a in articles if "m118" in a.get("selection_source", ""))
    assert baseline == 20, f"Expected 20 baseline, got {baseline}"
    assert extension == 33, f"Expected 33 extension, got {extension}"

    pdf_count = sum(1 for a in articles if a.get("source_kind") == "pdf_converted")
    html_count = sum(1 for a in articles if a.get("source_kind") != "pdf_converted")
    print(
        f"OK: 53 articles ({baseline} baseline + {extension} extension; {pdf_count} pdf + {html_count} html/md)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
