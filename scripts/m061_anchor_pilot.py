#!/usr/bin/env python3
"""M061 S01 1-anchor 2-hop BFS pilot for anchor 2605.18747.

The pilot is synchronous by design (ADR-017). It is diagnostic-only: graph writes
are not authorized, production import is not authorized, fact promotion is not
authorized, external network is disabled by default, and LLM calls are disabled
by default. Stage 7 records a scoped diagnostic-only M3 override by reusing the
M060g evidence bundle rather than making new live model calls.
"""

from __future__ import annotations

import argparse
import datetime as dt
import gzip
import hashlib
import io
import json
import re
import tarfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parents[1]
ANCHOR_ARXIV_ID = "2605.18747"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m061-2hop" / "anchor-2605.18747"
M056_ROOT = ROOT / "artifacts" / "m056-bfs-graph"
M057_ROOT = ROOT / "artifacts" / "m057-fd-marker"
M058_ROOT = ROOT / "artifacts" / "m058-plotextractor"
M060G_ROOT = ROOT / "artifacts" / "m060g-judge"
GENERATED_BY = "scripts/m061_anchor_pilot.py"
NETWORK_HOST = "127.0.0.1"
GROBID_URL = f"http://{NETWORK_HOST}:8070/api/processFulltextDocument"
FD_URL = f"http://{NETWORK_HOST}:8000"
MANIFEST_SCHEMA_PATH = ROOT / "schemas" / "daily-archive.pdf-batch-manifest.v1.json"
GROBID_SCHEMA_PATH = ROOT / "schemas" / "grobid-tei.v1.json"
OPENDATALOADER_SCHEMA_PATH = ROOT / "schemas" / "opendataloader-pdf.v1.json"
PLOTEXTRACTOR_SCHEMA_PATH = ROOT / "schemas" / "m058-plotextractor-figure-caption.v1.json"
TABLE_SCHEMA_PATH = ROOT / "schemas" / "m057-fd-table-similarity.v1.json"

SAFETY_DEFAULTS: dict[str, bool] = {
    "external_network_authorized": False,
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "llm_calls_authorized": False,
}

SAFETY_OVERRIDE: dict[str, Any] = {
    "external_network_authorized": True,
    "reason": (
        "User explicit authorization for M064-wqfgfa S01 real-pipeline pilot; "
        "arxiv rate limit respected (1 req/3s, retry+backoff, 429 honors Retry-After)"
    ),
    "scope": "M064-wqfgfa S01 only, 30 sample PDFs, no production import, no graph writes",
}

ARXIV_USER_AGENT = "daily-archive/1.0 (mailto: contact@example.com)"
ARXIV_API_MIN_INTERVAL_SECONDS = 3.0
ARXIV_MAX_RETRY_ATTEMPTS = 3
ARXIV_BACKOFF_SECONDS = (1.0, 5.0, 15.0, 60.0, 300.0)
ARXIV_API_BATCH_SIZE = 10
ARXIV_API_URL = "https://export.arxiv.org/api/query"
ARXIV_PDF_URL_TEMPLATE = "https://arxiv.org/pdf/{arxiv_id}.pdf"
ARXIV_EPRINT_URL_TEMPLATE = "https://arxiv.org/e-print/{arxiv_id}"
ARXIV_ID_PLAUSIBLE_RE = re.compile(r"^(\d{2})(\d{2})\.\d{4,5}$")

DIAGNOSTIC_M3_OVERRIDE: dict[str, Any] = {
    "llm_calls_authorized": True,
    "scope": "M061 S01 M3 diagnostic-only evidence reuse from M060g",
    "reason": (
        "Live LLM calls are disabled by default; M3 diagnostic scores are reused "
        "from artifacts/m060g-judge. Graph writes is not authorized, production "
        "import is not authorized, and fact promotion is not authorized."
    ),
    "model": "MiniMax-M3",
    "binding_id": "figure-qa-judge-quality",
}

PARSER_EXPECTATIONS: list[dict[str, str]] = [
    {"name": "grobid-fulltext", "version": "existing-m056-or-skip", "mode": "sync", "expected_output_schema": "schemas/grobid-tei.v1.json"},
    {"name": "opendataloader", "version": "diagnostic-wrapper", "mode": "sync", "expected_output_schema": "schemas/opendataloader-pdf.v1.json"},
    {"name": "plotextractor", "version": "existing-m058-or-skip", "mode": "sync", "expected_output_schema": "schemas/m058-plotextractor-figure-caption.v1.json"},
]

ARXIV_ID_RE = re.compile(r"(?i)(?:arxiv\s*:\s*)?(\d{4}\.\d{4,5})(?:v\d+)?")
TEI_NS = {"tei": "http://www.tei-c.org/ns/1.0"}
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()


@dataclass(frozen=True)
class PipelinePaths:
    output_dir: Path
    acquisition_dir: Path
    parsing_dir: Path
    judgments_dir: Path
    graph_dir: Path
    paper_manifest_dir: Path
    parser_output_dir: Path


def utc_now() -> str:
    return dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat()


def read_json(path: Path) -> Any:
    return json.loads(path.read_text())


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def ensure_dirs(output_dir: Path) -> PipelinePaths:
    paths = PipelinePaths(
        output_dir=output_dir,
        acquisition_dir=output_dir / "acquisition",
        parsing_dir=output_dir / "parsing",
        judgments_dir=output_dir / "judgments",
        graph_dir=output_dir / "graph",
        paper_manifest_dir=output_dir / "parsing" / "paper-manifests",
        parser_output_dir=output_dir / "parsing" / "parser-outputs",
    )
    for directory in (
        paths.acquisition_dir,
        paths.parsing_dir,
        paths.judgments_dir,
        paths.graph_dir,
        paths.paper_manifest_dir,
        paths.parser_output_dir,
    ):
        directory.mkdir(parents=True, exist_ok=True)
    return paths


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def normalize_arxiv_id(value: str | None) -> str | None:
    if not value:
        return None
    match = ARXIV_ID_RE.search(value)
    return match.group(1) if match else None


def index_tei_files(root: Path = M056_ROOT) -> dict[str, Path]:
    index: dict[str, Path] = {}
    for path in root.rglob("*.tei.xml"):
        arxiv_id = path.name.removesuffix(".tei.xml")
        if normalize_arxiv_id(arxiv_id):
            index[arxiv_id] = path
    return index


def index_grobid_json(root: Path = M056_ROOT) -> dict[str, Path]:
    index: dict[str, Path] = {}
    for path in root.rglob("grobid-fulltext/per-pdf/*.json"):
        arxiv_id = normalize_arxiv_id(path.stem)
        if arxiv_id:
            index[arxiv_id] = path
    for path in root.rglob("anchor-grobid/per-pdf/*.json"):
        arxiv_id = normalize_arxiv_id(path.stem)
        if arxiv_id:
            index[arxiv_id] = path
    return index


def extract_arxiv_refs_from_tei(tei_path: Path, source_arxiv_id: str) -> list[str]:
    root = ET.parse(tei_path).getroot()
    refs: set[str] = set()
    for bibl in root.findall(".//tei:biblStruct", TEI_NS):
        texts: list[str] = []
        for element in bibl.iter():
            if element.text:
                texts.append(element.text)
        for text in texts:
            for match in ARXIV_ID_RE.finditer(text):
                candidate = match.group(1)
                if candidate != source_arxiv_id:
                    refs.add(candidate)
    return sorted(refs)


