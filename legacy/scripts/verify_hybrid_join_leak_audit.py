#!/usr/bin/env python3
"""Audit hybrid body unique IDs vs catalog-joined hybrid_found (M258 S03).

Explains hybrid_unique_paper_ids vs hybrid_found mismatches (join leak).
Read-only. Never import.

Usage::

    uv run python scripts/verify_hybrid_join_leak_audit.py
    uv run python scripts/verify_hybrid_join_leak_audit.py --json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from research_graph.application.corpus.etl_body_coverage_audit import (
    _load_articles,
    find_hybrid_body,
    paper_id_for_article,
)
from research_graph.workflows.composition.etl_body_coverage import (
    DEFAULT_BODY_ROOTS,
    DEFAULT_CATALOG_INDEX,
    DEFAULT_CATALOG_ROOT,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = Path("artifacts/etl/hybrid-join-leak-audit.json")


def _discover_body_ids(body_roots: list[Path]) -> dict[str, list[str]]:
    """Map paper_id -> list of body file paths.

    Filenames are usually ``<id>.hybrid.body.md``. Some expand runs write
    ``original.hybrid.body.md`` under ``<id>/body/``; map those to parent id.
    """
    out: dict[str, list[str]] = {}
    for root in body_roots:
        if not root.is_dir():
            continue
        for path in root.rglob("*.hybrid.body.md"):
            name = path.name
            if not name.endswith(".hybrid.body.md"):
                continue
            pid = name[: -len(".hybrid.body.md")]
            if pid == "original":
                # <root>/<paper_id>/body/original.hybrid.body.md
                parent = path.parent.parent.name if path.parent.name == "body" else ""
                if parent:
                    pid = parent
            if not pid:
                continue
            out.setdefault(pid, []).append(str(path))
    return out


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Audit hybrid unique body ids vs catalog-joined hybrid_found. "
            "Import always false."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--catalog-index", type=Path, default=DEFAULT_CATALOG_INDEX)
    parser.add_argument("--catalog-root", type=Path, default=DEFAULT_CATALOG_ROOT)
    parser.add_argument("--body-root", action="append", type=Path, default=None)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--sample-limit", type=int, default=20)
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)

    def _r(p: Path) -> Path:
        return p if p.is_absolute() else (repo / p).resolve()

    index = _r(args.catalog_index)
    cat_root = _r(args.catalog_root)
    roots = (
        [_r(p) for p in args.body_root]
        if args.body_root
        else [_r(Path(p)) for p in DEFAULT_BODY_ROOTS]
    )

    articles = _load_articles(index)
    catalog_ids: set[str] = set()
    joined: list[str] = []
    catalog_missing_body: list[str] = []
    for art in articles:
        if not isinstance(art, dict):
            continue
        pid = paper_id_for_article(art)
        if not pid:
            continue
        catalog_ids.add(pid)
        body = find_hybrid_body(pid, roots)
        if body is not None:
            joined.append(pid)
        else:
            catalog_missing_body.append(pid)

    body_map = _discover_body_ids(roots)
    body_ids = set(body_map)
    # Raw filename scan for original.hybrid.body.md naming debt
    original_named_files: list[str] = []
    for root in roots:
        if not root.is_dir():
            continue
        for path in root.rglob("original.hybrid.body.md"):
            original_named_files.append(str(path))
    orphan_bodies = sorted(body_ids - catalog_ids)
    joined_set = set(joined)
    body_not_joined = sorted(body_ids - joined_set)

    sample_n = max(0, int(args.sample_limit))
    payload: dict[str, Any] = {
        "schema_version": "hybrid-join-leak-audit.v1",
        "article_count": len(catalog_ids),
        "hybrid_found": len(joined_set),
        "hybrid_unique_body_ids": len(body_ids),
        "body_artifact_files": sum(len(v) for v in body_map.values()),
        "orphan_body_ids_not_in_catalog": orphan_bodies[:sample_n],
        "orphan_body_id_count": len(orphan_bodies),
        "body_ids_not_joined_count": len(body_not_joined),
        "body_ids_not_joined_sample": body_not_joined[:sample_n],
        "delta_unique_minus_found": len(body_ids) - len(joined_set),
        "original_hybrid_body_filename_count": len(original_named_files),
        "original_hybrid_body_filename_sample": original_named_files[:sample_n],
        "naming_debt": (
            "original.hybrid.body.md under <paper_id>/body/ inflates naive "
            "filename unique counts as paper_id=original; mapped to parent id here"
        ),
        "explanation": (
            "hybrid_found counts catalog articles with a resolvable hybrid body. "
            "hybrid_unique_body_ids counts distinct ids from *.hybrid.body.md filenames. "
            "delta > 0 means orphan bodies (id not in catalog) and/or join resolver miss."
        ),
        "body_roots": [str(r) for r in roots],
        "import_eligible": False,
        "graph_writes_allowed": False,
        "note": "Read-only join leak audit; never import.",
    }

    out = _r(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "hybrid-join-leak-audit | "
            f"catalog: {payload['article_count']} | "
            f"hybrid_found: {payload['hybrid_found']} | "
            f"unique_bodies: {payload['hybrid_unique_body_ids']} | "
            f"delta: {payload['delta_unique_minus_found']} | "
            f"orphans: {payload['orphan_body_id_count']} | "
            "import_eligible: false\n"
        )
        if orphan_bodies:
            sys.stdout.write(
                "  orphan_sample: " + ", ".join(orphan_bodies[:5]) + "\n"
            )
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
