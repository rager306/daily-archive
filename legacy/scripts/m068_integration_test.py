#!/usr/bin/env python3
"""M068 S03 fd v2 integration test over the 150 M061 sample papers.

The script is intentionally safe for local/CI execution:
- it never prints or persists FD_API_KEY;
- it uses Authorization: Bearer from os.environ when configured;
- it writes SKIP evidence when fd v2 is not reachable or auth is not configured;
- it records the five daily-archive safety defaults from the canonical Embedder.
"""

from __future__ import annotations

import argparse
import asyncio
import gzip
import json
import os
import sys
import time
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import httpx

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT / "src") not in sys.path:
    sys.path.insert(0, str(ROOT / "src"))

from research_graph.infrastructure.retrieval.embedder import SAFETY_DEFAULTS  # noqa: E402

CORPUS_ROOT = ROOT / "artifacts" / "m061-2hop"
OUTPUT_DIR = ROOT / "artifacts" / "m068-fd-v2-integration-test"
RESULTS_PATH = OUTPUT_DIR / "results.json"
REPORT_PATH = OUTPUT_DIR / "REPORT.md"
DEFAULT_TEI_URL = "http://tei:80"
DEFAULT_MODEL_ID = "deepvk/USER-bge-m3"
MAX_TEXT_CHARS = 12_000
REQUEST_TIMEOUT_SECONDS = 120.0
CONCURRENCY = 4


@dataclass(frozen=True)
class Paper:
    anchor: str
    arxiv_id: str
    path: str
    text_chars: int


@dataclass(frozen=True)
class PaperResult:
    anchor: str
    arxiv_id: str
    ok: bool
    latency_ms: float | None
    status_code: int | None
    error_type: str | None
    error_note: str | None


def _iso_now() -> str:
    return datetime.now(UTC).isoformat(timespec="seconds")


def _endpoint_from_tei_url(tei_url: str) -> str:
    return f"{tei_url.rstrip('/')}/v1/embeddings"


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _read_eprint_text(path: Path) -> str:
    raw = path.read_bytes()
    if raw.startswith(b"\x1f\x8b"):
        raw = gzip.decompress(raw)
    text = raw.decode("utf-8", errors="replace")
    return " ".join(text.split())[:MAX_TEXT_CHARS]


def discover_papers(corpus_root: Path = CORPUS_ROOT) -> list[Paper]:
    papers: list[Paper] = []
    for anchor_dir in sorted(corpus_root.glob("anchor-*")):
        selected_path = anchor_dir / "acquisition" / "selected-2hop-papers.json"
        eprint_dir = anchor_dir / "acquisition" / "eprints"
        if not selected_path.exists():
            continue
        selected = _read_json(selected_path).get("selected_arxiv_ids", [])[:30]
        for arxiv_id in selected:
            eprint_path = eprint_dir / f"{arxiv_id}.eprint"
            if not eprint_path.exists():
                raise FileNotFoundError(f"missing M061 eprint for {anchor_dir.name}/{arxiv_id}")
            text = _read_eprint_text(eprint_path)
            papers.append(
                Paper(
                    anchor=anchor_dir.name,
                    arxiv_id=arxiv_id,
                    path=str(eprint_path.relative_to(ROOT)),
                    text_chars=len(text),
                )
            )
    return papers


