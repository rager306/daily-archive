"""M220 composition: citation candidate inventory over hybrid body_root artifacts.

Reads precomputed hybrid.header.json + hybrid.citations.jsonl only.
Never starts GROBID/ODL. Never authorizes import/writes.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from research_graph.application.corpus.citation_candidate_inventory import (
    SCHEMA_VERSION,
    CitationInventoryPackage,
    build_citation_inventory,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags

DEFAULT_SELECTION = Path("artifacts/m213-hybrid-gate/selection.json")
DEFAULT_BODY_ROOT = Path("artifacts/m213-hybrid-gate/runs-live-scholarly")


@dataclass(frozen=True, slots=True)
class CitationInventoryRequest:
    hybrid_selection_path: Path = DEFAULT_SELECTION
    body_root: Path = DEFAULT_BODY_ROOT
    output_path: Path | None = None
    repo_root: Path = field(default_factory=lambda: Path("."))


@dataclass(frozen=True, slots=True)
class CitationInventoryResult:
    schema_version: str
    package: CitationInventoryPackage
    papers_scanned: int
    headers_loaded: int
    citations_files_loaded: int
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()
    output_path: str | None = None

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("citation inventory result cannot authorize import or writes")
        if self.package.import_eligible or self.package.graph_writes_allowed:
            raise ValueError("package cannot authorize import inside inventory result")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "papers_scanned": self.papers_scanned,
            "headers_loaded": self.headers_loaded,
            "citations_files_loaded": self.citations_files_loaded,
            "package": self.package.to_dict(),
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
            "output_path": self.output_path,
        }


def _resolve(path: Path, repo_root: Path) -> Path:
    if path.is_file() or path.is_dir() or path.is_absolute():
        return path
    return repo_root / path


def _load_header(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    try:
        obj = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return obj if isinstance(obj, dict) else None


def _load_citations(path: Path) -> tuple[list[dict[str, Any]] | None, bool]:
    """Return (rows, file_found)."""
    if not path.is_file():
        return None, False
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return None, False
    rows: list[dict[str, Any]] = []
    for line in lines:
        if not line.strip():
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(obj, dict):
            rows.append(obj)
    return rows, True


def run_citation_candidate_inventory(
    request: CitationInventoryRequest,
) -> CitationInventoryResult:
    """Scan selection papers under body_root and build inventory package."""
    repo = request.repo_root
    sel_path = _resolve(request.hybrid_selection_path, repo)
    body_root = _resolve(request.body_root, repo)
    if not sel_path.is_file():
        raise FileNotFoundError(f"hybrid selection missing: {sel_path}")

    selection = json.loads(sel_path.read_text(encoding="utf-8"))
    papers_raw = selection.get("papers")
    if not isinstance(papers_raw, list):
        papers_raw = []

    loaded: list[dict[str, Any]] = []
    headers_loaded = 0
    cites_loaded = 0
    for raw in papers_raw:
        if not isinstance(raw, dict):
            continue
        paper_id = str(raw.get("paper_id") or "").strip()
        if not paper_id:
            continue
        body_dir = body_root / paper_id / "body"
        header = _load_header(body_dir / f"{paper_id}.hybrid.header.json")
        cites, has_file = _load_citations(body_dir / f"{paper_id}.hybrid.citations.jsonl")
        if header is not None:
            headers_loaded += 1
        if has_file:
            cites_loaded += 1
        loaded.append(
            {
                "paper_id": paper_id,
                "header": header,
                "citations": cites if has_file else None,
                "has_citations_file": has_file,
            }
        )

    package = build_citation_inventory(loaded)
    diag = (
        f"selection:{sel_path}",
        f"body_root:{body_root}",
        f"papers_scanned:{len(loaded)}",
        f"headers_loaded:{headers_loaded}",
        f"citations_files_loaded:{cites_loaded}",
        f"citation_total:{package.citation_total}",
        "import_write_fail_closed",
        "no_live_sidecar_start",
        "not_review_gate",
    )

    out_path = request.output_path
    if out_path is not None:
        out_path = _resolve(out_path, repo)
        out_path.parent.mkdir(parents=True, exist_ok=True)

    result = CitationInventoryResult(
        schema_version=SCHEMA_VERSION,
        package=package,
        papers_scanned=len(loaded),
        headers_loaded=headers_loaded,
        citations_files_loaded=cites_loaded,
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
    "DEFAULT_BODY_ROOT",
    "DEFAULT_SELECTION",
    "CitationInventoryRequest",
    "CitationInventoryResult",
    "run_citation_candidate_inventory",
]
