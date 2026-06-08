#!/usr/bin/env python3
"""Select a deterministic real-article batch for M036 no-write smoke."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
ARTICLE_ROOT = ROOT / "data" / "article_catalog" / "article_catalog"
DEFAULT_CATALOG = ROOT / "data" / "article_catalog" / "catalog.json"

SOURCE_EXTENSIONS = {".html", ".pdf", ".md", ".txt", ".xml", ".json"}
FORBIDDEN_TERMS = ("api_key", "secret", "token", "credential", "embedding", "vector", "raw_text")


def emit(message: str) -> None:
    sys.stdout.write(f"{message}\n")


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def safety_flags_false(article: dict[str, Any]) -> bool:
    flags = article.get("safety_flags")
    if not isinstance(flags, dict):
        return False
    for key in (
        "production_graphdb_write_allowed",
        "production_ladybugdb_write_allowed",
        "trusted_kg_import_allowed",
        "production_import_attempted",
        "ladybugdb_written",
        "raw_text_embedded_in_metadata",
        "raw_binary_embedded_in_metadata",
    ):
        if flags.get(key) is not False:
            return False
    return True


def source_files(article_path: Path) -> list[Path]:
    source_dir = article_path.parent / "source"
    if not source_dir.exists():
        return []
    return sorted(path for path in source_dir.iterdir() if path.is_file() and path.suffix.lower() in SOURCE_EXTENSIONS)


def loader_refs(article_path: Path) -> list[str]:
    loader_dir = article_path.parent / "loader"
    if not loader_dir.exists():
        return []
    return [
        f"artifact:{path.relative_to(ROOT).as_posix()}"
        for path in sorted(loader_dir.iterdir())
        if path.is_file()
    ]


def article_ref(article_path: Path) -> str:
    return f"artifact:{article_path.relative_to(ROOT).as_posix()}"


def metadata_safe(value: str) -> bool:
    lowered = value.lower()
    return not any(term in lowered for term in FORBIDDEN_TERMS)


def candidate_entry(article_path: Path) -> dict[str, Any] | None:
    article = load_json(article_path)
    key = str(article.get("article_key") or "").strip()
    catalog_path = str(article.get("catalog_path") or "").strip()
    identity = article.get("identity") if isinstance(article.get("identity"), dict) else {}
    title = str(identity.get("title") or "").strip()
    sources = source_files(article_path)
    diagnostics: list[str] = []
    if not key or not catalog_path or not title:
        return None
    if not safety_flags_false(article):
        diagnostics.append("safety_flags_missing_or_not_false")
    if not sources:
        diagnostics.append("missing_local_source_file")
    loader = loader_refs(article_path)
    if not loader:
        diagnostics.append("missing_loader_evidence")

    evidence_refs = [article_ref(article_path)]
    evidence_refs.extend(f"artifact:{path.relative_to(ROOT).as_posix()}" for path in sources[:2])
    evidence_refs.extend(loader[:2])
    if not all(metadata_safe(ref) for ref in evidence_refs):
        diagnostics.append("unsafe_metadata_ref")
    if "missing_local_source_file" in diagnostics or "unsafe_metadata_ref" in diagnostics:
        return None

    return {
        "article_key": key,
        "catalog_path": catalog_path,
        "title": title,
        "source_code": article.get("source_code"),
        "source_type": article.get("source_type"),
        "publisher": article.get("publisher"),
        "candidate_id": f"real-article:{key}",
        "candidate_type": "real_article_metadata",
        "article_ref": article_ref(article_path),
        "evidence_refs": evidence_refs,
        "source_file_count": len(sources),
        "loader_ref_count": len(loader),
        "diagnostics": diagnostics,
        "safety_flags": {
            "graph_write_allowed": False,
            "promotion_allowed": False,
            "production_import_attempted": False,
            "import_eligible": False,
        },
    }


def select_entries(limit: int) -> list[dict[str, Any]]:
    article_paths = sorted(ARTICLE_ROOT.rglob("article.json"))
    selected: list[dict[str, Any]] = []
    for article_path in article_paths:
        entry = candidate_entry(article_path)
        if entry is None:
            continue
        selected.append(entry)
        if len(selected) >= limit:
            break
    return selected


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--limit", type=int, default=5)
    parser.add_argument("--catalog", type=Path, default=DEFAULT_CATALOG)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    if args.limit < 3 or args.limit > 5:
        raise SystemExit("--limit must be between 3 and 5 for the M036 smoke")
    if not args.catalog.exists():
        raise SystemExit(f"catalog not found: {args.catalog}")
    entries = select_entries(args.limit)
    if len(entries) < 3:
        raise SystemExit(f"only selected {len(entries)} usable articles; need at least 3")

    payload = {
        "schema_version": "m036-real-corpus-smoke-manifest.v1",
        "catalog_ref": f"artifact:{args.catalog.relative_to(ROOT).as_posix()}",
        "article_count": len(entries),
        "articles": entries,
        "safety_flags": {
            "graph_write_allowed": False,
            "promotion_allowed": False,
            "production_import_attempted": False,
            "import_eligible": False,
        },
        "diagnostics": sorted({diagnostic for entry in entries for diagnostic in entry["diagnostics"]}),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    emit(f"selected_articles={len(entries)}")
    emit(f"output={args.output}")
    emit("graph_write_allowed=false promotion_allowed=false production_import_attempted=false import_eligible=false")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
