"""Probe / ensure GROBID + OpenDataLoader sidecars for hybrid parser path.

Infrastructure only (docker compose + HTTP + importlib). Application policy still
decides hybrid honesty; this module never sets import_eligible.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

DEFAULT_GROBID_URL = "http://127.0.0.1:8070"
DEFAULT_COMPOSE_FILE = ".docker/docker-compose.yml"
DEFAULT_START_TIMEOUT_SECONDS = 180.0
USER_AGENT = "daily-archive-parser-sidecars/1.0"


def _env_bool(name: str, default: bool) -> bool:
    raw = os.environ.get(name)
    if raw is None:
        return default
    return raw.strip().lower() in {"1", "true", "yes", "on"}


def _env_float(name: str, default: float) -> float:
    raw = os.environ.get(name)
    if raw is None or not raw.strip():
        return default
    try:
        return float(raw)
    except ValueError:
        return default


def grobid_base_url() -> str:
    return (os.environ.get("GROBID_URL") or DEFAULT_GROBID_URL).rstrip("/")


def grobid_isalive_url(base: str | None = None) -> str:
    root = (base or grobid_base_url()).rstrip("/")
    if root.endswith("/api/isalive"):
        return root
    return f"{root}/api/isalive"


@dataclass(frozen=True, slots=True)
class SidecarProbe:
    name: str
    available: bool
    detail: str
    endpoint: str | None = None
    latency_ms: int | None = None
    auto_start_attempted: bool = False
    auto_start_ok: bool = False
    diagnostics: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "available": self.available,
            "detail": self.detail,
            "endpoint": self.endpoint,
            "latency_ms": self.latency_ms,
            "auto_start_attempted": self.auto_start_attempted,
            "auto_start_ok": self.auto_start_ok,
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class ParserSidecarStatus:
    grobid: SidecarProbe
    opendataloader: SidecarProbe
    compose_file: str
    diagnostics: tuple[str, ...] = field(default_factory=tuple)

    def to_dict(self) -> dict[str, Any]:
        return {
            "grobid": self.grobid.to_dict(),
            "opendataloader": self.opendataloader.to_dict(),
            "compose_file": self.compose_file,
            "diagnostics": list(self.diagnostics),
            "both_available": self.grobid.available and self.opendataloader.available,
        }


def probe_grobid(url: str | None = None, *, timeout_s: float = 5.0) -> SidecarProbe:
    endpoint = grobid_base_url() if url is None else url.rstrip("/")
    alive = grobid_isalive_url(endpoint)
    started = time.perf_counter()
    try:
        req = urllib.request.Request(alive, headers={"User-Agent": USER_AGENT})
        with urllib.request.urlopen(req, timeout=timeout_s) as resp:
            body = resp.read().decode("utf-8", errors="replace").strip().lower()
            ok = int(getattr(resp, "status", 0) or 0) == 200 and body == "true"
            latency = int((time.perf_counter() - started) * 1000)
            return SidecarProbe(
                name="grobid",
                available=ok,
                detail="isalive_true" if ok else f"unexpected_body:{body[:80]}",
                endpoint=endpoint,
                latency_ms=latency,
                diagnostics=(f"isalive_url:{alive}",),
            )
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        latency = int((time.perf_counter() - started) * 1000)
        return SidecarProbe(
            name="grobid",
            available=False,
            detail=f"{type(exc).__name__}:{exc}",
            endpoint=endpoint,
            latency_ms=latency,
            diagnostics=(f"isalive_url:{alive}", "connection_failed"),
        )


def probe_opendataloader() -> SidecarProbe:
    spec = importlib.util.find_spec("opendataloader_pdf")
    if spec is None:
        return SidecarProbe(
            name="opendataloader",
            available=False,
            detail="module_not_importable",
            diagnostics=("hint:uv_pip_install_opendataloader-pdf",),
        )
    return SidecarProbe(
        name="opendataloader",
        available=True,
        detail="importable",
        diagnostics=(f"origin:{spec.origin}",),
    )


def _compose_file_path(repo_root: Path | None = None) -> Path:
    root = repo_root or Path.cwd()
    rel = os.environ.get("GROBID_COMPOSE_FILE") or DEFAULT_COMPOSE_FILE
    path = Path(rel)
    return path if path.is_absolute() else (root / path)


def start_grobid_container(
    *,
    repo_root: Path | None = None,
    compose_file: Path | None = None,
) -> tuple[bool, str]:
    """docker compose up -d grobid. Returns (ok, detail)."""
    compose = compose_file or _compose_file_path(repo_root)
    if not compose.is_file():
        return False, f"compose_missing:{compose}"
    cmd = [
        "docker",
        "compose",
        "-f",
        str(compose),
        "up",
        "-d",
        "grobid",
    ]
    env_file = (repo_root or Path.cwd()) / ".env"
    if env_file.is_file():
        cmd[3:3] = ["--env-file", str(env_file)]
    try:
        proc = subprocess.run(
            cmd,
            check=False,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return False, f"compose_exec_failed:{type(exc).__name__}:{exc}"
    if proc.returncode != 0:
        err = (proc.stderr or proc.stdout or "").strip()[:400]
        return False, f"compose_exit_{proc.returncode}:{err}"
    return True, "compose_up_grobid_ok"


def wait_for_grobid(
    *,
    url: str | None = None,
    timeout_s: float | None = None,
    poll_s: float = 3.0,
) -> SidecarProbe:
    deadline = time.monotonic() + (
        timeout_s
        if timeout_s is not None
        else _env_float("GROBID_START_TIMEOUT_SECONDS", DEFAULT_START_TIMEOUT_SECONDS)
    )
    last = probe_grobid(url)
    while time.monotonic() < deadline:
        if last.available:
            return last
        time.sleep(poll_s)
        last = probe_grobid(url)
    return last


def ensure_grobid(
    *,
    auto_start: bool | None = None,
    repo_root: Path | None = None,
    url: str | None = None,
) -> SidecarProbe:
    """Probe GROBID; optionally docker-compose up and wait."""
    do_start = (
        _env_bool("GROBID_AUTO_START", True)
        if auto_start is None
        else auto_start
    )
    probe = probe_grobid(url)
    if probe.available:
        return probe
    if not do_start:
        return SidecarProbe(
            name="grobid",
            available=False,
            detail=probe.detail,
            endpoint=probe.endpoint,
            latency_ms=probe.latency_ms,
            auto_start_attempted=False,
            diagnostics=probe.diagnostics + ("auto_start_disabled",),
        )
    ok, detail = start_grobid_container(repo_root=repo_root)
    if not ok:
        return SidecarProbe(
            name="grobid",
            available=False,
            detail=detail,
            endpoint=probe.endpoint,
            auto_start_attempted=True,
            auto_start_ok=False,
            diagnostics=probe.diagnostics + (f"start_failed:{detail}",),
        )
    ready = wait_for_grobid(url=url)
    return SidecarProbe(
        name="grobid",
        available=ready.available,
        detail=ready.detail if ready.available else f"started_but_not_ready:{ready.detail}",
        endpoint=ready.endpoint,
        latency_ms=ready.latency_ms,
        auto_start_attempted=True,
        auto_start_ok=ready.available,
        diagnostics=ready.diagnostics + ("compose_start_attempted", detail),
    )


def ensure_opendataloader(*, auto_install: bool | None = None) -> SidecarProbe:
    """Probe ODL import; optional best-effort pip install (off by default)."""
    probe = probe_opendataloader()
    if probe.available:
        return probe
    do_install = (
        _env_bool("ODL_AUTO_INSTALL", False) if auto_install is None else auto_install
    )
    if not do_install:
        return SidecarProbe(
            name="opendataloader",
            available=False,
            detail=probe.detail,
            auto_start_attempted=False,
            diagnostics=probe.diagnostics + ("auto_install_disabled",),
        )
    try:
        proc = subprocess.run(
            ["uv", "pip", "install", "opendataloader-pdf"],
            check=False,
            capture_output=True,
            text=True,
            timeout=300,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return SidecarProbe(
            name="opendataloader",
            available=False,
            detail=f"install_failed:{type(exc).__name__}",
            auto_start_attempted=True,
            auto_start_ok=False,
            diagnostics=(f"exc:{exc}",),
        )
    again = probe_opendataloader()
    return SidecarProbe(
        name="opendataloader",
        available=again.available,
        detail=again.detail if again.available else f"install_exit_{proc.returncode}",
        auto_start_attempted=True,
        auto_start_ok=again.available,
        diagnostics=again.diagnostics + ("pip_install_attempted",),
    )


def probe_parser_sidecars(*, ensure: bool = False, repo_root: Path | None = None) -> ParserSidecarStatus:
    """Probe (and optionally ensure) both sidecars."""
    compose = str(_compose_file_path(repo_root))
    if ensure and _env_bool("HYBRID_AUTO_START_CONTAINERS", True):
        grobid = ensure_grobid(repo_root=repo_root)
        odl = ensure_opendataloader()
    else:
        grobid = probe_grobid()
        odl = probe_opendataloader()
    return ParserSidecarStatus(
        grobid=grobid,
        opendataloader=odl,
        compose_file=compose,
        diagnostics=(
            f"ensure:{ensure}",
            f"grobid_url:{grobid_base_url()}",
        ),
    )


__all__ = [
    "DEFAULT_COMPOSE_FILE",
    "DEFAULT_GROBID_URL",
    "ParserSidecarStatus",
    "SidecarProbe",
    "ensure_grobid",
    "ensure_opendataloader",
    "grobid_base_url",
    "grobid_isalive_url",
    "probe_grobid",
    "probe_opendataloader",
    "probe_parser_sidecars",
    "start_grobid_container",
    "wait_for_grobid",
]
