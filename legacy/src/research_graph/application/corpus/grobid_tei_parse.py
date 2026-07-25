"""Pure GROBID TEI → header + citation candidates (M217).

Stdlib xml.etree only. No network, no graph write, no import authorization.
Separates teiHeader/sourceDesc biblStruct (paper self) from back/listBibl cites.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any
from xml.etree import ElementTree as ET

TEI_NS = "http://www.tei-c.org/ns/1.0"
SCHEMA_VERSION = "grobid-tei-candidates.v1"


def _local(tag: str) -> str:
    if "}" in tag:
        return tag.rsplit("}", 1)[-1]
    return tag


def _text(el: ET.Element | None) -> str:
    if el is None:
        return ""
    return " ".join("".join(el.itertext()).split()).strip()


def _find_child(parent: ET.Element, name: str) -> ET.Element | None:
    for ch in parent:
        if _local(ch.tag) == name:
            return ch
    return None


def _find_all(parent: ET.Element, name: str) -> list[ET.Element]:
    return [ch for ch in parent.iter() if _local(ch.tag) == name]


def _parent_map(root: ET.Element) -> dict[ET.Element, ET.Element]:
    parents: dict[ET.Element, ET.Element] = {}
    for p in root.iter():
        for ch in p:
            parents[ch] = p
    return parents


def _ancestor_locals(el: ET.Element, parents: dict[ET.Element, ET.Element]) -> tuple[str, ...]:
    parts: list[str] = []
    cur: ET.Element | None = el
    while cur is not None:
        parts.append(_local(cur.tag))
        cur = parents.get(cur)
    return tuple(reversed(parts))


def _pers_name(el: ET.Element) -> dict[str, str]:
    forename = ""
    surname = ""
    for ch in el.iter():
        loc = _local(ch.tag)
        if loc == "forename" and (ch.text or "").strip():
            # prefer first forename
            if not forename or ch.attrib.get("type") == "first":
                forename = (ch.text or "").strip()
        if loc == "surname" and (ch.text or "").strip():
            surname = (ch.text or "").strip()
    full = _text(el)
    return {
        "forename": forename,
        "surname": surname,
        "full": full or " ".join(x for x in (forename, surname) if x),
    }


def _parse_authors_from(container: ET.Element) -> list[dict[str, Any]]:
    authors: list[dict[str, Any]] = []
    for author in container.iter():
        if _local(author.tag) != "author":
            continue
        pers = None
        for ch in author:
            if _local(ch.tag) == "persName":
                pers = _pers_name(ch)
                break
        if pers is None:
            # sometimes persName nested deeper
            for ch in author.iter():
                if _local(ch.tag) == "persName":
                    pers = _pers_name(ch)
                    break
        email = ""
        for ch in author.iter():
            if _local(ch.tag) == "email" and (ch.text or "").strip():
                email = (ch.text or "").strip()
                break
        affs: list[str] = []
        for ch in author.iter():
            if _local(ch.tag) == "affiliation":
                t = _text(ch)
                if t:
                    affs.append(t)
        if not pers and not email and not affs:
            continue
        authors.append(
            {
                "forename": (pers or {}).get("forename", ""),
                "surname": (pers or {}).get("surname", ""),
                "full_name": (pers or {}).get("full", ""),
                "email": email,
                "affiliations": affs,
            }
        )
    return authors


def _idnos(container: ET.Element) -> dict[str, str]:
    out: dict[str, str] = {}
    for el in container.iter():
        if _local(el.tag) != "idno":
            continue
        typ = (el.attrib.get("type") or "id").strip()
        val = (el.text or "").strip() or _text(el)
        if val:
            out[typ] = val
    return out


def _bibl_title(bibl: ET.Element) -> str:
    # Prefer analytic title level=a, else any title
    titles: list[tuple[str, str, str]] = []
    for el in bibl.iter():
        if _local(el.tag) != "title":
            continue
        t = (el.text or "").strip() or _text(el)
        if not t:
            continue
        titles.append((el.attrib.get("level") or "", el.attrib.get("type") or "", t))
    for level, typ, t in titles:
        if level == "a" or typ == "main":
            return t
    return titles[0][2] if titles else ""


def _parse_bibl_struct(bibl: ET.Element, *, index: int) -> dict[str, Any]:
    authors = _parse_authors_from(bibl)
    title = _bibl_title(bibl)
    date = ""
    when = ""
    for el in bibl.iter():
        if _local(el.tag) == "date":
            when = el.attrib.get("when") or ""
            date = (el.text or "").strip() or when
            if when or date:
                break
    monogr = ""
    for el in bibl.iter():
        if _local(el.tag) == "title" and el.attrib.get("level") == "j":
            monogr = (el.text or "").strip() or _text(el)
            break
    if not monogr:
        for el in bibl.iter():
            if _local(el.tag) == "monogr":
                # meeting / imprint text
                monogr = _text(el)[:200]
                break
    idnos = _idnos(bibl)
    raw = _text(bibl)
    return {
        "candidate_id": f"citation:{index:04d}",
        "index": index,
        "title": title,
        "authors": authors,
        "date": date,
        "when": when,
        "venue_or_monogr": monogr,
        "idnos": idnos,
        "raw_text": raw[:500],
        "source": "grobid_tei_listBibl",
        "import_eligible": False,
        "graph_writes_allowed": False,
    }


@dataclass(frozen=True, slots=True)
class GrobidHeaderCandidate:
    paper_id: str
    title: str
    authors: tuple[dict[str, Any], ...]
    abstract: str
    idnos: dict[str, str]
    published: str
    diagnostics: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": SCHEMA_VERSION,
            "kind": "grobid_header_candidate",
            "paper_id": self.paper_id,
            "title": self.title,
            "authors": list(self.authors),
            "abstract": self.abstract,
            "idnos": dict(self.idnos),
            "published": self.published,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class GrobidTeiParseResult:
    paper_id: str
    header: GrobidHeaderCandidate
    citations: tuple[dict[str, Any], ...]
    tei_bytes: int
    parse_ok: bool
    diagnostics: tuple[str, ...] = field(default_factory=tuple)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": SCHEMA_VERSION,
            "paper_id": self.paper_id,
            "parse_ok": self.parse_ok,
            "tei_bytes": self.tei_bytes,
            "header": self.header.to_dict(),
            "citation_count": len(self.citations),
            "citations": list(self.citations),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "diagnostics": list(self.diagnostics),
        }


def parse_grobid_tei(tei: bytes | str, *, paper_id: str) -> GrobidTeiParseResult:
    """Parse GROBID TEI into header + listBibl citation candidates."""
    raw = tei if isinstance(tei, bytes) else tei.encode("utf-8")
    diagnostics: list[str] = []
    empty_header = GrobidHeaderCandidate(
        paper_id=paper_id,
        title="",
        authors=(),
        abstract="",
        idnos={},
        published="",
        diagnostics=("empty",),
    )
    try:
        root = ET.fromstring(raw)
    except ET.ParseError as exc:
        return GrobidTeiParseResult(
            paper_id=paper_id,
            header=empty_header,
            citations=(),
            tei_bytes=len(raw),
            parse_ok=False,
            diagnostics=(f"parse_error:{type(exc).__name__}",),
        )

    parents = _parent_map(root)

    # --- title from titleStmt (prefer) ---
    title = ""
    for el in root.iter():
        if _local(el.tag) != "title":
            continue
        anc = _ancestor_locals(el, parents)
        if "titleStmt" in anc:
            t = (el.text or "").strip() or _text(el)
            if t and (el.attrib.get("type") == "main" or el.attrib.get("level") == "a" or not title):
                title = t
                if el.attrib.get("type") == "main":
                    break
    if not title:
        diagnostics.append("title_missing_titleStmt")

    # --- authors from sourceDesc biblStruct (not listBibl) ---
    authors: list[dict[str, Any]] = []
    for el in root.iter():
        if _local(el.tag) != "biblStruct":
            continue
        anc = _ancestor_locals(el, parents)
        if "sourceDesc" in anc and "listBibl" not in anc:
            authors = _parse_authors_from(el)
            break
    if not authors:
        # fallback: teiHeader authors outside listBibl
        for el in root.iter():
            if _local(el.tag) != "teiHeader":
                continue
            authors = [
                a
                for a in _parse_authors_from(el)
                if True
            ]
            # filter authors that appear only under listBibl by re-walk
            break
        # better: only fileDesc
        authors = []
        for el in root.iter():
            if _local(el.tag) != "fileDesc":
                continue
            authors = _parse_authors_from(el)
            break
    if not authors:
        diagnostics.append("authors_missing")

    # --- abstract ---
    abstract = ""
    for el in root.iter():
        if _local(el.tag) == "abstract":
            abstract = _text(el)
            if abstract:
                break

    # --- idnos / published from sourceDesc bibl ---
    idnos: dict[str, str] = {}
    published = ""
    for el in root.iter():
        if _local(el.tag) != "biblStruct":
            continue
        anc = _ancestor_locals(el, parents)
        if "sourceDesc" in anc:
            idnos = _idnos(el)
            for d in el.iter():
                if _local(d.tag) == "date":
                    published = d.attrib.get("when") or (d.text or "").strip()
                    if published:
                        break
            break

    # --- citations: listBibl only ---
    citations: list[dict[str, Any]] = []
    idx = 0
    for el in root.iter():
        if _local(el.tag) != "biblStruct":
            continue
        anc = _ancestor_locals(el, parents)
        if "listBibl" not in anc:
            continue
        idx += 1
        citations.append(_parse_bibl_struct(el, index=idx))
    if not citations:
        diagnostics.append("citations_missing_listBibl")

    header = GrobidHeaderCandidate(
        paper_id=paper_id,
        title=title,
        authors=tuple(authors),
        abstract=abstract,
        idnos=idnos,
        published=published,
        diagnostics=tuple(diagnostics),
    )
    parse_ok = bool(title) or bool(citations) or bool(authors)
    return GrobidTeiParseResult(
        paper_id=paper_id,
        header=header,
        citations=tuple(citations),
        tei_bytes=len(raw),
        parse_ok=parse_ok,
        diagnostics=tuple(diagnostics)
        + (
            f"citation_count:{len(citations)}",
            f"author_count:{len(authors)}",
            f"title_present:{bool(title)}",
        ),
    )


__all__ = [
    "SCHEMA_VERSION",
    "GrobidHeaderCandidate",
    "GrobidTeiParseResult",
    "parse_grobid_tei",
]