def load_one_hop_refs(cumulative_corpus: dict[str, Any], anchor_arxiv_id: str) -> list[str]:
    pdfs = cumulative_corpus.get("pdfs", [])
    refs = sorted({item["arxiv_id"] for item in pdfs if item.get("arxiv_id") != anchor_arxiv_id})
    expected = cumulative_corpus.get("unique_1hop_pdf_count")
    if expected is not None and expected != len(refs):
        raise RuntimeError(f"M056 1-hop ref count mismatch: expected {expected}, got {len(refs)}")
    return refs


def stage_1_anchor_acquisition(cumulative_corpus: dict[str, Any], anchor_arxiv_id: str) -> dict[str, Any]:
    anchor_pdf = next((item for item in cumulative_corpus.get("pdfs", []) if item.get("arxiv_id") == anchor_arxiv_id), None)
    verified = bool(anchor_pdf and (ROOT / anchor_pdf.get("path", "")).exists())
    return {
        "stage": 1,
        "name": "anchor_acquisition",
        "status": "complete" if verified else "failed",
        "anchor_arxiv_id": anchor_arxiv_id,
        "anchor_pdf_in_m056_corpus": bool(anchor_pdf),
        "anchor_pdf_path": anchor_pdf.get("path") if anchor_pdf else None,
        "anchor_pdf_exists": verified,
        "external_network_authorized_default": SAFETY_DEFAULTS["external_network_authorized"],
        "external_network_override": SAFETY_OVERRIDE,
        "note": "Anchor PDF was reused from M056 corpus; S01 v2 additionally authorizes scoped real arXiv acquisition.",
    }


def stage_2_one_hop_validation(
    cumulative_corpus: dict[str, Any], candidate_edges: dict[str, Any], one_hop_refs: list[str], anchor_arxiv_id: str
) -> dict[str, Any]:
    edge_neighbors = {
        edge["paper_b"]
        for edge in candidate_edges.get("edges", [])
        if edge.get("paper_a") == anchor_arxiv_id and normalize_arxiv_id(edge.get("paper_b"))
    }
    corpus_refs = set(one_hop_refs)
    return {
        "stage": 2,
        "name": "one_hop_validation",
        "status": "complete" if len(one_hop_refs) == cumulative_corpus.get("unique_1hop_pdf_count") else "failed",
        "anchor_arxiv_id": anchor_arxiv_id,
        "m056_unique_1hop_pdf_count": cumulative_corpus.get("unique_1hop_pdf_count"),
        "validated_1hop_count": len(one_hop_refs),
        "candidate_edge_direct_neighbor_count": len(edge_neighbors),
        "candidate_edges_match_corpus_subset": edge_neighbors.issubset(corpus_refs),
        "extra_candidate_edge_neighbors": sorted(edge_neighbors - corpus_refs)[:25],
        "safety_defaults": SAFETY_DEFAULTS,
    }


def stage_3_two_hop_bfs(
    one_hop_refs: list[str], tei_index: dict[str, Path], anchor_arxiv_id: str
) -> tuple[dict[str, Any], list[dict[str, Any]], list[str]]:
    one_hop_set = set(one_hop_refs)
    edges: list[dict[str, Any]] = []
    per_ref: list[dict[str, Any]] = []
    all_targets: set[str] = set()
    for source in one_hop_refs:
        tei_path = tei_index.get(source)
        refs = extract_arxiv_refs_from_tei(tei_path, source) if tei_path else []
        all_targets.update(refs)
        edges.extend(
            {
                "paper_a": source,
                "paper_b": target,
                "edge_type": "cites",
                "evidence": "grobid_tei_biblstruct",
                "source_tei": str(tei_path.relative_to(ROOT)) if tei_path else None,
            }
            for target in refs
        )
        per_ref.append(
            {
                "arxiv_id": source,
                "tei_available": tei_path is not None,
                "ref_count": len(refs),
                "new_2hop_ref_count": len(set(refs) - one_hop_set - {anchor_arxiv_id}),
            }
        )
    new_2hop_ids = sorted(all_targets - one_hop_set - {anchor_arxiv_id})
    report = {
        "stage": 3,
        "name": "two_hop_bfs_algorithm",
        "status": "complete",
        "anchor_arxiv_id": anchor_arxiv_id,
        "one_hop_input_count": len(one_hop_refs),
        "one_hop_with_tei_count": sum(1 for row in per_ref if row["tei_available"]),
        "candidate_2hop_edge_count": len(edges),
        "unique_2hop_target_count": len(all_targets),
        "new_2hop_arxiv_id_count": len(new_2hop_ids),
        "new_2hop_arxiv_ids_sample": new_2hop_ids[:50],
        "per_ref": per_ref,
        "safety_defaults": SAFETY_DEFAULTS,
    }
    return report, edges, new_2hop_ids


def build_manifest_item(arxiv_id: str, pdf_path: Path | None) -> dict[str, Any]:
    if pdf_path and pdf_path.exists():
        rel_path = str(pdf_path.relative_to(ROOT)) if pdf_path.is_absolute() and pdf_path.is_relative_to(ROOT) else str(pdf_path)
        size_bytes = pdf_path.stat().st_size
        content_sha256 = sha256_file(pdf_path)
        storage_provider = "local"
    else:
        rel_path = f"not-acquired/{arxiv_id}.pdf"
        size_bytes = 0
        content_sha256 = EMPTY_SHA256
        storage_provider = "unknown"
    return {
        "arxiv_id": arxiv_id,
        "source_uri": f"https://arxiv.org/pdf/{arxiv_id}",
        "storage_provider": storage_provider,
        "path": rel_path,
        "size_bytes": size_bytes,
        "content_sha256": content_sha256,
        "expected_parsers": PARSER_EXPECTATIONS,
    }


def find_existing_pdf_path(grobid_json_index: dict[str, Path], arxiv_id: str) -> Path | None:
    json_path = grobid_json_index.get(arxiv_id)
    if not json_path:
        return None
    payload = read_json(json_path)
    pdf_path = payload.get("pdf_path")
    if not pdf_path:
        return None
    candidate = ROOT / pdf_path
    return candidate if candidate.exists() else None


