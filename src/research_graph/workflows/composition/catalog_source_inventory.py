"""M222 composition: multi-source catalog inventory from article.json tree.

Offline scan of data/article_catalog; no network; no import authorization.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from research_graph.application.corpus.catalog_source_inventory import (
    SCHEMA_VERSION,
    CatalogSourceInventoryPackage,
    build_catalog_source_inventory,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags

DEFAULT_CATALOG_ROOT = Path("data/article_catalog/article_catalog")


@dataclass(frozen=True, slots=True)
class CatalogSourceInventoryRequest:
    catalog_root: Path = DEFAULT_CATALOG_ROOT
    output_path: Path | None = None
    repo_root: Path = field(default_factory=lambda: Path("."))
    max_samples: int = 12
    max_articles: int | None = None


@dataclass(frozen=True, slots=True)
class CatalogSourceInventoryResult:
    schema_version: str
    package: CatalogSourceInventoryPackage
    articles_loaded: int
    article_json_paths: int
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()
    output_path: str | None = None

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("catalog inventory result cannot authorize import/writes")
        if self.package.import_eligible or self.package.graph_writes_allowed:
            raise ValueError("package cannot authorize import inside result")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "articles_loaded": self.articles_loaded,
            "article_json_paths": self.article_json_paths,
            "package": self.package.to_dict(),
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
            "output_path": self.output_path,
        }


def _resolve(path: Path, repo_root: Path) -> Path:
    if path.is_file() or path.is_dir() or path.is_absolute():
        return path
    return repo_root / path


def load_article_records(catalog_root: Path, *, max_articles: int | None = None) -> list[dict[str, Any]]:
    """Load article.json dicts under catalog_root (sorted paths)."""
    paths = sorted(catalog_root.rglob("article.json"))
    if max_articles is not None:
        paths = paths[: max(0, max_articles)]
    records: list[dict[str, Any]] = []
    for path in paths:
        try:
            obj = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if isinstance(obj, dict):
            records.append(obj)
    return records


def run_catalog_source_inventory(
    request: CatalogSourceInventoryRequest,
) -> CatalogSourceInventoryResult:
    repo = request.repo_root
    root = _resolve(request.catalog_root, repo)
    if not root.is_dir():
        raise FileNotFoundError(f"catalog root missing: {root}")

    path_count = sum(1 for _ in root.rglob("article.json"))
    records = load_article_records(root, max_articles=request.max_articles)
    package = build_catalog_source_inventory(records, max_samples=request.max_samples)
    diag = (
        f"catalog_root:{root}",
        f"article_json_paths:{path_count}",
        f"articles_loaded:{len(records)}",
        f"variant_count:{package.variant_count}",
        f"non_arxiv:{package.non_arxiv_articles}",
        "import_write_fail_closed",
        "no_network",
    )

    out_path = request.output_path
    if out_path is not None:
        out_path = _resolve(out_path, repo)
        out_path.parent.mkdir(parents=True, exist_ok=True)

    result = CatalogSourceInventoryResult(
        schema_version=SCHEMA_VERSION,
        package=package,
        articles_loaded=len(records),
        article_json_paths=path_count,
        diagnostics=diag,
        output_path=str(out_path) if out_path else None,
    )
    if out_path is not None:
        out_path.write_text(
            json.dumps(result.to_dict(), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    return result


__all__ = [
    "DEFAULT_CATALOG_ROOT",
    "CatalogSourceInventoryRequest",
    "CatalogSourceInventoryResult",
    "load_article_records",
    "run_catalog_source_inventory",
]
