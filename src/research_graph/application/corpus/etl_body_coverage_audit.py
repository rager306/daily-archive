"""Wave A ETL body coverage audit (M241/M242).

Read-only: catalog index + optional local article.json paths + hybrid body
markdown under body roots. Never network, never authorizes import.

Hybrid body convention (M213/M216)::

    {body_root}/{paper_id}/body/{paper_id}.hybrid.body.md

M242: separate **catalog-joined unique paper_ids** (``hybrid_body_found``)
from **raw artifact file volume** across overlapping body roots
(``hybrid_body_artifact_files`` / ``hybrid_body_unique_paper_ids``).
"""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "etl-body-coverage-audit.v1"


def paper_id_for_article(article: Mapping[str, Any]) -> str:
    """Best-effort paper id for hybrid body path resolution."""
    ref = str(article.get("article_ref") or "").strip()
    if ref:
        # arxiv/cs-cl/1706.03762 → 1706.03762
        tail = ref.rstrip("/").split("/")[-1]
        if tail:
            return tail
    key = str(article.get("article_key") or "").strip()
    if key:
        return key
    return ""


def _load_articles(catalog_index_path: Path) -> list[dict[str, Any]]:
    raw = json.loads(catalog_index_path.read_text(encoding="utf-8"))
    if not isinstance(raw, dict):
        return []
    arts = raw.get("articles")
    if not isinstance(arts, list):
        return []
    return [a for a in arts if isinstance(a, dict)]


def _hybrid_body_path(body_root: Path, paper_id: str) -> Path:
    return body_root / paper_id / "body" / f"{paper_id}.hybrid.body.md"


def find_hybrid_body(paper_id: str, body_roots: Sequence[Path]) -> Path | None:
    """Return first existing hybrid body path for paper_id, else None."""
    if not paper_id:
        return None
    for root in body_roots:
        candidate = _hybrid_body_path(Path(root), paper_id)
        if candidate.is_file():
            return candidate
    return None


def scan_hybrid_body_artifacts(
    body_roots: Sequence[Path],
) -> tuple[int, int, dict[str, int]]:
    """Scan body roots for ``*.hybrid.body.md`` files.

    Returns:
        (artifact_file_count, unique_paper_id_count, files_by_root)
    Multi-root copies of the same paper_id count as multiple files but one unique id.
    """
    files_by_root: dict[str, int] = {}
    unique_ids: set[str] = set()
    total_files = 0
    for root in body_roots:
        root_p = Path(root)
        if not root_p.is_dir():
            files_by_root[str(root_p)] = 0
            continue
        n = 0
        for path in root_p.rglob("*.hybrid.body.md"):
            if not path.is_file():
                continue
            n += 1
            total_files += 1
            name = path.name
            if name.endswith(".hybrid.body.md"):
                unique_ids.add(name[: -len(".hybrid.body.md")])
        files_by_root[str(root_p)] = n
    return total_files, len(unique_ids), files_by_root



@dataclass(frozen=True, slots=True)
class MultiRootHybridCopyInventory:
    """Content taxonomy for multi-root hybrid body copies (read-only)."""

    multi_root_paper_id_count: int
    identical_content_count: int
    divergent_content_count: int
    sample_paper_ids: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("multi-root inventory cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "multi_root_paper_id_count": self.multi_root_paper_id_count,
            "identical_content_count": self.identical_content_count,
            "divergent_content_count": self.divergent_content_count,
            "sample_paper_ids": list(self.sample_paper_ids),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Identical multi-root copies are storage/process debt, not data corruption. "
                "Divergent copies require human review before expand/dedup."
            ),
        }