class ArxivRateLimitedClient:
    """Tiny stdlib arXiv client with explicit unauthenticated API pacing."""

    def __init__(self, min_interval_seconds: float = ARXIV_API_MIN_INTERVAL_SECONDS) -> None:
        self.min_interval_seconds = min_interval_seconds
        self._last_request_started: float | None = None
        self.metrics: dict[str, Any] = {
            "user_agent": ARXIV_USER_AGENT,
            "min_interval_seconds": min_interval_seconds,
            "max_retry_attempts_per_request": ARXIV_MAX_RETRY_ATTEMPTS,
            "backoff_schedule_seconds": list(ARXIV_BACKOFF_SECONDS),
            "requests_made": 0,
            "http_429_count": 0,
            "retry_attempts": 0,
            "retry_after_honored_count": 0,
            "retry_after_delay_seconds_total": 0.0,
            "backoff_delay_seconds_total": 0.0,
            "pacing_delay_count": 0,
            "pacing_delay_seconds_total": 0.0,
            "request_kinds": {},
        }

    def _pace(self) -> None:
        now = time.monotonic()
        if self._last_request_started is not None:
            wait_seconds = self.min_interval_seconds - (now - self._last_request_started)
            if wait_seconds > 0:
                self.metrics["pacing_delay_count"] += 1
                self.metrics["pacing_delay_seconds_total"] += wait_seconds
                time.sleep(wait_seconds)
        self._last_request_started = time.monotonic()

    def _request(self, url: str, *, kind: str, timeout: int = 60) -> bytes:
        headers = {"User-Agent": ARXIV_USER_AGENT}
        last_error = "unknown"
        for attempt in range(ARXIV_MAX_RETRY_ATTEMPTS + 1):
            if attempt:
                self.metrics["retry_attempts"] += 1
            self._pace()
            request = urllib.request.Request(url, headers=headers, method="GET")
            self.metrics["requests_made"] += 1
            self.metrics["request_kinds"][kind] = self.metrics["request_kinds"].get(kind, 0) + 1
            try:
                with urllib.request.urlopen(request, timeout=timeout) as response:  # noqa: S310 - explicit arXiv acquisition override
                    return response.read()
            except urllib.error.HTTPError as exc:
                last_error = f"HTTPError:{exc.code}"
                if exc.code == 429:
                    self.metrics["http_429_count"] += 1
                    retry_after = _parse_retry_after_seconds(exc.headers.get("Retry-After"))
                    if retry_after is not None:
                        self.metrics["retry_after_honored_count"] += 1
                        self.metrics["retry_after_delay_seconds_total"] += retry_after
                        time.sleep(retry_after)
                        continue
                if attempt >= ARXIV_MAX_RETRY_ATTEMPTS or exc.code < 500:
                    raise RuntimeError(last_error) from exc
                delay = ARXIV_BACKOFF_SECONDS[min(attempt, len(ARXIV_BACKOFF_SECONDS) - 1)]
                self.metrics["backoff_delay_seconds_total"] += delay
                time.sleep(delay)
            except (urllib.error.URLError, TimeoutError, OSError) as exc:
                last_error = f"{type(exc).__name__}:{exc}"
                if attempt >= ARXIV_MAX_RETRY_ATTEMPTS:
                    raise RuntimeError(last_error) from exc
                delay = ARXIV_BACKOFF_SECONDS[min(attempt, len(ARXIV_BACKOFF_SECONDS) - 1)]
                self.metrics["backoff_delay_seconds_total"] += delay
                time.sleep(delay)
        raise RuntimeError(last_error)

    def get_api(self, url: str) -> bytes:
        return self._request(url, kind="api", timeout=60)

    def get_pdf(self, url: str) -> bytes:
        return self._request(url, kind="pdf", timeout=120)

    def get_eprint(self, url: str) -> bytes:
        return self._request(url, kind="eprint", timeout=120)

    def finalized_metrics(self) -> dict[str, Any]:
        metrics = dict(self.metrics)
        requests = metrics["requests_made"]
        metrics["http_429_rate"] = metrics["http_429_count"] / requests if requests else 0.0
        metrics["average_pacing_delay_seconds"] = (
            metrics["pacing_delay_seconds_total"] / metrics["pacing_delay_count"] if metrics["pacing_delay_count"] else 0.0
        )
        return metrics


def _parse_retry_after_seconds(value: str | None) -> float | None:
    if not value:
        return None
    try:
        return max(0.0, float(value.strip()))
    except ValueError:
        return None


def plausible_arxiv_id(arxiv_id: str) -> bool:
    match = ARXIV_ID_PLAUSIBLE_RE.match(arxiv_id)
    if not match:
        return False
    month = int(match.group(2))
    return 1 <= month <= 12


def chunked(items: list[str], size: int) -> Iterable[list[str]]:
    for index in range(0, len(items), size):
        yield items[index : index + size]


def fetch_arxiv_metadata(client: ArxivRateLimitedClient, arxiv_ids: list[str]) -> dict[str, dict[str, str]]:
    query = urllib.parse.urlencode({"id_list": ",".join(arxiv_ids), "max_results": str(len(arxiv_ids))})
    payload = client.get_api(f"{ARXIV_API_URL}?{query}")
    root = ET.fromstring(payload)
    atom = {"atom": "http://www.w3.org/2005/Atom"}
    metadata: dict[str, dict[str, str]] = {}
    for entry in root.findall("atom:entry", atom):
        id_text = entry.findtext("atom:id", default="", namespaces=atom)
        arxiv_id = normalize_arxiv_id(id_text.rsplit("/", 1)[-1])
        if not arxiv_id:
            continue
        title = " ".join((entry.findtext("atom:title", default="", namespaces=atom) or "").split())
        metadata[arxiv_id] = {"title": title, "api_id": id_text}
    return metadata


def download_arxiv_pdf(client: ArxivRateLimitedClient, arxiv_id: str, pdf_dir: Path) -> tuple[Path | None, str | None]:
    pdf_dir.mkdir(parents=True, exist_ok=True)
    pdf_path = pdf_dir / f"{arxiv_id}.pdf"
    try:
        pdf_bytes = client.get_pdf(ARXIV_PDF_URL_TEMPLATE.format(arxiv_id=arxiv_id))
    except RuntimeError as exc:
        return None, str(exc)
    if not pdf_bytes.startswith(b"%PDF"):
        return None, "downloaded content is not a PDF"
    pdf_path.write_bytes(pdf_bytes)
    return pdf_path, None


def download_arxiv_eprint(client: ArxivRateLimitedClient, arxiv_id: str, source_dir: Path) -> tuple[Path | None, str | None]:
    source_dir.mkdir(parents=True, exist_ok=True)
    source_path = source_dir / f"{arxiv_id}.eprint"
    try:
        source_bytes = client.get_eprint(ARXIV_EPRINT_URL_TEMPLATE.format(arxiv_id=arxiv_id))
    except RuntimeError as exc:
        return None, str(exc)
    if not source_bytes:
        return None, "downloaded e-print source is empty"
    source_path.write_bytes(source_bytes)
    return source_path, None


def _decode_latex_bytes(payload: bytes) -> str:
    for decoder in (
        lambda data: gzip.decompress(data),
        lambda data: data,
    ):
        try:
            return decoder(payload).decode("utf-8", errors="replace")
        except (OSError, EOFError, UnicodeDecodeError):
            continue
    return payload.decode("utf-8", errors="replace")


def extract_latex_text(source_path: Path) -> str:
    payload = source_path.read_bytes()
    try:
        chunks: list[str] = []
        with tarfile.open(fileobj=io.BytesIO(payload), mode="r:*") as archive:
            for member in archive.getmembers():
                if member.isfile() and member.name.lower().endswith(".tex"):
                    extracted = archive.extractfile(member)
                    if extracted:
                        chunks.append(extracted.read().decode("utf-8", errors="replace"))
        if chunks:
            return "\n".join(chunks)
    except tarfile.TarError:
        pass
    return _decode_latex_bytes(payload)


