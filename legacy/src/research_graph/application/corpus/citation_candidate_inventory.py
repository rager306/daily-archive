"""Pure citation candidate inventory over GROBID hybrid artifacts.

Loads nothing from disk itself — operates on already-parsed dict/list rows.
Fail-closed: never sets import_eligible or graph_writes_allowed true.
Not a review gate and not graph truth.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

SCHEMA_VERSION = "citation-candidate-inventory.v1"


def _nonempty_str(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _has_authors(row: dict[str, Any]) -> bool:
    authors = row.get("authors")
    if not isinstance(authors, list) or not authors:
        return False
    for a in authors:
        if not isinstance(a, dict):
            continue
        if _nonempty_str(a.get("full_name")) or _nonempty_str(a.get("full")):
            return True
        if _nonempty_str(a.get("surname")):
            return True
    return False


def _has_idno(row: dict[str, Any]) -> bool:
    idnos = row.get("idnos")
    if isinstance(idnos, dict) and any(_nonempty_str(str(v)) for v in idnos.values()):
        return True
    if _nonempty_str(row.get("doi")):
        return True
    return False


def _has_date(row: dict[str, Any]) -> bool:
    return _nonempty_str(row.get("date")) or _nonempty_str(row.get("when"))


def _has_venue(row: dict[str, Any]) -> bool:
    return _nonempty_str(row.get("venue_or_monogr")) or _nonempty_str(row.get("journal"))


@dataclass(frozen=True, slots=True)
class PaperCitationInventory:
    paper_id: str
    citation_count: int
    with_title: int
    with_authors: int
    with_idno: int
    with_date: int
    with_venue: int
    empty_title: int
    header_title_present: bool
    header_author_count: int
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("citation inventory paper row cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        n = max(self.citation_count, 0)
        return {
            "paper_id": self.paper_id,
            "citation_count": self.citation_count,
            "with_title": self.with_title,
            "with_authors": self.with_authors,
            "with_idno": self.with_idno,
            "with_date": self.with_date,
            "with_venue": self.with_venue,
            "empty_title": self.empty_title,
            "title_coverage": (self.with_title / n) if n else 0.0,
            "author_coverage": (self.with_authors / n) if n else 0.0,
            "idno_coverage": (self.with_idno / n) if n else 0.0,
            "header_title_present": self.header_title_present,
            "header_author_count": self.header_author_count,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


@dataclass(frozen=True, slots=True)
class CitationInventoryPackage:
    schema_version: str
    paper_count: int
    papers_with_citations_file: int
    citation_total: int
    with_title: int
    with_authors: int
    with_idno: int
    with_date: int
    with_venue: int
    empty_title: int
    papers: tuple[PaperCitationInventory, ...]
    diagnostics: tuple[str, ...] = ()
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("citation inventory package cannot authorize import/writes")
        for p in self.papers:
            if p.import_eligible or p.graph_writes_allowed:
                raise ValueError("nested paper inventory cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        n = max(self.citation_total, 0)
        return {
            "schema_version": self.schema_version,
            "paper_count": self.paper_count,
            "papers_with_citations_file": self.papers_with_citations_file,
            "citation_total": self.citation_total,
            "with_title": self.with_title,
            "with_authors": self.with_authors,
            "with_idno": self.with_idno,
            "with_date": self.with_date,
            "with_venue": self.with_venue,
            "empty_title": self.empty_title,
            "title_coverage": (self.with_title / n) if n else 0.0,
            "author_coverage": (self.with_authors / n) if n else 0.0,
            "idno_coverage": (self.with_idno / n) if n else 0.0,
            "papers": [p.to_dict() for p in self.papers],
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": "candidate inventory only; not review gate; not graph import",
        }


def inventory_paper_citations(
    *,
    paper_id: str,
    header: dict[str, Any] | None,
    citations: list[dict[str, Any]] | None,
) -> PaperCitationInventory:
    """Inventory one paper's structured citation candidates."""
    rows = [r for r in (citations or []) if isinstance(r, dict)]
    with_title = sum(1 for r in rows if _nonempty_str(r.get("title")))
    with_authors = sum(1 for r in rows if _has_authors(r))
    with_idno = sum(1 for r in rows if _has_idno(r))
    with_date = sum(1 for r in rows if _has_date(r))
    with_venue = sum(1 for r in rows if _has_venue(r))
    empty_title = sum(1 for r in rows if not _nonempty_str(r.get("title")))
    header_title = False
    header_authors = 0
    if isinstance(header, dict):
        header_title = _nonempty_str(header.get("title"))
        authors = header.get("authors")
        if isinstance(authors, list):
            header_authors = sum(1 for a in authors if isinstance(a, dict))
    return PaperCitationInventory(
        paper_id=paper_id,
        citation_count=len(rows),
        with_title=with_title,
        with_authors=with_authors,
        with_idno=with_idno,
        with_date=with_date,
        with_venue=with_venue,
        empty_title=empty_title,
        header_title_present=header_title,
        header_author_count=header_authors,
    )


def build_citation_inventory(
    papers: list[dict[str, Any]] | tuple[dict[str, Any], ...],
) -> CitationInventoryPackage:
    """Build aggregate inventory from list of {paper_id, header?, citations?}.

    Missing citations list → treated as zero rows for that paper (honest).
    """
    rows: list[PaperCitationInventory] = []
    diag: list[str] = ["source:structured_candidates", "fail_closed"]
    papers_with_file = 0
    for raw in papers:
        if not isinstance(raw, dict):
            continue
        paper_id = str(raw.get("paper_id") or "").strip()
        header = raw.get("header") if isinstance(raw.get("header"), dict) else None
        cites_raw = raw.get("citations")
        has_file = raw.get("has_citations_file")
        cites: list[dict[str, Any]] | None
        if cites_raw is None:
            cites = None
            if has_file is True:
                papers_with_file += 1
        elif isinstance(cites_raw, list):
            cites = [c for c in cites_raw if isinstance(c, dict)]
            # list present (even empty) means citations file was loaded
            if has_file is not False:
                papers_with_file += 1
        else:
            cites = []
            diag.append(f"bad_citations_type:{paper_id or '?'}")
        rows.append(
            inventory_paper_citations(
                paper_id=paper_id or "unknown",
                header=header,
                citations=cites,
            )
        )

    citation_total = sum(r.citation_count for r in rows)
    return CitationInventoryPackage(
        schema_version=SCHEMA_VERSION,
        paper_count=len(rows),
        papers_with_citations_file=papers_with_file,
        citation_total=citation_total,
        with_title=sum(r.with_title for r in rows),
        with_authors=sum(r.with_authors for r in rows),
        with_idno=sum(r.with_idno for r in rows),
        with_date=sum(r.with_date for r in rows),
        with_venue=sum(r.with_venue for r in rows),
        empty_title=sum(r.empty_title for r in rows),
        papers=tuple(rows),
        diagnostics=tuple(diag),
    )


__all__ = [
    "SCHEMA_VERSION",
    "CitationInventoryPackage",
    "PaperCitationInventory",
    "build_citation_inventory",
    "inventory_paper_citations",
]
