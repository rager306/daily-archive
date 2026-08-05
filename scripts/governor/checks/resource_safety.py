"""Resource safety check — disk space and memory before cargo builds.

Prevents mid-process crashes from:
- Disk full (target/ artifacts, logs, snapshots)
- Memory exhaustion (rocksdb rebuild, large PDF ingest)

Called from trajectory_service preflight.
"""

import os
import shutil
from pathlib import Path


def check_disk_space(path: str = "/") -> dict:
    """Check available disk space.

    Returns dict with:
    - free_gb: float
    - total_gb: float
    - used_pct: float
    - status: "healthy" | "warning" | "critical"
    - message: str
    """
    usage = shutil.disk_usage(path)
    free_gb = usage.free / (1024**3)
    total_gb = usage.total / (1024**3)
    used_pct = (usage.used / usage.total) * 100

    if free_gb < 2.0:
        status = "critical"
        message = f"CRITICAL: only {free_gb:.1f}GB free on {path} — cargo builds will fail"
    elif free_gb < 10.0:
        status = "warning"
        message = f"WARNING: {free_gb:.1f}GB free on {path} — large builds risky"
    else:
        status = "healthy"
        message = f"{free_gb:.1f}GB free on {path}"

    return {
        "free_gb": round(free_gb, 1),
        "total_gb": round(total_gb, 1),
        "used_pct": round(used_pct, 1),
        "status": status,
        "message": message,
    }


def check_memory() -> dict:
    """Check available memory via /proc/meminfo.

    Returns dict with:
    - free_gb: float
    - total_gb: float
    - available_gb: float
    - status: "healthy" | "warning" | "critical"
    - message: str
    """
    try:
        meminfo = Path("/proc/meminfo").read_text()
        info = {}
        for line in meminfo.splitlines():
            parts = line.split(":")
            if len(parts) == 2:
                key = parts[0].strip()
                val = int(parts[1].strip().split()[0])  # kB
                info[key] = val

        total_kb = info.get("MemTotal", 0)
        available_kb = info.get("MemAvailable", 0)
        free_kb = info.get("MemFree", 0)

        total_gb = total_kb / (1024**2)
        available_gb = available_kb / (1024**2)
        free_gb = free_kb / (1024**2)

        if available_gb < 0.5:
            status = "critical"
            message = f"CRITICAL: only {available_gb:.1f}GB memory available — OOM risk"
        elif available_gb < 2.0:
            status = "warning"
            message = f"WARNING: {available_gb:.1f}GB memory available — large builds risky"
        else:
            status = "healthy"
            message = f"{available_gb:.1f}GB memory available"

        return {
            "free_gb": round(free_gb, 2),
            "total_gb": round(total_gb, 2),
            "available_gb": round(available_gb, 2),
            "status": status,
            "message": message,
        }
    except Exception as e:
        return {
            "free_gb": 0,
            "total_gb": 0,
            "available_gb": 0,
            "status": "healthy",
            "message": f"Cannot read /proc/meminfo: {e}",
        }


def check_target_dir(workspace: str = ".") -> dict:
    """Check target/ directory size if it exists.

    Returns dict with size_gb and recommendation.
    """
    target = Path(workspace) / "target"
    if not target.exists():
        return {"exists": False, "size_gb": 0, "status": "healthy", "message": "target/ not found"}

    total = 0
    for dirpath, _dirnames, filenames in os.walk(target):
        for f in filenames:
            fp = os.path.join(dirpath, f)
            try:
                total += os.path.getsize(fp)
            except OSError:
                pass

    size_gb = total / (1024**3)

    if size_gb > 20:
        status = "warning"
        message = f"target/ is {size_gb:.1f}GB — consider 'cargo clean' before workspace builds"
    elif size_gb > 50:
        status = "critical"
        message = f"target/ is {size_gb:.1f}GB — run 'cargo clean' immediately"
    else:
        status = "healthy"
        message = f"target/ is {size_gb:.1f}GB"

    return {
        "exists": True,
        "size_gb": round(size_gb, 1),
        "status": status,
        "message": message,
    }


def preflight_check(workspace: str = ".") -> dict:
    """Full resource safety preflight.

    Returns combined status. If any check is critical, overall is critical.
    """
    disk = check_disk_space("/")
    mem = check_memory()
    target = check_target_dir(workspace)

    statuses = [disk["status"], mem["status"], target["status"]]
    if "critical" in statuses:
        overall = "critical"
    elif "warning" in statuses:
        overall = "warning"
    else:
        overall = "healthy"

    return {
        "overall": overall,
        "disk": disk,
        "memory": mem,
        "target_dir": target,
        "recommendations": _build_recommendations(disk, mem, target),
    }


def _build_recommendations(disk, mem, target) -> list[str]:
    recs = []
    if disk["status"] in ("warning", "critical"):
        recs.append("Run 'cargo clean' to free target/ space")
        recs.append("Remove stale .sgsnap files: rm artifacts/*/[old].sgsnap")
    if mem["status"] in ("warning", "critical"):
        recs.append("Close other processes before cargo builds")
        recs.append("Avoid cargo test --workspace (rocksdb memory heavy)")
    if target["status"] in ("warning", "critical"):
        recs.append(f"Run 'cargo clean' — target/ is {target.get('size_gb', 0)}GB")
    if not recs:
        recs.append("Resources healthy — proceed with builds")
    return recs


if __name__ == "__main__":
    import json

    result = preflight_check()
    print(json.dumps(result, indent=2))
    if result["overall"] == "critical":
        exit(1)