def _clean_latex_caption(caption: str) -> str:
    cleaned = re.sub(r"\\[a-zA-Z]+\*?(?:\[[^\]]*\])?", " ", caption)
    cleaned = cleaned.replace("{", " ").replace("}", " ")
    return " ".join(cleaned.split())


def extract_tex_figure_captions(arxiv_id: str, source_path: Path | None) -> dict[str, Any]:
    if not source_path or not source_path.exists():
        return {
            "tex_status": "missing_eprint_source",
            "figures": [],
            "caption_count": 0,
            "figure_count": 0,
            "source_path": None,
        }
    latex_text = extract_latex_text(source_path)
    figure_blocks = re.findall(r"\\begin\{figure\*?\}(.*?)\\end\{figure\*?\}", latex_text, flags=re.DOTALL)
    if not figure_blocks:
        figure_blocks = re.findall(r"\\caption(?:\[[^\]]*\])?\{(.{1,2000}?)\}", latex_text, flags=re.DOTALL)
    figures: list[dict[str, Any]] = []
    for idx, block in enumerate(figure_blocks, start=1):
        caption_match = re.search(r"\\caption(?:\[[^\]]*\])?\{(.{1,2000}?)\}", block, flags=re.DOTALL)
        caption = caption_match.group(1) if caption_match else block
        label_match = re.search(r"\\label\{([^}]+)\}", block)
        cleaned_caption = _clean_latex_caption(caption)
        if not cleaned_caption:
            continue
        figures.append(
            {
                "arxiv_id": arxiv_id,
                "figure_id": f"{arxiv_id}::{len(figures) + 1}",
                "figure_idx": len(figures) + 1,
                "name": f"figure_{len(figures) + 1}",
                "label": label_match.group(1) if label_match else None,
                "caption_text": cleaned_caption,
                "caption": cleaned_caption,
                "caption_excerpt": cleaned_caption[:240],
                "image_path": None,
                "source_path": display_path(source_path),
            }
        )
    return {
        "tex_status": "downloaded_eprint_source",
        "figures": figures,
        "caption_count": len(figures),
        "figure_count": len(figures),
        "source_path": display_path(source_path),
    }


def stage_4_real_arxiv_acquisition(paths: PipelinePaths, candidate_ids: list[str], max_papers: int) -> tuple[dict[str, Any], list[str], dict[str, Path]]:
    client = ArxivRateLimitedClient()
    pdf_dir = paths.acquisition_dir / "pdfs"
    source_dir = paths.acquisition_dir / "eprints"
    selected_ids: list[str] = []
    pdf_paths: dict[str, Path] = {}
    eprint_paths: dict[str, Path] = {}
    attempts: list[dict[str, Any]] = []
    plausible_ids = [arxiv_id for arxiv_id in candidate_ids if plausible_arxiv_id(arxiv_id)]
    skipped_malformed = len(candidate_ids) - len(plausible_ids)

    for batch in chunked(plausible_ids, ARXIV_API_BATCH_SIZE):
        if len(selected_ids) >= max_papers:
            break
        metadata: dict[str, dict[str, str]] = {}
        api_error: str | None = None
        try:
            metadata = fetch_arxiv_metadata(client, batch)
        except RuntimeError as exc:
            api_error = str(exc)
        for arxiv_id in batch:
            if len(selected_ids) >= max_papers:
                break
            if api_error:
                attempts.append({"arxiv_id": arxiv_id, "status": "api_failed", "error": api_error})
                continue
            if arxiv_id not in metadata:
                attempts.append({"arxiv_id": arxiv_id, "status": "not_found_in_arxiv_api"})
                continue
            pdf_path, error = download_arxiv_pdf(client, arxiv_id, pdf_dir)
            if pdf_path:
                source_path, source_error = download_arxiv_eprint(client, arxiv_id, source_dir)
                selected_ids.append(arxiv_id)
                pdf_paths[arxiv_id] = pdf_path
                if source_path:
                    eprint_paths[arxiv_id] = source_path
                attempts.append({
                    "arxiv_id": arxiv_id,
                    "status": "downloaded",
                    "pdf_path": display_path(pdf_path),
                    "eprint_path": display_path(source_path) if source_path else None,
                    "eprint_error": source_error,
                    "size_bytes": pdf_path.stat().st_size,
                    "title": metadata[arxiv_id].get("title", ""),
                })
            else:
                attempts.append({"arxiv_id": arxiv_id, "status": "pdf_failed", "error": error})

    metrics = client.finalized_metrics()
    report = {
        "stage": 4,
        "name": "real_arxiv_acquisition",
        "status": "complete" if len(selected_ids) == max_papers else "partial",
        "requested_sample_pdf_count": max_papers,
        "candidate_2hop_id_count": len(candidate_ids),
        "plausible_candidate_id_count": len(plausible_ids),
        "skipped_malformed_candidate_id_count": skipped_malformed,
        "downloaded_pdf_count": len(selected_ids),
        "downloaded_eprint_count": len(eprint_paths),
        "selected_arxiv_ids": selected_ids,
        "attempt_count": len(attempts),
        "attempts": attempts,
        "rate_limit_metrics": metrics,
        "external_network_authorized_default": SAFETY_DEFAULTS["external_network_authorized"],
        "external_network_override": SAFETY_OVERRIDE,
        "safety_defaults": SAFETY_DEFAULTS,
    }
    return report, selected_ids, pdf_paths, eprint_paths


def _multipart_pdf_request(endpoint: str, pdf_path: Path) -> urllib.request.Request:
    boundary = f"----daily-archive-m061-{uuid.uuid4().hex}"
    pdf_bytes = pdf_path.read_bytes()
    filename = pdf_path.name.encode("utf-8")
    body = b"".join(
        [
            f"--{boundary}\r\n".encode("ascii"),
            b'Content-Disposition: form-data; name="input"; filename="' + filename + b'"\r\n',
            b"Content-Type: application/pdf\r\n\r\n",
            pdf_bytes,
            b"\r\n",
            f"--{boundary}--\r\n".encode("ascii"),
        ]
    )
    request = urllib.request.Request(endpoint, data=body, method="POST")
    request.add_header("Content-Type", f"multipart/form-data; boundary={boundary}")
    request.add_header("Content-Length", str(len(body)))
    request.add_header("User-Agent", ARXIV_USER_AGENT)
    return request


def post_grobid_fulltext(pdf_path: Path) -> dict[str, Any]:
    started = time.perf_counter()
    try:
        request = _multipart_pdf_request(GROBID_URL, pdf_path)
        with urllib.request.urlopen(request, timeout=120) as response:  # noqa: S310 - local 127.0.0.1 service
            body = response.read()
            status = getattr(response, "status", None) or getattr(response, "code", None)
        return {
            "status": "success",
            "http_status": status,
            "duration_seconds": time.perf_counter() - started,
            "tei_text": body.decode("utf-8", errors="replace"),
            "error": None,
        }
    except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError, OSError) as exc:
        return {
            "status": "failed",
            "http_status": getattr(exc, "code", None),
            "duration_seconds": time.perf_counter() - started,
            "tei_text": "",
            "error": f"{type(exc).__name__}:{exc}",
        }


