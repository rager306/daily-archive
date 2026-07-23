"""M223/M226: prove non-arxiv HTML sources via M207 universal_source path.

Default subject: company_blog PageIndex article (already captured).
M226: optional ArticlePreprocessPackage enrichment (language/outline/fingerprint)
on loaded HTML body — diagnostics only, never import/hybrid authorization.
Never claims hybrid TEI scholarly success. Never authorizes import.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from research_graph.application.corpus.preprocess_summary import (
    preprocess_summary_for_body,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.workflows.composition.yake_keyword_inject import (
    yake_keywords_for_text,
)
from research_graph.workflows.composition.universal_source import (
    StructuredSourceBundle,
    load_local_html_chapter,
    structure_loaded_source,
)

SCHEMA_VERSION = "m226-non-arxiv-html-source-proof.v1"
DEFAULT_BLOG_ARTICLE = Path(
    "data/article_catalog/article_catalog/company_blog/cs-ir/"
    "pageindex_zhang2025pageindex/article.json"
)


@dataclass(frozen=True, slots=True)
class NonArxivHtmlSourceProofRequest:
    article_json_path: Path = DEFAULT_BLOG_ARTICLE
    catalog_root: Path = Path("data/article_catalog")
    output_path: Path | None = None
    repo_root: Path = field(default_factory=lambda: Path("."))
    min_body_chars: int = 500
    min_chunks: int = 1
    # M230: optional YAKE keyword inject at composition boundary (default off).
    use_yake_keywords: bool = False


@dataclass(frozen=True, slots=True)
class NonArxivHtmlSourceProofResult:
    schema_version: str
    article_ref: str
    source_code: str
    article_key: str
    html_path: str
    load_outcome: str
    body_chars: int
    source_kind: str
    structure: StructuredSourceBundle | None
    proof_pass: bool
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    hybrid_claimed_success: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()
    output_path: str | None = None
    preprocess: dict[str, Any] | None = None

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("non-arxiv html proof cannot authorize import/writes")
        if self.hybrid_claimed_success:
            raise ValueError("non-arxiv html proof cannot claim hybrid TEI success")
        if self.preprocess is not None and self.preprocess.get("import_eligible") is True:
            raise ValueError("preprocess enrichment cannot authorize import")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "article_ref": self.article_ref,
            "source_code": self.source_code,
            "article_key": self.article_key,
            "html_path": self.html_path,
            "load_outcome": self.load_outcome,
            "body_chars": self.body_chars,
            "source_kind": self.source_kind,
            "structure": {
                "page_index_node_count": self.structure.page_index_node_count,
                "chunk_count": self.structure.chunk_count,
                "evidence_count": self.structure.evidence_count,
                "source_kind": self.structure.source_kind,
            }
            if self.structure is not None
            else None,
            "proof_pass": self.proof_pass,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "hybrid_claimed_success": False,
            "preprocess": self.preprocess,
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
            "output_path": self.output_path,
            "note": "HTML universal_source proof; not GROBID hybrid; not graph import",
        }


def _resolve(path: Path, repo_root: Path) -> Path:
    if path.is_file() or path.is_dir() or path.is_absolute():
        return path
    return repo_root / path


def _resolve_html_path(article_dir: Path, article: dict[str, Any]) -> Path | None:
    variants = article.get("source_variants")
    if not isinstance(variants, list):
        return None
    for v in variants:
        if not isinstance(v, dict):
            continue
        if not v.get("is_content_bearing"):
            continue
        fmt = str(v.get("source_format") or "").lower()
        if "html" not in fmt:
            continue
        rel = v.get("path") or v.get("local_path")
        if not rel:
            continue
        candidate = article_dir / str(rel)
        if candidate.is_file():
            return candidate
    return None


def run_non_arxiv_html_source_proof(
    request: NonArxivHtmlSourceProofRequest,
) -> NonArxivHtmlSourceProofResult:
    """Load+structure non-arxiv HTML article from catalog capture."""
    repo = request.repo_root
    article_path = _resolve(request.article_json_path, repo)
    if not article_path.is_file():
        raise FileNotFoundError(f"article.json missing: {article_path}")

    article = json.loads(article_path.read_text(encoding="utf-8"))
    if not isinstance(article, dict):
        raise ValueError("article.json root must be object")

    article_key = str(article.get("article_key") or "")
    source_code = str(article.get("source_code") or "")
    catalog_path = str(article.get("catalog_path") or "")
    article_ref = catalog_path or f"{source_code}/?/{article_key}"
    article_dir = article_path.parent
    html_path = _resolve_html_path(article_dir, article)
    diag: list[str] = [
        f"source_code:{source_code}",
        f"article_key:{article_key}",
        "hybrid_claimed_success:false",
        "import_write_fail_closed",
    ]

    out_path = request.output_path
    out_path_str: str | None = None
    if out_path is not None:
        out_path = _resolve(out_path, repo)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path_str = str(out_path)

    if html_path is None:
        result = NonArxivHtmlSourceProofResult(
            schema_version=SCHEMA_VERSION,
            article_ref=article_ref,
            source_code=source_code,
            article_key=article_key,
            html_path="",
            load_outcome="missing_html",
            body_chars=0,
            source_kind="html",
            structure=None,
            proof_pass=False,
            diagnostics=tuple(diag + ["missing_content_bearing_html"]),
            output_path=out_path_str,
        )
    else:
        load = load_local_html_chapter(html_path, paper_id=article_key or "non-arxiv")
        body_chars = len(load.text or "")
        structure: StructuredSourceBundle | None = None
        structure_error: str | None = None
        if load.outcome == "loaded" and load.text:
            try:
                structure = structure_loaded_source(
                    load, paper_id=article_key or "non-arxiv"
                )
            except Exception as exc:  # noqa: BLE001 - fail-closed structure
                structure_error = f"{type(exc).__name__}:{exc}"
        diag.append(f"load_outcome:{load.outcome}")
        diag.append(f"body_chars:{body_chars}")
        if structure is not None:
            diag.append(f"chunks:{structure.chunk_count}")
            diag.append(f"source_kind:{structure.source_kind}")
        if structure_error:
            diag.append(f"structure_error:{structure_error}")

        proof_pass = (
            load.outcome == "loaded"
            and body_chars >= request.min_body_chars
            and structure is not None
            and structure.chunk_count >= request.min_chunks
            and structure.source_kind == "html"
            and source_code != "arxiv"
        )
        diag.append(f"proof_pass:{proof_pass}")

        preprocess_summary: dict[str, Any] | None = None
        if load.outcome == "loaded" and load.text:
            injected: list[str] | None = None
            if request.use_yake_keywords:
                injected = yake_keywords_for_text(
                    load.text, language="en", top_k=12
                )
            preprocess_summary = preprocess_summary_for_body(
                source_id=article_key or "non-arxiv",
                text=load.text,
                source_class=source_code or "company_blog",
                profile="web",
                is_html=True,
                keywords=injected,
            )
            fp = str(preprocess_summary.get("content_fingerprint_sha256") or "")
            diag.append(f"preprocess_language:{preprocess_summary.get('language')}")
            diag.append(f"content_fingerprint:{fp[:12]}")
            diag.append(
                f"keyword_source:{preprocess_summary.get('keyword_source')}"
            )
            diag.append(f"use_yake_keywords:{request.use_yake_keywords}")

        result = NonArxivHtmlSourceProofResult(
            schema_version=SCHEMA_VERSION,
            article_ref=article_ref,
            source_code=source_code,
            article_key=article_key,
            html_path=str(html_path),
            load_outcome=str(load.outcome),
            body_chars=body_chars,
            source_kind="html",
            structure=structure,
            proof_pass=proof_pass,
            diagnostics=tuple(diag),
            output_path=out_path_str,
            preprocess=preprocess_summary,
        )

    if out_path is not None:
        out_path.write_text(
            json.dumps(result.to_dict(), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    return result


__all__ = [
    "DEFAULT_BLOG_ARTICLE",
    "SCHEMA_VERSION",
    "NonArxivHtmlSourceProofRequest",
    "NonArxivHtmlSourceProofResult",
    "run_non_arxiv_html_source_proof",
]
