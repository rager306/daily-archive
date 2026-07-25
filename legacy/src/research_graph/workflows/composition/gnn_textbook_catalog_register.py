"""M223: register GNN textbook HTML chapters into canonical article catalog.

Copies local HTML into article_catalog/gnn_textbook/html/<key>/source/,
writes article.v00.01 records, rebuilds index.json via M025 helper.
Never authorizes import/writes.
"""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from research_graph.application.corpus.non_arxiv_source_records import (
    build_gnn_textbook_article_record,
    build_multi_source_selection,
    fingerprint_html_bytes,
    gnn_chapter_article_key,
)
from research_graph.application.profiles.textbook import (
    GNN_TEXTBOOK_BASE_URL,
    GNN_TEXTBOOK_SOURCE_CODE,
    GNN_TEXTBOOK_TITLE,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags

SCHEMA_VERSION = "gnn-textbook-catalog-register.v1"
DEFAULT_CATALOG_ROOT = Path("data/article_catalog")
DEFAULT_SOURCE_DIR = Path("artifacts/m222-gnn-textbook/source")

# Map local m222 filenames → (chapter_slug, title)
DEFAULT_CHAPTER_MAP: dict[str, tuple[str, str]] = {
    "about.html": ("about/", f"About — {GNN_TEXTBOOK_TITLE}"),
    "chapters__00-math-prerequisites.html": (
        "chapters/00-math-prerequisites/",
        "Chapter 00: Math Prerequisites",
    ),
    "chapters__01-intro-to-graphs.html": (
        "chapters/01-intro-to-graphs/",
        "Chapter 01: Introduction to Graphs",
    ),
    "chapters__02-graph-properties-and-features.html": (
        "chapters/02-graph-properties-and-features/",
        "Chapter 02: Graph Properties and Features",
    ),
}


@dataclass(frozen=True, slots=True)
class GnnTextbookCatalogRegisterRequest:
    catalog_root: Path = DEFAULT_CATALOG_ROOT
    source_dir: Path = DEFAULT_SOURCE_DIR
    chapter_map: dict[str, tuple[str, str]] = field(
        default_factory=lambda: dict(DEFAULT_CHAPTER_MAP)
    )
    rebuild_index: bool = True
    output_path: Path | None = None
    selection_output_path: Path | None = None
    repo_root: Path = field(default_factory=lambda: Path("."))


@dataclass(frozen=True, slots=True)
class RegisteredChapter:
    article_ref: str
    article_key: str
    chapter_slug: str
    html_path: str
    sha256: str
    byte_size: int
    status: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "article_ref": self.article_ref,
            "article_key": self.article_key,
            "chapter_slug": self.chapter_slug,
            "html_path": self.html_path,
            "sha256": self.sha256,
            "byte_size": self.byte_size,
            "status": self.status,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


@dataclass(frozen=True, slots=True)
class GnnTextbookCatalogRegisterResult:
    schema_version: str
    registered: tuple[RegisteredChapter, ...]
    skipped: tuple[str, ...]
    index_updated: bool
    index_article_count: int | None
    selection: dict[str, Any]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()
    output_path: str | None = None

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("gnn catalog register cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "registered": [r.to_dict() for r in self.registered],
            "skipped": list(self.skipped),
            "index_updated": self.index_updated,
            "index_article_count": self.index_article_count,
            "selection": dict(self.selection),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
            "output_path": self.output_path,
            "note": "gnn_textbook catalog registration; not graph import",
        }


def _resolve(path: Path, repo_root: Path) -> Path:
    if path.is_file() or path.is_dir() or path.is_absolute():
        return path
    return repo_root / path


def _atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    tmp.replace(path)


def _rebuild_catalog_index(catalog_root: Path) -> tuple[bool, int | None, list[str]]:
    """Rebuild data/article_catalog/index.json from article.json tree."""
    index_path = catalog_root / "index.json"
    catalog_manifest = catalog_root / "catalog.json"
    if not index_path.is_file() or not catalog_manifest.is_file():
        return False, None, ["index_or_catalog_manifest_missing"]

    # composition is under src/research_graph/workflows/composition → parents[4]=repo root
    repo_root = Path(__file__).resolve().parents[4]
    scripts_dir = repo_root / "scripts"
    if str(scripts_dir) not in sys.path:
        sys.path.insert(0, str(scripts_dir))
    from verify_m025_article_catalog import (  # type: ignore[import-not-found]  # noqa: PLC0415
        rebuild_index_from_articles,
    )

    existing = json.loads(index_path.read_text(encoding="utf-8"))
    rebuilt, diagnostics = rebuild_index_from_articles(catalog_manifest, existing)
    _atomic_write_text(
        index_path, json.dumps(rebuilt, indent=2, sort_keys=False) + "\n"
    )
    count = (
        len(rebuilt["articles"])
        if isinstance(rebuilt.get("articles"), list)
        else None
    )
    diag_msgs = []
    for d in diagnostics:
        if isinstance(d, dict):
            diag_msgs.append(str(d.get("code") or d.get("message") or d))
        else:
            diag_msgs.append(str(d))
    return True, count, diag_msgs


def register_gnn_textbook_chapters(
    request: GnnTextbookCatalogRegisterRequest,
) -> GnnTextbookCatalogRegisterResult:
    """Register local GNN HTML chapters into canonical catalog + optional index rebuild."""
    repo = request.repo_root
    catalog_root = _resolve(request.catalog_root, repo)
    source_dir = _resolve(request.source_dir, repo)
    records_root = catalog_root / "article_catalog" / GNN_TEXTBOOK_SOURCE_CODE / "html"

    registered: list[RegisteredChapter] = []
    skipped: list[str] = []
    selection_rows: list[dict[str, Any]] = []
    diag: list[str] = [
        f"source_dir:{source_dir}",
        f"catalog_root:{catalog_root}",
        "import_write_fail_closed",
    ]

    if not source_dir.is_dir():
        diag.append("source_dir_missing")
    else:
        for filename, (chapter_slug, title) in sorted(request.chapter_map.items()):
            src = source_dir / filename
            if not src.is_file():
                skipped.append(f"missing:{filename}")
                continue
            raw = src.read_bytes()
            sha, size = fingerprint_html_bytes(raw)
            article_key = gnn_chapter_article_key(chapter_slug)
            article_dir = records_root / article_key
            dest_html = article_dir / "source" / "chapter.html"
            dest_html.parent.mkdir(parents=True, exist_ok=True)
            # idempotent copy
            dest_html.write_bytes(raw)
            record = build_gnn_textbook_article_record(
                chapter_slug=chapter_slug,
                title=title,
                html_rel_path="source/chapter.html",
                html_sha256=sha,
                html_byte_size=size,
                canonical_url=GNN_TEXTBOOK_BASE_URL.rstrip("/")
                + "/"
                + chapter_slug.lstrip("/"),
            )
            article_json = article_dir / "article.json"
            _atomic_write_text(
                article_json, json.dumps(record, indent=2, sort_keys=False) + "\n"
            )
            article_ref = str(record["catalog_path"])
            registered.append(
                RegisteredChapter(
                    article_ref=article_ref,
                    article_key=article_key,
                    chapter_slug=chapter_slug,
                    html_path=str(dest_html),
                    sha256=sha,
                    byte_size=size,
                    status="registered",
                )
            )
            selection_rows.append(
                {
                    "article_ref": article_ref,
                    "source_code": GNN_TEXTBOOK_SOURCE_CODE,
                    "article_key": article_key,
                    "title": title,
                    "primary_path": str(dest_html),
                    "source_format": "html",
                    "domain_profile": "textbook",
                }
            )
            diag.append(f"registered:{article_key}")

    index_updated = False
    index_count: int | None = None
    if request.rebuild_index and registered:
        try:
            index_updated, index_count, rebuild_diags = _rebuild_catalog_index(
                catalog_root
            )
            diag.append(f"index_updated:{index_updated}")
            diag.append(f"index_article_count:{index_count}")
            diag.extend(f"rebuild:{d}" for d in rebuild_diags[:20])
        except Exception as exc:  # noqa: BLE001 - surface rebuild failure
            diag.append(f"rebuild_error:{type(exc).__name__}:{exc}")

    # include company_blog row in selection for multi-source package
    blog_ref = "company_blog/cs-ir/pageindex_zhang2025pageindex"
    blog_html = (
        catalog_root
        / "article_catalog"
        / "company_blog"
        / "cs-ir"
        / "pageindex_zhang2025pageindex"
        / "source"
        / "article.html"
    )
    if blog_html.is_file():
        selection_rows.insert(
            0,
            {
                "article_ref": blog_ref,
                "source_code": "company_blog",
                "article_key": "pageindex_zhang2025pageindex",
                "title": "PageIndex: Next-Generation Vectorless, Reasoning-based RAG",
                "primary_path": str(blog_html),
                "source_format": "html",
                "domain_profile": "paper",
            },
        )

    selection = build_multi_source_selection(selection_rows)

    out_path = request.output_path
    if out_path is not None:
        out_path = _resolve(out_path, repo)
        out_path.parent.mkdir(parents=True, exist_ok=True)

    sel_out = request.selection_output_path
    if sel_out is not None:
        sel_out = _resolve(sel_out, repo)
        sel_out.parent.mkdir(parents=True, exist_ok=True)
        _atomic_write_text(
            sel_out, json.dumps(selection, indent=2, sort_keys=True) + "\n"
        )
        diag.append(f"selection_path:{sel_out}")

    result = GnnTextbookCatalogRegisterResult(
        schema_version=SCHEMA_VERSION,
        registered=tuple(registered),
        skipped=tuple(skipped),
        index_updated=index_updated,
        index_article_count=index_count,
        selection=selection,
        diagnostics=tuple(diag),
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
    "DEFAULT_CHAPTER_MAP",
    "DEFAULT_SOURCE_DIR",
    "SCHEMA_VERSION",
    "GnnTextbookCatalogRegisterRequest",
    "GnnTextbookCatalogRegisterResult",
    "RegisteredChapter",
    "register_gnn_textbook_chapters",
]