def inventory_multi_root_hybrid_copies(
    body_roots: Sequence[Path],
    *,
    sample_limit: int = 12,
) -> MultiRootHybridCopyInventory:
    """Classify multi-root hybrid bodies as identical vs divergent content.

    SHA-256 over full file bytes. Read-only; never deletes or authorizes import.
    """
    by_id: dict[str, list[Path]] = {}
    for root in body_roots:
        root_p = Path(root)
        if not root_p.is_dir():
            continue
        for path in root_p.rglob("*.hybrid.body.md"):
            if not path.is_file():
                continue
            name = path.name
            if not name.endswith(".hybrid.body.md"):
                continue
            pid = name[: -len(".hybrid.body.md")]
            by_id.setdefault(pid, []).append(path)

    multi_ids = sorted(pid for pid, paths in by_id.items() if len(paths) > 1)
    identical = 0
    divergent = 0
    samples: list[str] = []
    for pid in multi_ids:
        hashes: set[str] = set()
        for path in by_id[pid]:
            try:
                digest = hashlib.sha256(path.read_bytes()).hexdigest()
            except OSError:
                digest = f"unreadable:{path}"
            hashes.add(digest)
        if len(hashes) <= 1:
            identical += 1
        else:
            divergent += 1
        if len(samples) < sample_limit:
            samples.append(pid)

    diagnostics = (
        f"multi_root_ids:{len(multi_ids)}",
        f"identical_content:{identical}",
        f"divergent_content:{divergent}",
        "import_write_fail_closed",
        "wave_a_coverage_only",
    )
    return MultiRootHybridCopyInventory(
        multi_root_paper_id_count=len(multi_ids),
        identical_content_count=identical,
        divergent_content_count=divergent,
        sample_paper_ids=tuple(samples),
        diagnostics=diagnostics,
    )

@dataclass(frozen=True, slots=True)
class EtlBodyCoverageSample:
    article_ref: str
    source_code: str
    paper_id: str
    article_json_present: bool
    hybrid_body_present: bool
    hybrid_body_path: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "article_ref": self.article_ref,
            "source_code": self.source_code,
            "paper_id": self.paper_id,
            "article_json_present": self.article_json_present,
            "hybrid_body_present": self.hybrid_body_present,
            "hybrid_body_path": self.hybrid_body_path,
            "import_eligible": False,
        }


@dataclass(frozen=True, slots=True)
class EtlBodyCoveragePackage:
    schema_version: str
    article_count: int
    by_source_code: dict[str, int]
    hybrid_body_found: int
    hybrid_body_missing: int
    article_json_found: int
    article_json_missing: int
    body_roots_scanned: int
    gaps: tuple[str, ...]
    samples: tuple[EtlBodyCoverageSample, ...]
    diagnostics: tuple[str, ...]
    # M242: artifact volume (may exceed unique catalog join due to multi-root copies)
    hybrid_body_artifact_files: int = 0
    hybrid_body_unique_paper_ids: int = 0
    hybrid_body_files_by_root: dict[str, int] | None = None
    multi_root_paper_id_count: int = 0
    multi_root_identical_content_count: int = 0
    multi_root_divergent_content_count: int = 0
    multi_root_sample_paper_ids: tuple[str, ...] = ()
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("etl body coverage audit cannot authorize import/writes")

    @property
    def hybrid_body_fraction(self) -> float:
        if self.article_count <= 0:
            return 0.0
        return round(self.hybrid_body_found / self.article_count, 4)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "article_count": self.article_count,
            "by_source_code": dict(self.by_source_code),
            "hybrid_body_found": self.hybrid_body_found,
            "hybrid_body_missing": self.hybrid_body_missing,
            "hybrid_body_fraction": self.hybrid_body_fraction,
            "hybrid_body_artifact_files": self.hybrid_body_artifact_files,
            "hybrid_body_unique_paper_ids": self.hybrid_body_unique_paper_ids,
            "hybrid_body_files_by_root": dict(self.hybrid_body_files_by_root or {}),
            "multi_root_paper_id_count": self.multi_root_paper_id_count,
            "multi_root_identical_content_count": self.multi_root_identical_content_count,
            "multi_root_divergent_content_count": self.multi_root_divergent_content_count,
            "multi_root_sample_paper_ids": list(self.multi_root_sample_paper_ids),
            "article_json_found": self.article_json_found,
            "article_json_missing": self.article_json_missing,
            "body_roots_scanned": self.body_roots_scanned,
            "gaps": list(self.gaps),
            "samples": [s.to_dict() for s in self.samples],
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave A coverage audit only; not graph import; not extraction quality. "
                "hybrid_body_found = unique catalog join; "
                "hybrid_body_artifact_files = raw *.hybrid.body.md under roots."
            ),
        }


