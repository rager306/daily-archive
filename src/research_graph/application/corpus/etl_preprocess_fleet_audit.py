"""Wave A preprocess fleet metrics on hybrid bodies (M243).

Discovers unique hybrid body markdown files across body roots (first root wins
per paper_id), runs ``preprocess_summary_for_body`` with scholarly profile,
and aggregates quality/language/keyword diagnostics.

Never network, never YAKE by default, never authorizes import.
"""

from __future__ import annotations

from collections import Counter
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.application.corpus.preprocess_summary import (
    preprocess_summary_for_body,
)

SCHEMA_VERSION = "m243-etl-preprocess-fleet.v1"


@dataclass(frozen=True, slots=True)
class HybridBodyRef:
    paper_id: str
    path: Path
    body_root: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "path": str(self.path),
            "body_root": self.body_root,
            "import_eligible": False,
        }


def discover_unique_hybrid_bodies(
    body_roots: Sequence[Path],
) -> tuple[HybridBodyRef, ...]:
    """Unique paper_id → first existing hybrid.body.md in body_roots order."""
    seen: dict[str, HybridBodyRef] = {}
    for root in body_roots:
        root_p = Path(root)
        if not root_p.is_dir():
            continue
        for path in sorted(root_p.rglob("*.hybrid.body.md")):
            if not path.is_file():
                continue
            name = path.name
            if not name.endswith(".hybrid.body.md"):
                continue
            paper_id = name[: -len(".hybrid.body.md")]
            if not paper_id or paper_id in seen:
                continue
            seen[paper_id] = HybridBodyRef(
                paper_id=paper_id,
                path=path,
                body_root=str(root_p),
            )
    return tuple(seen[k] for k in sorted(seen))


@dataclass(frozen=True, slots=True)
class EtlPreprocessFleetSample:
    paper_id: str
    quality_status: str
    language: str
    keyword_source: str
    word_count: int
    keyword_span_count: int
    path: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "quality_status": self.quality_status,
            "language": self.language,
            "keyword_source": self.keyword_source,
            "word_count": self.word_count,
            "keyword_span_count": self.keyword_span_count,
            "path": self.path,
            "import_eligible": False,
        }


@dataclass(frozen=True, slots=True)
class EtlPreprocessFleetPackage:
    schema_version: str
    body_count: int
    error_count: int
    quality_status_counts: dict[str, int]
    language_counts: dict[str, int]
    keyword_source_counts: dict[str, int]
    samples: tuple[EtlPreprocessFleetSample, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("preprocess fleet audit cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "body_count": self.body_count,
            "error_count": self.error_count,
            "quality_status_counts": dict(self.quality_status_counts),
            "language_counts": dict(self.language_counts),
            "keyword_source_counts": dict(self.keyword_source_counts),
            "samples": [s.to_dict() for s in self.samples],
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave A preprocess fleet only; scholarly profile; "
                "token_frequency keywords; not graph import"
            ),
        }


def audit_preprocess_fleet(
    *,
    body_roots: Sequence[Path],
    sample_limit: int = 12,
    profile: str = "scholarly",
) -> EtlPreprocessFleetPackage:
    """Run preprocess_summary_for_body over unique hybrid bodies (read-only)."""
    refs = discover_unique_hybrid_bodies(body_roots)
    quality: Counter[str] = Counter()
    languages: Counter[str] = Counter()
    keywords: Counter[str] = Counter()
    samples: list[EtlPreprocessFleetSample] = []
    errors = 0

    for ref in refs:
        try:
            text = ref.path.read_text(encoding="utf-8")
            summary = preprocess_summary_for_body(
                source_id=ref.paper_id,
                text=text,
                source_class="arxiv",
                profile=profile,  # type: ignore[arg-type]
                is_html=False,
            )
        except Exception:  # noqa: BLE001 - fleet metrics must not abort
            errors += 1
            quality["error"] += 1
            continue

        q = str(summary.get("quality_status") or "unknown")
        lang = str(summary.get("language") or "unknown")
        ks = str(summary.get("keyword_source") or "unknown")
        quality[q] += 1
        languages[lang] += 1
        keywords[ks] += 1

        if len(samples) < sample_limit:
            samples.append(
                EtlPreprocessFleetSample(
                    paper_id=ref.paper_id,
                    quality_status=q,
                    language=lang,
                    keyword_source=ks,
                    word_count=int(summary.get("word_count") or 0),
                    keyword_span_count=int(summary.get("keyword_span_count") or 0),
                    path=str(ref.path),
                )
            )

    diagnostics = (
        f"bodies:{len(refs)}",
        f"errors:{errors}",
        f"profile:{profile}",
        "yake:false",
        "import_write_fail_closed",
        "wave_a_preprocess_fleet_only",
    )

    return EtlPreprocessFleetPackage(
        schema_version=SCHEMA_VERSION,
        body_count=len(refs),
        error_count=errors,
        quality_status_counts=dict(sorted(quality.items())),
        language_counts=dict(sorted(languages.items())),
        keyword_source_counts=dict(sorted(keywords.items())),
        samples=tuple(samples),
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "EtlPreprocessFleetPackage",
    "EtlPreprocessFleetSample",
    "HybridBodyRef",
    "audit_preprocess_fleet",
    "discover_unique_hybrid_bodies",
]
