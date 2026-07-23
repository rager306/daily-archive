"""M222 GNN textbook HTML ingest composition (ADR-032 first non-paper domain).

Offline: load local HTML chapter via M207 universal_source path.
Optional live: bounded stdlib fetch of seed pages into a work dir (gitignored).
Never authorizes import/writes. No BeautifulSoup dependency.
"""

from __future__ import annotations

import json
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any
from xml.etree import ElementTree as ET

from research_graph.application.profiles.textbook import (
    DOMAIN_PROFILE,
    GNN_TEXTBOOK_BASE_URL,
    GNN_TEXTBOOK_SEED_PATHS,
    GNN_TEXTBOOK_SITEMAP_URL,
    GNN_TEXTBOOK_SOURCE_CODE,
    GNN_TEXTBOOK_TITLE,
    textbook_profile_dict,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.workflows.composition.universal_source import (
    StructuredSourceBundle,
    load_local_html_chapter,
    structure_loaded_source,
)

SCHEMA_VERSION = "m222-gnn-textbook-ingest.v1"
DEFAULT_WORK_DIR = Path("artifacts/m222-gnn-textbook")


@dataclass(frozen=True, slots=True)
class GnnChapterIngestResult:
    chapter_id: str
    source_path: str
    domain_profile: str
    source_code: str
    source_kind: str
    load_outcome: str
    body_chars: int
    structure: StructuredSourceBundle | None
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("gnn chapter ingest cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "chapter_id": self.chapter_id,
            "source_path": self.source_path,
            "domain_profile": self.domain_profile,
            "source_code": self.source_code,
            "source_kind": self.source_kind,
            "load_outcome": self.load_outcome,
            "body_chars": self.body_chars,
            "structure": {
                "page_index_node_count": self.structure.page_index_node_count,
                "chunk_count": self.structure.chunk_count,
                "evidence_count": self.structure.evidence_count,
                "source_kind": self.structure.source_kind,
            }
            if self.structure is not None
            else None,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class GnnTextbookIngestRequest:
    chapter_path: Path | None = None
    chapter_id: str = "gnn-chapter-fixture"
    work_dir: Path = DEFAULT_WORK_DIR
    allow_network: bool = False
    seed_paths: tuple[str, ...] = GNN_TEXTBOOK_SEED_PATHS
    max_fetch: int = 5
    timeout_s: float = 20.0
    output_path: Path | None = None
    repo_root: Path = field(default_factory=lambda: Path("."))


@dataclass(frozen=True, slots=True)
class GnnTextbookIngestPackage:
    schema_version: str
    domain_profile: str
    source_code: str
    title: str
    base_url: str
    chapters: tuple[GnnChapterIngestResult, ...]
    fetched_paths: tuple[str, ...]
    profile: dict[str, Any]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()
    output_path: str | None = None

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("gnn textbook package cannot authorize import/writes")
        for ch in self.chapters:
            if ch.import_eligible or ch.graph_writes_allowed:
                raise ValueError("chapter cannot authorize import inside package")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "domain_profile": self.domain_profile,
            "source_code": self.source_code,
            "title": self.title,
            "base_url": self.base_url,
            "chapters": [c.to_dict() for c in self.chapters],
            "fetched_paths": list(self.fetched_paths),
            "profile": dict(self.profile),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
            "output_path": self.output_path,
            "note": "ADR-032 textbook tracer; candidate-only; not graph import",
        }


def _resolve(path: Path, repo_root: Path) -> Path:
    if path.is_file() or path.is_dir() or path.is_absolute():
        return path
    return repo_root / path


def _slug_from_path(rel: str) -> str:
    cleaned = rel.strip("/").replace("/", "__")
    return cleaned or "index"


def ingest_local_gnn_chapter(
    path: Path,
    *,
    chapter_id: str,
) -> GnnChapterIngestResult:
    """Load + structure one local HTML chapter (no network)."""
    load = load_local_html_chapter(path, paper_id=chapter_id)
    diag = [
        f"domain_profile:{DOMAIN_PROFILE}",
        f"source_code:{GNN_TEXTBOOK_SOURCE_CODE}",
        f"load_outcome:{load.outcome}",
        "network_fetch_attempted:false",
    ]
    structure: StructuredSourceBundle | None = None
    body_chars = len(load.text or "")
    if load.outcome == "loaded" and load.text:
        try:
            structure = structure_loaded_source(load, paper_id=chapter_id)
            diag.append(f"chunks:{structure.chunk_count}")
            diag.append(f"source_kind:{structure.source_kind}")
        except Exception as exc:  # noqa: BLE001 - fail-closed chapter structure
            diag.append(f"structure_error:{type(exc).__name__}")
    return GnnChapterIngestResult(
        chapter_id=chapter_id,
        source_path=str(path),
        domain_profile=DOMAIN_PROFILE,
        source_code=GNN_TEXTBOOK_SOURCE_CODE,
        source_kind="html",
        load_outcome=str(load.outcome),
        body_chars=body_chars,
        structure=structure,
        diagnostics=tuple(diag),
    )


def parse_sitemap_locs(xml_text: str) -> tuple[str, ...]:
    """Extract <loc> URLs from sitemap XML (stdlib)."""
    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError:
        return ()
    locs: list[str] = []
    for el in root.iter():
        if el.tag.endswith("loc") and el.text:
            locs.append(el.text.strip())
    return tuple(locs)


def _fetch_bytes(url: str, *, timeout_s: float) -> bytes:
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "daily-archive-m222-gnn-textbook/1.0"},
        method="GET",
    )
    with urllib.request.urlopen(req, timeout=timeout_s) as resp:  # noqa: S310 - operator opt-in
        return resp.read()