def audit_catalog_body_coverage(
    *,
    catalog_index_path: Path,
    body_roots: Sequence[Path] = (),
    catalog_root: Path | None = None,
    sample_limit: int = 12,
) -> EtlBodyCoveragePackage:
    """Audit catalog articles against hybrid body artifacts (read-only)."""
    index_path = Path(catalog_index_path)
    articles = _load_articles(index_path) if index_path.is_file() else []
    roots = tuple(Path(r) for r in body_roots)
    cat_root = Path(catalog_root) if catalog_root is not None else index_path.parent

    by_code: Counter[str] = Counter()
    hybrid_found = 0
    hybrid_missing = 0
    json_found = 0
    json_missing = 0
    samples: list[EtlBodyCoverageSample] = []
    gaps: list[str] = []

    if not index_path.is_file():
        gaps.append("catalog_index_missing")

    for art in articles:
        source = str(art.get("source_code") or "unknown").strip() or "unknown"
        by_code[source] += 1
        paper_id = paper_id_for_article(art)
        ref = str(art.get("article_ref") or art.get("article_key") or paper_id)

        rel = str(art.get("article_path") or "").strip()
        article_json_ok = False
        if rel:
            candidate = cat_root / rel
            # also try catalog_root parent layouts
            if not candidate.is_file() and (cat_root / "article_catalog").is_dir():
                candidate = cat_root / rel
            article_json_ok = candidate.is_file()
            # some indexes store path relative to data/article_catalog
            if not article_json_ok:
                alt = index_path.parent / rel
                article_json_ok = alt.is_file()
        if article_json_ok:
            json_found += 1
        else:
            json_missing += 1

        body_path = find_hybrid_body(paper_id, roots)
        if body_path is not None:
            hybrid_found += 1
        else:
            hybrid_missing += 1

        if len(samples) < sample_limit:
            samples.append(
                EtlBodyCoverageSample(
                    article_ref=ref,
                    source_code=source,
                    paper_id=paper_id,
                    article_json_present=article_json_ok,
                    hybrid_body_present=body_path is not None,
                    hybrid_body_path=str(body_path) if body_path else "",
                )
            )

    artifact_files, unique_ids, files_by_root = scan_hybrid_body_artifacts(roots)
    multi_inv = inventory_multi_root_hybrid_copies(roots)

    if articles and hybrid_found == 0 and roots:
        gaps.append("no_hybrid_bodies_under_body_roots")
    if articles and hybrid_found < len(articles) and roots:
        gaps.append("partial_hybrid_body_coverage")
    if not roots:
        gaps.append("no_body_roots_configured")
    if artifact_files > unique_ids and unique_ids > 0:
        gaps.append("multi_root_hybrid_body_copies")
        if (
            multi_inv.identical_content_count > 0
            and multi_inv.divergent_content_count == 0
        ):
            gaps.append("multi_root_identical_content_only")
        if multi_inv.divergent_content_count > 0:
            gaps.append("multi_root_divergent_content")

    diagnostics = (
        f"articles:{len(articles)}",
        f"hybrid_found:{hybrid_found}",
        f"hybrid_missing:{hybrid_missing}",
        f"hybrid_artifact_files:{artifact_files}",
        f"hybrid_unique_paper_ids:{unique_ids}",
        f"multi_root_ids:{multi_inv.multi_root_paper_id_count}",
        f"multi_root_identical:{multi_inv.identical_content_count}",
        f"multi_root_divergent:{multi_inv.divergent_content_count}",
        f"body_roots:{len(roots)}",
        "import_write_fail_closed",
        "wave_a_coverage_only",
    )

    return EtlBodyCoveragePackage(
        schema_version=SCHEMA_VERSION,
        article_count=len(articles),
        by_source_code=dict(sorted(by_code.items())),
        hybrid_body_found=hybrid_found,
        hybrid_body_missing=hybrid_missing,
        article_json_found=json_found,
        article_json_missing=json_missing,
        body_roots_scanned=len(roots),
        gaps=tuple(gaps),
        samples=tuple(samples),
        diagnostics=diagnostics,
        hybrid_body_artifact_files=artifact_files,
        hybrid_body_unique_paper_ids=unique_ids,
        hybrid_body_files_by_root=dict(sorted(files_by_root.items())),
        multi_root_paper_id_count=multi_inv.multi_root_paper_id_count,
        multi_root_identical_content_count=multi_inv.identical_content_count,
        multi_root_divergent_content_count=multi_inv.divergent_content_count,
        multi_root_sample_paper_ids=multi_inv.sample_paper_ids,
    )


__all__ = [
    "SCHEMA_VERSION",
    "EtlBodyCoveragePackage",
    "EtlBodyCoverageSample",
    "MultiRootHybridCopyInventory",
    "audit_catalog_body_coverage",
    "find_hybrid_body",
    "inventory_multi_root_hybrid_copies",
    "paper_id_for_article",
    "scan_hybrid_body_artifacts",
]