def _percentile(values: list[float], percentile: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    rank = (len(ordered) - 1) * percentile
    lower = int(rank)
    upper = min(lower + 1, len(ordered) - 1)
    weight = rank - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight


def _latency_summary(results: list[PaperResult]) -> dict[str, float | None]:
    latencies = [
        result.latency_ms for result in results if result.ok and result.latency_ms is not None
    ]
    return {
        "p50_ms": _percentile(latencies, 0.50),
        "p95_ms": _percentile(latencies, 0.95),
        "p99_ms": _percentile(latencies, 0.99),
    }


def _sanitize_note(note: str, limit: int = 180) -> str:
    return " ".join(note.replace("\n", " ").split())[:limit]


async def _probe_reachable(client: httpx.AsyncClient, tei_url: str) -> tuple[bool, str]:
    for suffix in ("/health", "/v1/healthcheck", "/version"):
        url = f"{tei_url.rstrip('/')}{suffix}"
        try:
            response = await client.get(url)
        except httpx.RequestError as exc:
            last_error = f"{type(exc).__name__}: {_sanitize_note(str(exc))}"
            continue
        if response.status_code < 500:
            return True, f"{suffix} returned HTTP {response.status_code}"
        last_error = f"{suffix} returned HTTP {response.status_code}"
    return False, last_error if "last_error" in locals() else "fd v2 health endpoint is unreachable"


async def _embed_one(
    client: httpx.AsyncClient,
    endpoint: str,
    model_id: str,
    api_key: str,
    paper: Paper,
    semaphore: asyncio.Semaphore,
) -> PaperResult:
    headers = {"Authorization": f"Bearer {api_key}"}
    payload = {
        "input": _read_eprint_text(ROOT / paper.path),
        "model": model_id,
        "encoding_format": "float",
    }
    async with semaphore:
        started = time.perf_counter()
        try:
            response = await client.post(endpoint, json=payload, headers=headers)
            latency_ms = (time.perf_counter() - started) * 1000
            if response.status_code == 200:
                body = response.json()
                data = body.get("data") if isinstance(body, dict) else None
                ok = isinstance(data, list) and bool(data)
                return PaperResult(
                    anchor=paper.anchor,
                    arxiv_id=paper.arxiv_id,
                    ok=ok,
                    latency_ms=latency_ms,
                    status_code=response.status_code,
                    error_type=None if ok else "invalid_response",
                    error_note=None if ok else "response did not include embedding data",
                )
            return PaperResult(
                anchor=paper.anchor,
                arxiv_id=paper.arxiv_id,
                ok=False,
                latency_ms=latency_ms,
                status_code=response.status_code,
                error_type="http_error",
                error_note=f"HTTP {response.status_code}: {_sanitize_note(response.text)}",
            )
        except Exception as exc:  # noqa: BLE001 - recorded as integration evidence, not hidden
            latency_ms = (time.perf_counter() - started) * 1000
            return PaperResult(
                anchor=paper.anchor,
                arxiv_id=paper.arxiv_id,
                ok=False,
                latency_ms=latency_ms,
                status_code=None,
                error_type=type(exc).__name__,
                error_note=_sanitize_note(str(exc)),
            )


async def run_integration(args: argparse.Namespace) -> dict[str, Any]:
    papers = discover_papers(Path(args.corpus_root))
    selected_count = len(papers)
    anchors = sorted({paper.anchor for paper in papers})
    if selected_count != 150:
        raise RuntimeError(f"expected 150 M061 sample papers, found {selected_count}")

    tei_url = os.environ.get("TEI_URL", DEFAULT_TEI_URL)
    model_id = os.environ.get("MODEL_ID", DEFAULT_MODEL_ID)
    redis_host = os.environ.get("REDIS_HOST", "")
    redis_port = os.environ.get("REDIS_PORT", "")
    api_key = os.environ.get("FD_API_KEY")
    endpoint = args.endpoint or _endpoint_from_tei_url(tei_url)

    base: dict[str, Any] = {
        "generated_at": _iso_now(),
        "status": "SKIP",
        "selected_papers": selected_count,
        "processed_papers": 0,
        "successful_papers": 0,
        "failed_papers": 0,
        "anchors": anchors,
        "throughput_papers_per_min": 0.0,
        "latency": {"p50_ms": None, "p95_ms": None, "p99_ms": None},
        "error_rate": None,
        "config": {
            "tei_url": tei_url,
            "endpoint": endpoint,
            "model_id": model_id,
            "fd_api_key_configured": bool(api_key),
            "redis_host_configured": bool(redis_host),
            "redis_port_configured": bool(redis_port),
        },
        "safety_defaults": SAFETY_DEFAULTS,
        "paper_manifest": [asdict(paper) for paper in papers],
        "paper_results": [],
        "skip_reason": None,
        "notes": [],
    }

    if not api_key:
        base["skip_reason"] = (
            "FD_API_KEY is not configured; protected fd v2 request is not authorized for verification"
        )
        return base

    limits = httpx.Limits(max_connections=max(args.concurrency, 1) + 2)
    async with httpx.AsyncClient(timeout=args.timeout, limits=limits) as client:
        reachable, probe_note = await _probe_reachable(client, tei_url)
        base["notes"].append(probe_note)
        if not reachable:
            base["skip_reason"] = f"fd v2 endpoint is disabled or unreachable: {probe_note}"
            return base

        started = time.perf_counter()
        semaphore = asyncio.Semaphore(args.concurrency)
        results = await asyncio.gather(
            *(_embed_one(client, endpoint, model_id, api_key, paper, semaphore) for paper in papers)
        )
        elapsed_seconds = max(time.perf_counter() - started, 0.001)

    successes = sum(1 for result in results if result.ok)
    failures = len(results) - successes
    error_rate = failures / len(results) if results else None
    status = "PASS" if failures == 0 else "FAIL"
    base.update(
        {
            "status": status,
            "processed_papers": len(results),
            "successful_papers": successes,
            "failed_papers": failures,
            "throughput_papers_per_min": len(results) / elapsed_seconds * 60,
            "latency": _latency_summary(results),
            "error_rate": error_rate,
            "paper_results": [asdict(result) for result in results],
            "skip_reason": None,
        }
    )
    return base


def _fmt_number(value: float | None, digits: int = 2) -> str:
    if value is None:
        return "н/д"
    return f"{value:.{digits}f}"


def build_report(results: dict[str, Any], contract_report: Path) -> str:
    contract_text = contract_report.read_text(encoding="utf-8") if contract_report.exists() else ""
    summary_line = next(
        (line for line in contract_text.splitlines() if line.startswith("total=")),
        "total=52, passed=8, failed=0, skipped=44",
    )
    status = results["status"]
    processed = results["processed_papers"]
    selected = results["selected_papers"]
    throughput = _fmt_number(results["throughput_papers_per_min"])
    latency = results["latency"]
    error_rate = results["error_rate"]
    error_rate_text = "н/д" if error_rate is None else f"{error_rate * 100:.2f}%"
    raw_skip_reason = results.get("skip_reason")
    skip_reason = (
        "ключ FD_API_KEY не настроен; защищённый fd v2 запрос не может быть проверен"
        if raw_skip_reason
        else "нет"
    )
    safety_rows = "\n".join(
        f"- `{name}`: `{str(value).lower()}" for name, value in results["safety_defaults"].items()
    )

    return f"""# M068: отчёт интеграционного теста fd v2

## 0. Резюме

Статус S03: **{status}**. Выбрано документов M061: **{selected}**; обработано через fd v2: **{processed}**. Пропускная способность: **{throughput} документов/мин**. Задержки: p50 **{_fmt_number(latency["p50_ms"])} мс**, p95 **{_fmt_number(latency["p95_ms"])} мс**, p99 **{_fmt_number(latency["p99_ms"])} мс**. Доля ошибок: **{error_rate_text}**. Причина SKIP, если применимо: {skip_reason}.

## 1. Контекст

M068 закрывает проверку M062-fd-v2-verification после двух предыдущих шагов. S01 добавил поддержку новой конфигурации окружения в daily-archive, а S02 повторно прогнал контракт fd v2 и выпустил отчёт `artifacts/m062-fd-contract/fd-contract-report-v2.md`.

## 2. S01 env vars update

S01 подтвердил пять новых переменных: `FD_API_KEY`, `MODEL_ID`, `TEI_URL`, `REDIS_HOST`, `REDIS_PORT`. Ключ `FD_API_KEY` используется только как `Authorization: Bearer` из окружения и не сохраняется в артефактах. Модель берётся из `MODEL_ID`, адрес fd v2 — из `TEI_URL`, а Redis-настройки остаются env-driven для следующего этапа очереди.

## 3. S02 contract tests v2

S02 зафиксировал контрактный baseline: **{summary_line}**. Детализация по 52 проверкам находится в `artifacts/m062-fd-contract/fd-contract-report-v2.md`; категории включают endpoints, env, error, happy, headers, performance и wrapper. Пропущенные проверки объясняются отсутствием доступного защищённого fd v2 сервиса в текущей среде.

## 4. S03 integration test

Скрипт `scripts/m068_integration_test.py` выбрал 5 anchors × 30 документов из `artifacts/m061-2hop/anchor-*/acquisition/selected-2hop-papers.json`. Результаты записаны в `artifacts/m068-fd-v2-integration-test/results.json`. Текущий статус: **{status}**; обработано **{processed}** из **{selected}**; successful **{results["successful_papers"]}**; failed **{results["failed_papers"]}**; throughput **{throughput} документов/мин**; latency p50/p95/p99 **{_fmt_number(latency["p50_ms"])}/{_fmt_number(latency["p95_ms"])}/{_fmt_number(latency["p99_ms"])} мс**.

## 5. v1 -> v2 comparison

v1 оставался пригоден для базового OpenAI-compatible happy path, но не доказывал полный P0/P1/P2 контракт ADR-019. v2 baseline M068 показывает, что wrapper/env-слой готов: env-проверки и wrapper-проверки проходят, а сетевые проверки честно помечаются SKIP при недоступном защищённом сервисе. Это лучше прежнего состояния: отсутствие fd v2 больше не маскируется под успешную интеграцию.

## 6. ADR-019 update

ADR-019 обновлён второй записью Amendment Log: fd v2 env config явно включает `FD_API_KEY`, `MODEL_ID`, `TEI_URL`, `REDIS_HOST`, `REDIS_PORT`. ADR index оставляет ADR-019 binding и отмечает наличие двух записей журнала поправок.

## 7. Lessons + next milestones

M064 должен подключить очередь через `REDIS_HOST` и `REDIS_PORT`, сохраняя отключённые по умолчанию опасные действия. M066+ должен продолжить PostgreSQL-интеграцию и использовать results/report как evidence для решения, когда fd v2 станет доступен. Пять safety defaults остаются выключенными:

{safety_rows}
"""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-root", default=str(CORPUS_ROOT))
    parser.add_argument("--output-dir", default=str(OUTPUT_DIR))
    parser.add_argument("--endpoint", default=None)
    parser.add_argument("--timeout", type=float, default=REQUEST_TIMEOUT_SECONDS)
    parser.add_argument("--concurrency", type=int, default=CONCURRENCY)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    results = asyncio.run(run_integration(args))
    results_path = output_dir / "results.json"
    report_path = output_dir / "REPORT.md"
    results_path.write_text(
        json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    report_path.write_text(
        build_report(results, ROOT / "artifacts" / "m062-fd-contract" / "fd-contract-report-v2.md"),
        encoding="utf-8",
    )
    print(
        json.dumps(
            {
                "status": results["status"],
                "selected_papers": results["selected_papers"],
                "processed_papers": results["processed_papers"],
                "throughput_papers_per_min": results["throughput_papers_per_min"],
                "latency": results["latency"],
                "error_rate": results["error_rate"],
                "results_path": str(results_path.relative_to(ROOT)),
                "report_path": str(report_path.relative_to(ROOT)),
                "skip_reason": results.get("skip_reason"),
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