def fetch_gnn_textbook_seeds(
    work_dir: Path,
    *,
    seed_paths: tuple[str, ...] = GNN_TEXTBOOK_SEED_PATHS,
    max_fetch: int = 5,
    timeout_s: float = 20.0,
    base_url: str = GNN_TEXTBOOK_BASE_URL,
) -> tuple[tuple[str, ...], tuple[str, ...]]:
    """Fetch bounded seed pages into work_dir/source/. Returns (saved_relpaths, diagnostics)."""
    source_dir = work_dir / "source"
    source_dir.mkdir(parents=True, exist_ok=True)
    saved: list[str] = []
    diag: list[str] = [f"base_url:{base_url}", f"max_fetch:{max_fetch}"]
    for rel in seed_paths[: max(0, max_fetch)]:
        url = base_url.rstrip("/") + "/" + rel.lstrip("/")
        slug = _slug_from_path(rel)
        dest = source_dir / f"{slug}.html"
        try:
            raw = _fetch_bytes(url, timeout_s=timeout_s)
            # basic HTML sanity
            text = raw.decode("utf-8", errors="replace")
            if "<html" not in text.casefold() and "<body" not in text.casefold():
                diag.append(f"skip_not_html:{rel}")
                continue
            dest.write_text(text, encoding="utf-8")
            saved.append(str(dest.relative_to(work_dir)))
            diag.append(f"fetched:{rel}:{len(text)}")
        except (urllib.error.URLError, TimeoutError, OSError) as exc:
            diag.append(f"fetch_error:{rel}:{type(exc).__name__}")
    return tuple(saved), tuple(diag)


def run_gnn_textbook_ingest(
    request: GnnTextbookIngestRequest,
) -> GnnTextbookIngestPackage:
    """Ingest local chapter and optionally fetch seed pages."""
    repo = request.repo_root
    work = _resolve(request.work_dir, repo)
    work.mkdir(parents=True, exist_ok=True)

    fetched: tuple[str, ...] = ()
    fetch_diag: tuple[str, ...] = ()
    if request.allow_network:
        fetched, fetch_diag = fetch_gnn_textbook_seeds(
            work,
            seed_paths=request.seed_paths,
            max_fetch=request.max_fetch,
            timeout_s=request.timeout_s,
        )

    chapters: list[GnnChapterIngestResult] = []
    if request.chapter_path is not None:
        chapter_path = _resolve(request.chapter_path, repo)
        chapters.append(
            ingest_local_gnn_chapter(chapter_path, chapter_id=request.chapter_id)
        )
    else:
        # Prefer explicitly fetched seeds, else any html under work/source
        source_dir = work / "source"
        candidates = sorted(source_dir.glob("*.html")) if source_dir.is_dir() else []
        for path in candidates:
            chapters.append(
                ingest_local_gnn_chapter(
                    path, chapter_id=f"gnn-{path.stem}"
                )
            )

    diag = (
        f"domain_profile:{DOMAIN_PROFILE}",
        f"source_code:{GNN_TEXTBOOK_SOURCE_CODE}",
        f"chapters:{len(chapters)}",
        f"allow_network:{request.allow_network}",
        "import_write_fail_closed",
        *fetch_diag,
    )

    out_path = request.output_path
    if out_path is not None:
        out_path = _resolve(out_path, repo)
        out_path.parent.mkdir(parents=True, exist_ok=True)

    package = GnnTextbookIngestPackage(
        schema_version=SCHEMA_VERSION,
        domain_profile=DOMAIN_PROFILE,
        source_code=GNN_TEXTBOOK_SOURCE_CODE,
        title=GNN_TEXTBOOK_TITLE,
        base_url=GNN_TEXTBOOK_BASE_URL,
        chapters=tuple(chapters),
        fetched_paths=fetched,
        profile=textbook_profile_dict(),
        diagnostics=diag,
        output_path=str(out_path) if out_path else None,
    )
    if out_path is not None:
        out_path.write_text(
            json.dumps(package.to_dict(), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    return package


def sitemap_chapter_paths(
    *,
    timeout_s: float = 20.0,
    max_paths: int = 50,
) -> tuple[str, ...]:
    """Optional helper: list chapter paths from live sitemap (network)."""
    raw = _fetch_bytes(GNN_TEXTBOOK_SITEMAP_URL, timeout_s=timeout_s).decode(
        "utf-8", errors="replace"
    )
    locs = parse_sitemap_locs(raw)
    base = GNN_TEXTBOOK_BASE_URL.rstrip("/") + "/"
    paths: list[str] = []
    for loc in locs:
        if not loc.startswith(base):
            continue
        rel = loc[len(base) :]
        if rel.startswith("chapters/") and not rel.endswith("quiz/") and not rel.endswith(
            "references/"
        ):
            paths.append(rel if rel.endswith("/") else rel + "/")
    # de-dupe preserve order
    seen: set[str] = set()
    out: list[str] = []
    for p in paths:
        if p not in seen:
            seen.add(p)
            out.append(p)
        if len(out) >= max_paths:
            break
    return tuple(out)


__all__ = [
    "DEFAULT_WORK_DIR",
    "SCHEMA_VERSION",
    "GnnChapterIngestResult",
    "GnnTextbookIngestPackage",
    "GnnTextbookIngestRequest",
    "fetch_gnn_textbook_seeds",
    "ingest_local_gnn_chapter",
    "parse_sitemap_locs",
    "run_gnn_textbook_ingest",
    "sitemap_chapter_paths",
]