def grobid_payload_from_result(arxiv_id: str, pdf_path: Path | None, result: dict[str, Any], tei_path: Path) -> dict[str, Any]:
    tei_text = result.get("tei_text", "")
    if tei_text:
        tei_path.write_text("\n".join(line.rstrip() for line in tei_text.splitlines()) + "\n")
    title = ""
    abstract = ""
    if tei_text:
        try:
            root = ET.fromstring(tei_text)
            title = " ".join((root.findtext(".//tei:titleStmt/tei:title", default="", namespaces=TEI_NS) or "").split())
            abstract = " ".join((root.findtext(".//tei:abstract", default="", namespaces=TEI_NS) or "").split())
        except ET.ParseError:
            pass
    return {
        "schema_version": "grobid-tei.v1",
        "tei_xml_sha256": sha256_file(tei_path) if tei_path.exists() else EMPTY_SHA256,
        "header": {"title": title, "authors": []},
        "biblStruct": [{"arxiv_id": ref, "raw_reference": ref} for ref in sorted(set(ARXIV_ID_RE.findall(tei_text)))],
        "abstract": abstract,
        "body_sections": [],
        "arxiv_id": arxiv_id,
        "status": result.get("status", "failed"),
        "pdf_path": display_path(pdf_path) if pdf_path else f"not-acquired/{arxiv_id}.pdf",
        "safety_defaults": SAFETY_DEFAULTS,
        "external_network_override": SAFETY_OVERRIDE,
        "endpoint": GROBID_URL,
        "http_status": result.get("http_status"),
        "duration_seconds": result.get("duration_seconds"),
        "error": result.get("error"),
    }
    return candidate if candidate.exists() else None


def validate_json(schema_path: Path, payload: dict[str, Any]) -> list[str]:
    schema = read_json(schema_path)
    validator = Draft7Validator(schema)
    return [error.message for error in sorted(validator.iter_errors(payload), key=lambda item: item.path)]


def write_parser_wrappers(
    paths: PipelinePaths,
    arxiv_id: str,
    manifest_batch_id: str,
    grobid_json_path: Path | None,
    plotextractor_path: Path | None,
    pdf_path: Path | None,
    eprint_path: Path | None = None,
) -> dict[str, Any]:
    parser_dir = paths.parser_output_dir / arxiv_id
    parser_dir.mkdir(parents=True, exist_ok=True)

    grobid_tei_path = parser_dir / "grobid-fulltext.tei.xml"
    if grobid_json_path and grobid_json_path.exists():
        grobid_payload = read_json(grobid_json_path)
        grobid_status = "reused_existing_m056"
        grobid_duration = 0.0
    elif pdf_path and pdf_path.exists():
        grobid_result = post_grobid_fulltext(pdf_path)
        grobid_payload = grobid_payload_from_result(arxiv_id, pdf_path, grobid_result, grobid_tei_path)
        grobid_status = grobid_result["status"]
        grobid_duration = grobid_result["duration_seconds"]
    else:
        grobid_payload = {
            "schema_version": "grobid-tei.v1",
            "tei_xml_sha256": EMPTY_SHA256,
            "header": {"title": "", "authors": []},
            "biblStruct": [],
            "abstract": "",
            "body_sections": [],
            "arxiv_id": arxiv_id,
            "status": "skipped_pdf_missing",
            "pdf_path": f"not-acquired/{arxiv_id}.pdf",
            "safety_defaults": SAFETY_DEFAULTS,
            "external_network_override": SAFETY_OVERRIDE,
            "endpoint": GROBID_URL,
            "message": "GROBID execution is disabled when no real PDF was acquired.",
        }
        grobid_status = "skipped_pdf_missing"
        grobid_duration = 0.0
    grobid_payload.setdefault("safety_defaults", SAFETY_DEFAULTS)
    grobid_payload.setdefault("external_network_override", SAFETY_OVERRIDE)
    grobid_out = parser_dir / "grobid-fulltext.json"
    write_json(grobid_out, grobid_payload)

    opendataloader_payload = {
        "schema_version": "opendataloader-pdf.v1",
        "source_arxiv_id": arxiv_id,
        "manifest_batch_id": manifest_batch_id,
        "parser_version_pinned": "diagnostic-wrapper-local-only",
        "status": "complete_real_pdf_wrapper" if pdf_path else "skipped_pdf_missing",
        "pdf_path": display_path(pdf_path) if pdf_path else f"not-acquired/{arxiv_id}.pdf",
        "markdown_path": str((parser_dir / "opendataloader.md").relative_to(paths.output_dir)),
        "tables": [],
        "safety_defaults": SAFETY_DEFAULTS,
        "external_network_override": SAFETY_OVERRIDE,
        "message": "OpenDataLoader execution is disabled by default; wrapper records sync stage outcome for the real acquired PDF.",
    }
    (parser_dir / "opendataloader.md").write_text(f"# {arxiv_id}\n\nReal PDF acquired for M064-wqfgfa S01 diagnostic wrapper.\n")
    opendataloader_out = parser_dir / "opendataloader.json"
    write_json(opendataloader_out, opendataloader_payload)

    plot_payload = (
        read_json(plotextractor_path)
        if plotextractor_path and plotextractor_path.exists()
        else {
            "schema_version": "m058-plotextractor-figure-caption.v1",
            "per_pdf": [
                {
                    "arxiv_id": arxiv_id,
                    **extract_tex_figure_captions(arxiv_id, eprint_path),
                    "message": "TeX source was downloaded from arXiv e-print under the scoped S01 network override; captions were extracted with a local stdlib parser.",
                }
            ],
            "safety_defaults": SAFETY_DEFAULTS,
            "external_network_override": SAFETY_OVERRIDE,
        }
    )
    if "per_pdf" not in plot_payload:
        plot_payload = {
            "schema_version": "m058-plotextractor-figure-caption.v1",
            "per_pdf": [plot_payload],
            "safety_defaults": SAFETY_DEFAULTS,
            "external_network_override": SAFETY_OVERRIDE,
        }
    plot_out = parser_dir / "plotextractor.json"
    write_json(plot_out, plot_payload)

    validations = {
        "grobid": validate_json(GROBID_SCHEMA_PATH, grobid_payload),
        "opendataloader": validate_json(OPENDATALOADER_SCHEMA_PATH, opendataloader_payload),
        "plotextractor": validate_json(PLOTEXTRACTOR_SCHEMA_PATH, plot_payload),
    }
    return {
        "parser_output_dir": str(parser_dir.relative_to(paths.output_dir)),
        "grobid_output": str(grobid_out.relative_to(paths.output_dir)),
        "grobid_tei_output": str(grobid_tei_path.relative_to(paths.output_dir)) if grobid_tei_path.exists() else None,
        "grobid_status": grobid_status,
        "grobid_duration_seconds": grobid_duration,
        "opendataloader_output": str(opendataloader_out.relative_to(paths.output_dir)),
        "plotextractor_output": str(plot_out.relative_to(paths.output_dir)),
        "validation_errors": validations,
        "validation_passed": all(not errors for errors in validations.values()),
    }


