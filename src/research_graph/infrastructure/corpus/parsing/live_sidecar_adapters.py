"""Live GROBID HTTP + OpenDataLoader library adapters for hybrid runtime.

Infrastructure adapters implementing the M212 sidecar ports. Callers should
probe/ensure services first (sidecar_services). Fail-closed metrics dicts —
never raise into application for ordinary service absence.
"""

from __future__ import annotations

import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any
from xml.etree import ElementTree as ET

from research_graph.infrastructure.corpus.parsing.sidecar_services import (
    USER_AGENT,
    ensure_grobid,
    ensure_opendataloader,
    grobid_base_url,
)


def _as_metrics_error(status: str, **extra: Any) -> dict[str, Any]:
    payload = {"status": status, "error": status}
    payload.update(extra)
    return payload


class LiveGrobidSidecarAdapter:
    """GROBID HTTP adapter: processFulltextDocument → metrics (+ optional TEI size)."""

    def __init__(
        self,
        *,
        base_url: str | None = None,
        timeout_s: float = 120.0,
        ensure_service: bool = True,
        artifact_dir: Path | None = None,
    ) -> None:
        self.base_url = (base_url or grobid_base_url()).rstrip("/")
        self.timeout_s = timeout_s
        self.ensure_service = ensure_service
        self.artifact_dir = Path(artifact_dir) if artifact_dir is not None else None

    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict[str, Any]:
        if self.ensure_service:
            probe = ensure_grobid(url=self.base_url)
            if not probe.available:
                return _as_metrics_error(
                    "unavailable",
                    detail=probe.detail,
                    diagnostics=list(probe.diagnostics),
                    paper_id=paper_id,
                )
        if not pdf_path.is_file():
            return _as_metrics_error("missing_pdf", path=str(pdf_path), paper_id=paper_id)

        endpoint = f"{self.base_url}/api/processFulltextDocument"
        boundary = f"----daily-archive-grobid-{int(time.time() * 1000)}"
        pdf_bytes = pdf_path.read_bytes()
        body = b"".join(
            [
                f"--{boundary}\r\n".encode(),
                b'Content-Disposition: form-data; name="consolidateHeader"\r\n\r\n0\r\n',
                f"--{boundary}\r\n".encode(),
                b'Content-Disposition: form-data; name="consolidateCitations"\r\n\r\n0\r\n',
                f"--{boundary}\r\n".encode(),
                (
                    f'Content-Disposition: form-data; name="input"; filename="{pdf_path.name}"\r\n'
                    "Content-Type: application/pdf\r\n\r\n"
                ).encode(),
                pdf_bytes + b"\r\n",
                f"--{boundary}--\r\n".encode(),
            ]
        )
        req = urllib.request.Request(
            endpoint,
            data=body,
            method="POST",
            headers={
                "User-Agent": USER_AGENT,
                "Content-Type": f"multipart/form-data; boundary={boundary}",
            },
        )
        started = time.perf_counter()
        try:
            with urllib.request.urlopen(req, timeout=self.timeout_s) as resp:
                tei = resp.read()
                status_code = int(getattr(resp, "status", 0) or 0)
        except (urllib.error.URLError, TimeoutError, OSError) as exc:
            return _as_metrics_error(
                "network_error",
                detail=f"{type(exc).__name__}:{exc}",
                paper_id=paper_id,
                duration_ms=int((time.perf_counter() - started) * 1000),
            )

        duration_ms = int((time.perf_counter() - started) * 1000)
        if status_code != 200 or not tei.strip():
            return _as_metrics_error(
                "failed",
                http_status=status_code,
                paper_id=paper_id,
                duration_ms=duration_ms,
            )

        title_present, author_count, bibl_count, body_count, ref_count = _tei_counts(tei)
        # Structured ETL (M217): header + listBibl citations as candidate payloads.
        # Still not graph-importable; raw TEI is not persisted here.
        from research_graph.application.corpus.grobid_tei_parse import parse_grobid_tei

        parsed = parse_grobid_tei(tei, paper_id=paper_id)
        tei_sha256 = None
        tei_path_str = None
        if self.artifact_dir is not None:
            from research_graph.application.corpus.parser_run_artifacts import (
                build_parser_run_manifest,
                write_bytes_artifact,
                write_parser_run_manifest,
            )

            art = Path(self.artifact_dir)
            tei_path = art / f"{paper_id}.grobid.tei.xml"
            tei_sha256 = write_bytes_artifact(tei_path, tei)
            tei_path_str = str(tei_path)
            man = build_parser_run_manifest(
                paper_id=paper_id,
                parser="grobid",
                parser_version=None,
                config={
                    "endpoint": "processFulltextDocument",
                    "base_url": self.base_url,
                    "consolidateHeader": "0",
                    "consolidateCitations": "0",
                },
                artifact_paths={"tei": tei_path_str},
                content_hashes={"tei": tei_sha256},
            )
            write_parser_run_manifest(art / f"{paper_id}.grobid.parser-run.json", man)
        else:
            from research_graph.application.corpus.parser_run_artifacts import sha256_bytes

            tei_sha256 = sha256_bytes(tei)

        return {
            "status": "success",
            "paper_id": paper_id,
            "arxiv_id": paper_id,
            "header_title_present": title_present or bool(parsed.header.title),
            "header_author_count": author_count or len(parsed.header.authors),
            "bibl_count": bibl_count if bibl_count else len(parsed.citations),
            "body_element_count": body_count,
            "ref_count": ref_count,
            "bytes": len(tei),
            "duration_ms": duration_ms,
            "endpoint": endpoint,
            "grobid_url": self.base_url,
            # TEI not graph body; optional raw_tei for caller persist when no artifact_dir.
            "tei_present": True,
            "tei_sha256": tei_sha256,
            "tei_path": tei_path_str,
            "tei_bytes": tei if self.artifact_dir is None else None,
            "header": parsed.header.to_dict(),
            "citations": list(parsed.citations),
            "citation_count": len(parsed.citations),
            "structured_parse_ok": parsed.parse_ok,
            "structured_diagnostics": list(parsed.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


def _tei_counts(tei: bytes) -> tuple[bool, int, int, int, int]:
    try:
        root = ET.fromstring(tei)
    except ET.ParseError:
        return False, 0, 0, 0, 0

    def count(local: str) -> int:
        suffix = "}" + local
        return sum(1 for el in root.iter() if el.tag == local or el.tag.endswith(suffix))

    titles = [
        el
        for el in root.iter()
        if el.tag == "title" or el.tag.endswith("}title")
    ]
    title_present = any((el.text or "").strip() for el in titles)
    return (
        title_present,
        count("author"),
        count("biblStruct"),
        count("div"),
        count("ref"),
    )


class LiveOpenDataLoaderSidecarAdapter:
    """In-process OpenDataLoader PDF adapter → markdown + layout metrics."""

    def __init__(self, *, ensure_import: bool = True, output_format: str = "markdown") -> None:
        self.ensure_import = ensure_import
        self.output_format = output_format

    def extract_metrics(self, pdf_path: Path, *, paper_id: str) -> dict[str, Any]:
        if self.ensure_import:
            probe = ensure_opendataloader()
            if not probe.available:
                return _as_metrics_error(
                    "unavailable",
                    detail=probe.detail,
                    diagnostics=list(probe.diagnostics),
                    paper_id=paper_id,
                )
        if not pdf_path.is_file():
            return _as_metrics_error("missing_pdf", path=str(pdf_path), paper_id=paper_id)

        started = time.perf_counter()
        try:
            import opendataloader_pdf  # type: ignore[import-not-found]
        except Exception as exc:  # noqa: BLE001 - fail-closed metrics
            return _as_metrics_error(
                "import_error",
                detail=f"{type(exc).__name__}:{exc}",
                paper_id=paper_id,
            )

        try:
            # Prefer JSON+markdown so layout/bbox is retained (M274 evidence foundation).
            converted = _odl_convert_json_and_markdown(opendataloader_pdf, pdf_path)
        except Exception as exc:  # noqa: BLE001
            return _as_metrics_error(
                "failed",
                detail=f"{type(exc).__name__}:{exc}",
                paper_id=paper_id,
                duration_ms=int((time.perf_counter() - started) * 1000),
            )

        duration_ms = int((time.perf_counter() - started) * 1000)
        text = str(converted.get("markdown") or "")
        layout = converted.get("layout_json")
        from research_graph.application.corpus.parser_run_artifacts import (
            count_layout_elements,
            sha256_text,
        )

        element_count, bbox_count = count_layout_elements(layout)
        if bbox_count == 0 and text:
            # Fallback proxy only when layout JSON absent/empty.
            bbox_count = text.count("\n")
            bbox_source = "newline_proxy"
        else:
            bbox_source = "layout_json" if bbox_count else "none"

        layout_sha = None
        if layout is not None:
            import json as _json

            try:
                layout_sha = sha256_text(
                    _json.dumps(layout, sort_keys=True, ensure_ascii=False)
                )
            except (TypeError, ValueError):
                layout_sha = None

        if not text.strip():
            return {
                "status": "success",
                "low_quality_source": True,
                "markdown": "",
                "markdown_size_bytes": 0,
                "bounding_box_count": bbox_count,
                "layout_element_count": element_count,
                "bbox_source": bbox_source,
                "layout_json": layout,
                "layout_json_sha256": layout_sha,
                "paper_id": paper_id,
                "duration_ms": duration_ms,
                "format": "json+markdown",
                "import_eligible": False,
                "graph_writes_allowed": False,
            }
        return {
            "status": "success",
            "low_quality_source": len(text) < 500,
            "markdown": text,
            "markdown_size_bytes": len(text.encode("utf-8")),
            "bounding_box_count": bbox_count,
            "layout_element_count": element_count,
            "bbox_source": bbox_source,
            "layout_json": layout,
            "layout_json_sha256": layout_sha,
            "paper_id": paper_id,
            "duration_ms": duration_ms,
            "format": "json+markdown",
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


def _odl_convert_json_and_markdown(mod: Any, pdf_path: Path) -> dict[str, Any]:
    """Best-effort ODL convert producing markdown + optional layout JSON.

    v2.x `convert(...)` writes artifacts to `output_dir` and returns None.
    Requests format=["json", "markdown"] when supported; falls back to markdown-only.
    """
    import json
    import tempfile

    if hasattr(mod, "convert_to_markdown") and not hasattr(mod, "convert"):
        out = mod.convert_to_markdown(str(pdf_path))
        return {"markdown": out if isinstance(out, str) else "", "layout_json": None}

    if not hasattr(mod, "convert"):
        if hasattr(mod, "convert_to_markdown"):
            out = mod.convert_to_markdown(str(pdf_path))
            return {
                "markdown": out if isinstance(out, str) else "",
                "layout_json": None,
            }
        raise RuntimeError("opendataloader_pdf has no convert API")

    with tempfile.TemporaryDirectory(prefix="odl-live-") as tmp:
        out_dir = Path(tmp)
        formats_try = (["json", "markdown"], ["markdown"])
        last_err: Exception | None = None
        for formats in formats_try:
            try:
                mod.convert(
                    str(pdf_path),
                    output_dir=str(out_dir),
                    format=list(formats),
                    quiet=True,
                )
                last_err = None
                break
            except Exception as exc:  # noqa: BLE001
                last_err = exc
                # clear partial outputs
                for p in out_dir.rglob("*"):
                    if p.is_file():
                        try:
                            p.unlink()
                        except OSError:
                            pass
        if last_err is not None and not any(out_dir.rglob("*")):
            raise last_err

        md_files = sorted(out_dir.rglob("*.md"))
        if not md_files:
            md_files = sorted(out_dir.rglob("*.txt"))
        json_files = sorted(out_dir.rglob("*.json"))

        markdown = ""
        if md_files:
            preferred = [p for p in md_files if p.stem.startswith(pdf_path.stem)]
            chosen = preferred[0] if preferred else md_files[0]
            markdown = chosen.read_text(encoding="utf-8", errors="replace")
        elif not json_files:
            raise RuntimeError(f"odl_no_markdown_or_json_output dir={out_dir}")

        layout_json: Any = None
        if json_files:
            preferred_j = [p for p in json_files if p.stem.startswith(pdf_path.stem)]
            jpath = preferred_j[0] if preferred_j else json_files[0]
            try:
                layout_json = json.loads(
                    jpath.read_text(encoding="utf-8", errors="replace")
                )
            except json.JSONDecodeError:
                layout_json = None

        return {"markdown": markdown, "layout_json": layout_json}


def _odl_convert_to_markdown(mod: Any, pdf_path: Path) -> str:
    """Backward-compatible markdown-only helper."""
    return str(_odl_convert_json_and_markdown(mod, pdf_path).get("markdown") or "")


def build_live_hybrid_ports(
    *,
    ensure: bool = True,
) -> tuple[LiveGrobidSidecarAdapter | None, LiveOpenDataLoaderSidecarAdapter | None, dict[str, Any]]:
    """Construct live ports after probe/ensure; missing services → None."""
    from research_graph.infrastructure.corpus.parsing.sidecar_services import (
        probe_grobid,
        probe_opendataloader,
    )

    g_probe = ensure_grobid() if ensure else probe_grobid()
    o_probe = ensure_opendataloader() if ensure else probe_opendataloader()
    diagnostics = {
        "grobid": g_probe.to_dict(),
        "opendataloader": o_probe.to_dict(),
    }
    if ensure:
        # Adapters re-probe on extract; always return objects for live path.
        return (
            LiveGrobidSidecarAdapter(ensure_service=True),
            LiveOpenDataLoaderSidecarAdapter(ensure_import=True),
            diagnostics,
        )
    return (
        LiveGrobidSidecarAdapter(ensure_service=False) if g_probe.available else None,
        LiveOpenDataLoaderSidecarAdapter(ensure_import=False) if o_probe.available else None,
        diagnostics,
    )


__all__ = [
    "LiveGrobidSidecarAdapter",
    "LiveOpenDataLoaderSidecarAdapter",
    "build_live_hybrid_ports",
]
