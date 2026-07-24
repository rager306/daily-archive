#!/usr/bin/env python3
"""Acquire local PDFs for Wave B gold paper IDs missing {id}.pdf (M258 S02).

For each gold ID:
  * resolve catalog article dir from index
  * if source/<id>.pdf missing, download from arxiv (network) or promote original.pdf
  * never invent gold labels / extraction gold
  * never import / graph write

Usage::

    uv run python scripts/verify_gold_pdf_acquisition.py
    uv run python scripts/verify_gold_pdf_acquisition.py --json
    uv run python scripts/verify_gold_pdf_acquisition.py --no-network
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]

DEFAULT_GOLD_IDS: tuple[str, ...] = (
    "2507.19457",
    "2511.20639",
    "2605.18211",
)
DEFAULT_OUTPUT = Path("artifacts/etl/gold-pdf-acquisition.json")
DEFAULT_INDEX = Path("data/article_catalog/index.json")
DEFAULT_CATALOG_ROOT = Path("data/article_catalog")


def _load_index_articles(index_path: Path) -> list[dict[str, Any]]:
    raw = json.loads(index_path.read_text(encoding="utf-8"))
    arts = raw.get("articles") if isinstance(raw, dict) else raw
    if not isinstance(arts, list):
        return []
    return [a for a in arts if isinstance(a, dict)]


def _find_article(articles: list[dict[str, Any]], paper_id: str) -> dict[str, Any] | None:
    for a in articles:
        blob = json.dumps(a, sort_keys=True)
        if paper_id in blob:
            return a
        key = str(a.get("article_key") or "")
        if key == paper_id or key.endswith(f"/{paper_id}"):
            return a
    return None


def _article_dir(catalog_root: Path, article: dict[str, Any], paper_id: str) -> Path | None:
    rel = str(article.get("article_path") or "").strip()
    if rel:
        # article_path like article_catalog/arxiv/cs-cl/<id>/article.json
        p = catalog_root / rel
        if p.is_file():
            return p.parent
        p2 = catalog_root / "article_catalog" / rel.replace("article_catalog/", "", 1)
        if p2.is_file():
            return p2.parent
    # fallback rglob dir
    hits = list(catalog_root.rglob(paper_id))
    for h in hits:
        if h.is_dir():
            return h
    return None


def _is_pdf(path: Path) -> bool:
    if not path.is_file() or path.stat().st_size < 100:
        return False
    try:
        return path.read_bytes()[:5] == b"%PDF-"
    except OSError:
        return False


def acquire_one(
    *,
    paper_id: str,
    catalog_root: Path,
    article: dict[str, Any] | None,
    allow_network: bool,
    cache_dir: Path,
) -> dict[str, Any]:
    row: dict[str, Any] = {
        "paper_id": paper_id,
        "status": "blocked",
        "blocked_reason": None,
        "catalog_dir": None,
        "pdf_path": None,
        "action": None,
        "bytes": 0,
        "import_eligible": False,
        "graph_writes_allowed": False,
        "gold_labels_invented": False,
    }
    if article is None:
        row["blocked_reason"] = "not_in_catalog_index"
        return row

    adir = _article_dir(catalog_root, article, paper_id)
    if adir is None:
        row["blocked_reason"] = "catalog_dir_not_found"
        return row
    row["catalog_dir"] = str(adir)
    source = adir / "source"
    source.mkdir(parents=True, exist_ok=True)
    dest = source / f"{paper_id}.pdf"

    if _is_pdf(dest):
        row["status"] = "already_present"
        row["pdf_path"] = str(dest)
        row["bytes"] = dest.stat().st_size
        row["action"] = "noop_existing_id_pdf"
        return row

    original = source / "original.pdf"
    if _is_pdf(original):
        shutil.copy2(original, dest)
        if _is_pdf(dest):
            row["status"] = "promoted_original_pdf"
            row["pdf_path"] = str(dest)
            row["bytes"] = dest.stat().st_size
            row["action"] = "copy_original_pdf_to_id_pdf"
            return row

    if not allow_network:
        row["blocked_reason"] = "missing_id_pdf_and_network_disabled"
        return row

    try:
        from research_graph.infrastructure.corpus.ingestion.fetchers import (
            PDFDownloader,
        )

        downloader = PDFDownloader(cache_dir=cache_dir)
        cached = downloader.download(paper_id)
        if not _is_pdf(Path(cached)):
            row["blocked_reason"] = "download_not_pdf"
            row["action"] = f"download_path:{cached}"
            return row
        shutil.copy2(cached, dest)
        row["status"] = "downloaded_and_placed"
        row["pdf_path"] = str(dest)
        row["bytes"] = dest.stat().st_size
        row["action"] = "arxiv_pdf_download_to_catalog_source"
        return row
    except Exception as exc:  # noqa: BLE001 - acquisition must report block
        row["blocked_reason"] = f"download_failed:{type(exc).__name__}:{exc}"
        row["action"] = "download_attempted"
        return row


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Acquire source/<id>.pdf for gold paper IDs. Never invent gold. "
            "Import always false."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--catalog-index", type=Path, default=DEFAULT_INDEX)
    parser.add_argument("--catalog-root", type=Path, default=DEFAULT_CATALOG_ROOT)
    parser.add_argument(
        "--gold-id",
        action="append",
        default=None,
        help="Gold paper id (repeatable; default three known missing hybrid golds)",
    )
    parser.add_argument("--no-network", action="store_true")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "--cache-dir",
        type=Path,
        default=Path("artifacts/etl/gold-pdf-cache"),
        help="Local download cache dir",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)

    def _r(p: Path) -> Path:
        return p if p.is_absolute() else (repo / p).resolve()

    index_path = _r(args.catalog_index)
    catalog_root = _r(args.catalog_root)
    cache_dir = _r(args.cache_dir)
    cache_dir.mkdir(parents=True, exist_ok=True)
    gold_ids = tuple(args.gold_id) if args.gold_id else DEFAULT_GOLD_IDS
    articles = _load_index_articles(index_path)

    rows: list[dict[str, Any]] = []
    for gid in gold_ids:
        art = _find_article(articles, gid)
        rows.append(
            acquire_one(
                paper_id=gid,
                catalog_root=catalog_root,
                article=art,
                allow_network=not args.no_network,
                cache_dir=cache_dir,
            )
        )

    ok_statuses = {
        "already_present",
        "promoted_original_pdf",
        "downloaded_and_placed",
    }
    ok = sum(1 for r in rows if r["status"] in ok_statuses)
    blocked = sum(1 for r in rows if r["status"] == "blocked")
    payload = {
        "schema_version": "gold-pdf-acquisition.v1",
        "gold_ids": list(gold_ids),
        "acquired_count": ok,
        "blocked_count": blocked,
        "records": rows,
        "network_enabled": not args.no_network,
        "import_eligible": False,
        "graph_writes_allowed": False,
        "gold_labels_invented": False,
        "note": (
            "Wave A/B acquisition only: places catalog source/<id>.pdf. "
            "Does not invent gold extraction labels; does not run hybrid; "
            "does not authorize import."
        ),
    }
    out = _r(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "gold-pdf-acquisition | "
            f"acquired: {ok}/{len(rows)} | blocked: {blocked} | "
            f"network: {str(not args.no_network).lower()} | "
            "import_eligible: false | gold_labels_invented: false\n"
        )
        for r in rows:
            sys.stdout.write(
                f"  {r['paper_id']}: {r['status']}"
                + (f" ({r['blocked_reason']})" if r.get("blocked_reason") else "")
                + (f" bytes={r['bytes']}" if r.get("bytes") else "")
                + "\n"
            )
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