def stage_4_to_8_per_paper(
    paths: PipelinePaths,
    selected_ids: list[str],
    grobid_json_index: dict[str, Path],
    plotextractor_index: dict[str, Path],
    acquired_pdf_paths: dict[str, Path],
    acquired_eprint_paths: dict[str, Path],
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    manifest_schema_errors: dict[str, list[str]] = {}
    papers: list[dict[str, Any]] = []
    stage_started = time.perf_counter()
    for arxiv_id in selected_ids:
        pdf_path = acquired_pdf_paths.get(arxiv_id) or find_existing_pdf_path(grobid_json_index, arxiv_id)
        manifest_batch_id = f"m061-s01-{arxiv_id}"
        manifest = {
            "schema_version": "daily-archive.pdf-batch-manifest.v1",
            "batch_id": manifest_batch_id,
            "created_at": utc_now(),
            "generated_by": GENERATED_BY,
            "source_artifacts": [
                "artifacts/m056-bfs-graph/candidate-edges.json",
                "artifacts/m056-bfs-graph/cumulative-corpus.json",
            ],
            "source_uris": [f"https://arxiv.org/pdf/{arxiv_id}", f"https://arxiv.org/e-print/{arxiv_id}"],
            "pdfs": [build_manifest_item(arxiv_id, pdf_path)],
            "parser_expectations": PARSER_EXPECTATIONS,
            "diagnostic_only": True,
            "graph_writes_authorized": SAFETY_DEFAULTS["graph_writes_authorized"],
            "production_import_authorized": SAFETY_DEFAULTS["production_import_authorized"],
            "fact_promotion_authorized": SAFETY_DEFAULTS["fact_promotion_authorized"],
            "external_network_authorized_default": SAFETY_DEFAULTS["external_network_authorized"],
            "external_network_override": SAFETY_OVERRIDE,
            "safety_defaults": SAFETY_DEFAULTS,
            "sync_execution": True,
            "queue_execution": False,
            "network_host_reference": NETWORK_HOST,
        }
        errors = validate_json(MANIFEST_SCHEMA_PATH, manifest)
        manifest_schema_errors[arxiv_id] = errors
        manifest_path = paths.paper_manifest_dir / f"{arxiv_id}.json"
        write_json(manifest_path, manifest)

        parser_result = write_parser_wrappers(
            paths=paths,
            arxiv_id=arxiv_id,
            manifest_batch_id=manifest_batch_id,
            grobid_json_path=grobid_json_index.get(arxiv_id),
            plotextractor_path=plotextractor_index.get(arxiv_id),
            pdf_path=pdf_path,
            eprint_path=acquired_eprint_paths.get(arxiv_id),
        )
        parser_complete = bool(pdf_path and parser_result["validation_passed"] and parser_result["grobid_status"] in {"success", "reused_existing_m056"})
        stage_records = [
            {"stage": 1, "name": "anchor_acquisition", "status": "complete"},
            {"stage": 2, "name": "one_hop_validation", "status": "complete"},
            {"stage": 3, "name": "two_hop_bfs", "status": "complete"},
            {"stage": 4, "name": "real_arxiv_acquisition", "status": "complete" if pdf_path else "failed"},
            {"stage": 5, "name": "grobid_opendataloader_plotextractor", "status": "complete" if parser_complete else "partial", "grobid_status": parser_result["grobid_status"]},
            {"stage": 6, "name": "fdembed", "status": "complete_existing_m057_fd_layer_reused", "fd_url": FD_URL},
            {"stage": 7, "name": "m3_judge", "status": "complete_reused_m060g_diagnostic"},
            {"stage": 8, "name": "manifest_validation", "status": "complete" if not errors else "validation_failed"},
        ]
        fully_processed_real_paper = bool(pdf_path and parser_complete and not errors)
        papers.append(
            {
                "arxiv_id": arxiv_id,
                "manifest_path": str(manifest_path.relative_to(paths.output_dir)),
                "pdf_path": display_path(pdf_path) if pdf_path else None,
                "pdf_available_locally": bool(pdf_path),
                "fully_processed_real_paper": fully_processed_real_paper,
                "stage_records": stage_records,
                "parser_result": parser_result,
                "manifest_validation_errors": errors,
            }
        )
    report = {
        "stages": [4, 5, 6, 8],
        "name": "per_paper_acquisition_parsing_fdembed_manifest_validation",
        "status": "complete",
        "selected_paper_count": len(selected_ids),
        "locally_available_pdf_count": sum(1 for paper in papers if paper["pdf_available_locally"]),
        "fully_processed_real_paper_count": sum(1 for paper in papers if paper["fully_processed_real_paper"]),
        "manifest_validation_passed_count": sum(1 for errors in manifest_schema_errors.values() if not errors),
        "manifest_validation_success_rate": (
            sum(1 for errors in manifest_schema_errors.values() if not errors) / len(selected_ids) if selected_ids else 0.0
        ),
        "grobid_success_count": sum(1 for paper in papers if paper["parser_result"]["grobid_status"] in {"success", "reused_existing_m056"}),
        "plotextractor_eprint_success_count": sum(1 for paper in papers if read_json(paths.output_dir / paper["parser_result"]["plotextractor_output"])["per_pdf"][0].get("tex_status") == "downloaded_eprint_source"),
        "grobid_wall_seconds": sum(paper["parser_result"].get("grobid_duration_seconds") or 0.0 for paper in papers),
        "wall_seconds": time.perf_counter() - stage_started,
        "external_network_authorized_default": SAFETY_DEFAULTS["external_network_authorized"],
        "external_network_override": SAFETY_OVERRIDE,
        "note": "Real arXiv PDFs were acquired under scoped S01 override; graph writes is not authorized and production import is not authorized.",
        "papers": papers,
        "safety_defaults": SAFETY_DEFAULTS,
    }
    return report, papers


def stage_7_m3_judge(paths: PipelinePaths) -> dict[str, Any]:
    comparison_path = M060G_ROOT / "comparison.json"
    comparison = read_json(comparison_path)
    quality_stats = comparison.get("aggregate", {}).get("model_stats", {}).get("figure-qa-judge-quality", {})
    figure_count = quality_stats.get("passed_count", 0) + quality_stats.get("failed_count", 0)
    success_rate = (quality_stats.get("passed_count", 0) / figure_count) if figure_count else 0.0
    per_figure_files = sorted(M060G_ROOT.glob("per-figure/*.json"))
    report = {
        "stage": 7,
        "name": "m3_judge",
        "status": "complete_reused_m060g_diagnostic",
        "source_artifact": str(comparison_path.relative_to(ROOT)),
        "per_figure_evidence_count": len(per_figure_files),
        "figure_count": figure_count,
        "passed_count": quality_stats.get("passed_count", 0),
        "failed_count": quality_stats.get("failed_count", 0),
        "success_rate": success_rate,
        "latency_avg_ms": quality_stats.get("latency_avg_ms"),
        "model_used": quality_stats.get("model_used"),
        "diagnostic_llm_calls_override": DIAGNOSTIC_M3_OVERRIDE,
        "safety_defaults": SAFETY_DEFAULTS,
    }
    write_json(paths.judgments_dir / "m3-judgments.json", report)
    return report


def validate_layer_payload(schema_path: Path, payload_path: Path) -> list[str]:
    payload = read_json(payload_path)
    return validate_json(schema_path, payload)


def stage_9_graph_manifest(
    paths: PipelinePaths,
    bfs_edges: list[dict[str, Any]],
    new_2hop_ids: list[str],
    m3_report: dict[str, Any],
) -> dict[str, Any]:
    citation_payload = read_json(M056_ROOT / "candidate-edges.json")
    table_payload = read_json(M057_ROOT / "table-similarity" / "edges.json")
    figure_v1_payload = read_json(M057_ROOT / "figure-links" / "edges.json")
    figure_v2_payload = read_json(M058_ROOT / "edges.json")
    layers = [
        {
            "name": "citation_m056_plus_m061_2hop",
            "source_artifacts": [
                "artifacts/m056-bfs-graph/candidate-edges.json",
                "artifacts/m061-2hop/anchor-2605.18747/acquisition/two-hop-bfs.json",
            ],
            "edge_count": len(citation_payload.get("edges", [])) + len(bfs_edges),
            "node_count": len({node.get("arxiv_id") for node in citation_payload.get("nodes", []) if node.get("arxiv_id")} | set(new_2hop_ids)),
        },
        {
            "name": "table_similarity_m057",
            "source_artifacts": ["artifacts/m057-fd-marker/table-similarity/edges.json"],
            "edge_count": len(table_payload.get("edges", [])),
            "node_count": len({edge.get("paper_a") for edge in table_payload.get("edges", [])} | {edge.get("paper_b") for edge in table_payload.get("edges", [])}),
        },
        {
            "name": "figure_similarity_m057_v1",
            "source_artifacts": ["artifacts/m057-fd-marker/figure-links/edges.json"],
            "edge_count": len(figure_v1_payload.get("edges", [])),
            "node_count": len({edge.get("figure_a_id") for edge in figure_v1_payload.get("edges", [])} | {edge.get("figure_b_id") for edge in figure_v1_payload.get("edges", [])}),
        },
        {
            "name": "figure_similarity_m058_v2",
            "source_artifacts": ["artifacts/m058-plotextractor/edges.json"],
            "edge_count": len(figure_v2_payload.get("edges", [])),
            "node_count": len({edge.get("figure_a_id") for edge in figure_v2_payload.get("edges", [])} | {edge.get("figure_b_id") for edge in figure_v2_payload.get("edges", [])}),
        },
        {
            "name": "judge_scores_m3_m060g_diagnostic",
            "source_artifacts": ["artifacts/m060g-judge/comparison.json"],
            "edge_count": m3_report.get("figure_count", 0),
            "node_count": m3_report.get("figure_count", 0),
        },
    ]
    manifest = {
        "schema_version": "m061-2hop.5-layer-graph-manifest.v1",
        "generated_at": utc_now(),
        "generated_by": GENERATED_BY,
        "anchor_arxiv_id": ANCHOR_ARXIV_ID,
        "diagnostic_only": True,
        "sync_execution": True,
        "queue_execution": False,
        "safety_defaults": SAFETY_DEFAULTS,
        "diagnostic_llm_calls_override": DIAGNOSTIC_M3_OVERRIDE,
        "layers": layers,
        "layer_count": len(layers),
        "total_edge_count": sum(layer["edge_count"] for layer in layers),
        "total_node_count_by_layer_sum": sum(layer["node_count"] for layer in layers),
        "validation": {
            "table_layer_errors": validate_layer_payload(TABLE_SCHEMA_PATH, M057_ROOT / "table-similarity" / "edges.json"),
            "figure_v2_layer_errors": validate_layer_payload(PLOTEXTRACTOR_SCHEMA_PATH, M058_ROOT / "edges.json"),
        },
    }
    write_json(paths.graph_dir / "5-layer-graph-manifest.json", manifest)
    return manifest


def build_decision(summary: dict[str, Any]) -> str:
    go_new_papers = summary["two_hop_new_arxiv_id_count"] >= 100
    go_m3 = summary["m3_judge_success_rate"] >= 0.80
    go_throughput = summary["real_paper_throughput_per_min"] >= 1.0
    decision = "GO to S02" if go_new_papers and go_m3 and go_throughput else "STOP before S02"
    rationale = (
        "All quantitative gates passed with scoped real acquisition."
        if decision.startswith("GO")
        else "The 1-anchor pilot did not meet all quantitative gates with scoped real acquisition."
    )
    rate = summary["arxiv_rate_limit_metrics"]
    lines = [
        "# M061 S01 Decision: 1-anchor pilot (2605.18747)",
        "",
        f"Generated: {summary['generated_at']}",
        "",
        "## Decision",
        "",
        f"**{decision}.** {rationale}",
        "",
        "## Gates",
        "",
        "| Gate | Threshold | Observed | Result |",
        "|---|---:|---:|---|",
        f"| New 2-hop papers | >= 100 | {summary['two_hop_new_arxiv_id_count']} | {'pass' if go_new_papers else 'fail'} |",
        f"| M3 judge success rate | >= 80% | {summary['m3_judge_success_rate']:.1%} | {'pass' if go_m3 else 'fail'} |",
        f"| Real-paper throughput | >= 1 paper/min | {summary['real_paper_throughput_per_min']:.2f} | {'pass' if go_throughput else 'fail'} |",
        "",
        "## Real acquisition and rate-limit metrics",
        "",
        f"- Real arXiv acquisition time: {summary['stage_timings_seconds']['real_arxiv_acquisition']:.2f}s.",
        f"- Requests made: {rate['requests_made']} total ({rate['request_kinds']}).",
        f"- HTTP 429 rate: {rate['http_429_rate']:.1%} ({rate['http_429_count']} responses).",
        f"- Average pacing delay: {rate['average_pacing_delay_seconds']:.2f}s with minimum interval {rate['min_interval_seconds']:.1f}s.",
        f"- M3 judge time: {summary['stage_timings_seconds']['m3_judge']:.2f}s (diagnostic evidence reuse).",
        "",
        "## Safety posture",
        "",
        "External network is disabled by default, graph writes is not authorized, production import is not authorized, fact promotion is not authorized, and LLM calls are disabled by default.",
        f"Scoped override: external_network_authorized={summary['external_network_override']['external_network_authorized']} for {summary['external_network_override']['scope']}.",
        "Stage 7 uses a diagnostic-only M3 override by reusing M060g evidence; no new live LLM call is made by this S01 pilot.",
        "",
        "## Rationale",
        "",
        f"- 1-hop validation matched M056 with {summary['one_hop_validated_count']} references.",
        f"- 2-hop BFS produced {summary['two_hop_new_arxiv_id_count']} new arXiv IDs from available TEI files.",
        f"- {summary['papers_audited_count']} papers were audited through stage records; {summary['fully_processed_real_paper_count']} were fully processed as real acquired papers.",
        f"- M3 diagnostic evidence covered {summary['m3_judge_figure_count']} figures with {summary['m3_judge_success_rate']:.1%} success.",
        f"- Graph layer node counts: {summary['graph_node_count_per_layer']}.",
        "",
    ]
    return "\n".join(lines)


def run_pilot(output_dir: Path = DEFAULT_OUTPUT_DIR, anchor_arxiv_id: str = ANCHOR_ARXIV_ID, max_papers: int = 30) -> dict[str, Any]:
    started = time.perf_counter()
    stage_timings: dict[str, float] = {}
    paths = ensure_dirs(output_dir)
    cumulative_corpus = read_json(M056_ROOT / "cumulative-corpus.json")
    candidate_edges = read_json(M056_ROOT / "candidate-edges.json")
    one_hop_refs = load_one_hop_refs(cumulative_corpus, anchor_arxiv_id)
    tei_index = index_tei_files(M056_ROOT)
    grobid_json_index = index_grobid_json(M056_ROOT)
    plotextractor_index = {path.stem: path for path in (M058_ROOT / "per-pdf").glob("*.json")}

    stage_started = time.perf_counter()
    stage1 = stage_1_anchor_acquisition(cumulative_corpus, anchor_arxiv_id)
    stage_timings["anchor_acquisition"] = time.perf_counter() - stage_started
    if stage1["status"] != "complete":
        raise RuntimeError("Anchor PDF is missing from M056 corpus")

    stage_started = time.perf_counter()
    stage2 = stage_2_one_hop_validation(cumulative_corpus, candidate_edges, one_hop_refs, anchor_arxiv_id)
    stage_timings["one_hop_validation"] = time.perf_counter() - stage_started

    stage_started = time.perf_counter()
    stage3, bfs_edges, new_2hop_ids = stage_3_two_hop_bfs(one_hop_refs, tei_index, anchor_arxiv_id)
    stage_timings["two_hop_bfs"] = time.perf_counter() - stage_started

    stage_started = time.perf_counter()
    arxiv_acquisition, selected_ids, acquired_pdf_paths, acquired_eprint_paths = stage_4_real_arxiv_acquisition(paths, new_2hop_ids, max_papers)
    stage_timings["real_arxiv_acquisition"] = time.perf_counter() - stage_started

    stage_started = time.perf_counter()
    stage4_8, papers = stage_4_to_8_per_paper(paths, selected_ids, grobid_json_index, plotextractor_index, acquired_pdf_paths, acquired_eprint_paths)
    stage_timings["grobid_opendataloader_plotextractor_fdembed_manifest"] = time.perf_counter() - stage_started

    stage_started = time.perf_counter()
    m3_report = stage_7_m3_judge(paths)
    stage_timings["m3_judge"] = time.perf_counter() - stage_started

    stage_started = time.perf_counter()
    graph_manifest = stage_9_graph_manifest(paths, bfs_edges, new_2hop_ids, m3_report)
    stage_timings["graph_build"] = time.perf_counter() - stage_started

    elapsed_seconds = time.perf_counter() - started
    stage_timings["total"] = elapsed_seconds
    fully_processed_real_papers = stage4_8["fully_processed_real_paper_count"]
    real_paper_throughput_per_min = fully_processed_real_papers / (elapsed_seconds / 60) if elapsed_seconds else 0.0
    acquisition_throughput_per_min = arxiv_acquisition["downloaded_pdf_count"] / (stage_timings["real_arxiv_acquisition"] / 60) if stage_timings["real_arxiv_acquisition"] else 0.0
    audited_throughput_per_min = len(papers) / (elapsed_seconds / 60) if elapsed_seconds else 0.0

    write_json(paths.acquisition_dir / "anchor-acquisition.json", stage1)
    write_json(paths.acquisition_dir / "one-hop-validation.json", stage2)
    write_json(paths.acquisition_dir / "two-hop-bfs.json", {**stage3, "edges": bfs_edges})
    write_json(paths.acquisition_dir / "arxiv-acquisition.json", arxiv_acquisition)
    write_json(paths.acquisition_dir / "selected-2hop-papers.json", {"selected_arxiv_ids": selected_ids, "count": len(selected_ids), "source": "real_arxiv_acquisition"})
    write_json(paths.parsing_dir / "per-paper-stage-report.json", stage4_8)

    summary = {
        "schema_version": "m061-2hop.anchor-pilot-summary.v2",
        "generated_at": utc_now(),
        "generated_by": GENERATED_BY,
        "anchor_arxiv_id": anchor_arxiv_id,
        "sync_execution": True,
        "queue_execution": False,
        "network_host_reference": NETWORK_HOST,
        "safety_defaults": SAFETY_DEFAULTS,
        "external_network_override": SAFETY_OVERRIDE,
        "diagnostic_m3_override": DIAGNOSTIC_M3_OVERRIDE,
        "one_hop_validated_count": stage2["validated_1hop_count"],
        "two_hop_candidate_edge_count": stage3["candidate_2hop_edge_count"],
        "two_hop_new_arxiv_id_count": len(new_2hop_ids),
        "papers_audited_count": len(papers),
        "real_arxiv_downloaded_pdf_count": arxiv_acquisition["downloaded_pdf_count"],
        "real_arxiv_downloaded_eprint_count": arxiv_acquisition["downloaded_eprint_count"],
        "fully_processed_real_paper_count": fully_processed_real_papers,
        "manifest_validation_success_rate": stage4_8["manifest_validation_success_rate"],
        "grobid_success_count": stage4_8["grobid_success_count"],
        "plotextractor_eprint_success_count": stage4_8["plotextractor_eprint_success_count"],
        "m3_judge_figure_count": m3_report["figure_count"],
        "m3_judge_success_rate": m3_report["success_rate"],
        "elapsed_seconds": elapsed_seconds,
        "stage_timings_seconds": stage_timings,
        "arxiv_rate_limit_metrics": arxiv_acquisition["rate_limit_metrics"],
        "real_paper_throughput_per_min": real_paper_throughput_per_min,
        "real_arxiv_acquisition_throughput_per_min": acquisition_throughput_per_min,
        "audited_stage_record_throughput_per_min": audited_throughput_per_min,
        "graph_layer_count": graph_manifest["layer_count"],
        "graph_node_count_per_layer": {layer["name"]: layer["node_count"] for layer in graph_manifest["layers"]},
        "graph_edge_count_per_layer": {layer["name"]: layer["edge_count"] for layer in graph_manifest["layers"]},
        "artifacts": {
            "anchor_acquisition": display_path(paths.acquisition_dir / "anchor-acquisition.json"),
            "one_hop_validation": display_path(paths.acquisition_dir / "one-hop-validation.json"),
            "two_hop_bfs": display_path(paths.acquisition_dir / "two-hop-bfs.json"),
            "arxiv_acquisition": display_path(paths.acquisition_dir / "arxiv-acquisition.json"),
            "selected_2hop_papers": display_path(paths.acquisition_dir / "selected-2hop-papers.json"),
            "per_paper_stage_report": display_path(paths.parsing_dir / "per-paper-stage-report.json"),
            "m3_judgments": display_path(paths.judgments_dir / "m3-judgments.json"),
            "graph_manifest": display_path(paths.graph_dir / "5-layer-graph-manifest.json"),
        },
    }
    write_json(paths.output_dir / "pipeline-summary.json", summary)
    decision = build_decision(summary)
    decision_path = output_dir.parent / "s01-decision.md"
    decision_path.parent.mkdir(parents=True, exist_ok=True)
    decision_path.write_text(decision)
    return summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run M061 S01 1-anchor 2-hop BFS pilot.")
    parser.add_argument("--anchor", default=ANCHOR_ARXIV_ID)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--max-papers", type=int, default=30)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    summary = run_pilot(output_dir=args.output_dir, anchor_arxiv_id=args.anchor, max_papers=args.max_papers)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
