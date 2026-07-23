#!/usr/bin/env python3
"""Wave A operator script: hybrid selection expand plan (M245).

Scans local catalog PDFs, excludes papers already in selection-20 and papers
that already have hybrid bodies, proposes the next selection rung under a
size cap with category round-robin.

Does NOT run hybrid batch, GROBID, or authorize import. Exit 0 after report.

Usage::

    uv run python scripts/verify_hybrid_selection_expand.py
    uv run python scripts/verify_hybrid_selection_expand.py --json
    uv run python scripts/verify_hybrid_selection_expand.py --write artifacts/m213-hybrid-gate/selection-40-proposal.json
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

from research_graph.application.corpus.hybrid_selection_expand import (
    DEFAULT_MAX_BYTES,
    DEFAULT_TARGET_COUNT,
    InventoryPdfRow,
    plan_next_hybrid_selection,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SELECTION = Path("artifacts/m213-hybrid-gate/selection-20.json")
DEFAULT_CATALOG = Path("data/article_catalog/article_catalog")


def _sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def scan_arxiv_pdf_inventory(catalog_root: Path, *, repo_root: Path) -> list[InventoryPdfRow]:
    """Walk arxiv/{category}/{paper_id}/source/{paper_id}.pdf under catalog_root."""
    rows: list[InventoryPdfRow] = []
    arxiv = catalog_root / "arxiv"
    if not arxiv.is_dir():
        return rows
    for pdf in sorted(arxiv.rglob("*.pdf")):
        if not pdf.is_file():
            continue
        # .../arxiv/<cat>/<paper_id>/source/<paper_id>.pdf
        try:
            rel = pdf.relative_to(arxiv)
        except ValueError:
            continue
        parts = rel.parts
        if len(parts) < 4:
            continue
        category, paper_id = parts[0], parts[1]
        if parts[-1] != f"{paper_id}.pdf":
            # still accept if leaf matches paper folder id
            if paper_id not in parts[-1]:
                continue
        try:
            rel_repo = pdf.resolve().relative_to(repo_root.resolve())
            pdf_path = str(rel_repo).replace("\\", "/")
        except ValueError:
            pdf_path = str(pdf)
        rows.append(
            InventoryPdfRow(
                paper_id=paper_id,
                category=category,
                pdf_path=pdf_path,
                byte_size=int(pdf.stat().st_size),
                sha256=_sha256_file(pdf),
            )
        )
    return rows


def load_selection_paper_ids(path: Path) -> set[str]:
    if not path.is_file():
        return set()
    data = json.loads(path.read_text(encoding="utf-8"))
    papers = data.get("papers") if isinstance(data, dict) else None
    if not isinstance(papers, list):
        return set()
    out: set[str] = set()
    for row in papers:
        if isinstance(row, dict):
            pid = str(row.get("paper_id") or "").strip()
            if pid:
                out.add(pid)
    return out


def discover_hybrid_body_ids(body_roots: list[Path]) -> set[str]:
    ids: set[str] = set()
    for root in body_roots:
        if not root.is_dir():
            continue
        for path in root.rglob("*.hybrid.body.md"):
            name = path.name
            if name.endswith(".hybrid.body.md"):
                pid = name[: -len(".hybrid.body.md")]
                if pid:
                    ids.add(pid)
    return ids


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave A hybrid selection expand plan from local PDFs. "
            "No hybrid batch. Import always false. Exit 0 after report."
        )
    )
    parser.add_argument(
        "--catalog-root",
        type=Path,
        default=DEFAULT_CATALOG,
        help="Catalog tree containing arxiv/<cat>/<id>/source/*.pdf",
    )
    parser.add_argument(
        "--selection",
        type=Path,
        default=DEFAULT_SELECTION,
        help="Existing hybrid selection JSON to exclude",
    )
    parser.add_argument(
        "--body-root",
        action="append",
        type=Path,
        default=None,
        help="Hybrid body root (repeatable). Default: known m213 runs-live* roots.",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=ROOT,
        help="Repository root for relative paths",
    )
    parser.add_argument(
        "--target-count",
        type=int,
        default=DEFAULT_TARGET_COUNT,
        help="Max papers to propose for next rung",
    )
    parser.add_argument(
        "--max-bytes",
        type=int,
        default=DEFAULT_MAX_BYTES,
        help="Skip PDFs larger than this many bytes",
    )
    parser.add_argument(
        "--rung",
        type=int,
        default=40,
        help="Proposed selection rung label",
    )
    parser.add_argument(
        "--milestone-id",
        type=str,
        default="M245-598ifi",
        help="milestone_id stamped into selection proposal",
    )
    parser.add_argument("--json", action="store_true", help="Print full JSON report")
    parser.add_argument(
        "--write",
        type=Path,
        default=None,
        help="Optional path to write selection proposal JSON (m213 shape)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional path to write full expand package JSON",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)

    def _resolve(p: Path) -> Path:
        path = Path(p)
        return path if path.is_absolute() else (repo / path).resolve()

    catalog_root = _resolve(args.catalog_root)
    selection_path = _resolve(args.selection)
    raw_roots = tuple(args.body_root) if args.body_root else DEFAULT_BODY_ROOTS
    body_roots = [_resolve(r) for r in raw_roots]

    inventory = scan_arxiv_pdf_inventory(catalog_root, repo_root=repo)
    selected = load_selection_paper_ids(selection_path)
    bodied = discover_hybrid_body_ids(body_roots)
    exclude = frozenset(selected | bodied)

    package = plan_next_hybrid_selection(
        inventory=inventory,
        exclude_paper_ids=exclude,
        target_count=args.target_count,
        max_bytes=args.max_bytes,
        rung=args.rung,
        extends=str(args.selection),
        milestone_id=args.milestone_id,
    )
    payload = package.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["excluded_selected_count"] = len(selected)
    payload["excluded_bodied_count"] = len(bodied)
    payload["selection_path"] = str(selection_path)
    payload["catalog_root"] = str(catalog_root)

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = _resolve(args.output) if not Path(args.output).is_absolute() else Path(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.write is not None:
        wpath = _resolve(args.write) if not Path(args.write).is_absolute() else Path(args.write)
        wpath.parent.mkdir(parents=True, exist_ok=True)
        sel_text = json.dumps(package.to_selection_dict(), indent=2, sort_keys=True) + "\n"
        wpath.write_text(sel_text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        cats: dict[str, int] = {}
        for p in package.proposed_papers:
            cats[p.category] = cats.get(p.category, 0) + 1
        cat_s = ",".join(f"{k}={v}" for k, v in sorted(cats.items())) or "none"
        sample = ",".join(p.paper_id for p in package.proposed_papers[:5]) or "none"
        sys.stdout.write(
            "hybrid-selection-expand | "
            f"inventory: {package.inventory_count} | "
            f"excluded_selected: {len(selected)} | "
            f"excluded_bodied: {len(bodied)} | "
            f"available: {package.available_after_filters} | "
            f"proposed: {package.proposed_count}/{package.target_count} | "
            f"categories: {cat_s} | "
            f"sample: {sample} | "
            "batch: false | "
            "import_eligible: false\n"
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
