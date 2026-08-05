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


def check_target_dir(workspace: str = ".", disk_free_gb: float | None = None) -> dict:
    """Check target/ directory size with detailed breakdown.

    Thresholds are ADAPTIVE based on available disk space:
    - If disk has >100GB free: tolerate larger target/ (warn >40GB, critical >80GB)
    - If disk has 20–100GB free: warn >20GB, critical >50GB
    - If disk has <20GB free: warn >5GB, critical >15GB

    Recommendations adapt: "plenty of space" vs "disk pressure — clean now".
    """
    target = Path(workspace) / "target"
    if not target.exists():
        return {"exists": False, "size_gb": 0, "status": "healthy", "message": "target/ not found"}

    total = 0
    subdir_sizes = {}
    for dirpath, _dirnames, filenames in os.walk(target):
        dir_total = 0
        for f in filenames:
            fp = os.path.join(dirpath, f)
            try:
                dir_total += os.path.getsize(fp)
            except OSError:
                pass
        total += dir_total

        # Categorize by top-level subdir
        rel = os.path.relpath(dirpath, target)
        top_subdir = rel.split(os.sep)[0] if rel != "." else "(root)"
        subdir_sizes[top_subdir] = subdir_sizes.get(top_subdir, 0) + dir_total

    size_gb = total / (1024**3)

    # Sort subdirs by size descending
    sorted_subdirs = sorted(subdir_sizes.items(), key=lambda x: -x[1])
    top_consumers = [
        {"dir": d, "size_gb": round(s / (1024**3), 2)}
        for d, s in sorted_subdirs[:5]
        if s > 100 * 1024 * 1024  # only show >100MB
    ]

    # Specific recommendations based on breakdown
    recommendations = []
    incremental_gb = subdir_sizes.get("debug" + os.sep + "incremental", 0) / (1024**3)
    deps_gb = subdir_sizes.get("debug" + os.sep + "deps", 0) / (1024**3)
    release_exists = "release" in subdir_sizes
    debug_gb = subdir_sizes.get("debug", 0) / (1024**3)
    release_gb = subdir_sizes.get("release", 0) / (1024**3)

    if incremental_gb > 2:
        recommendations.append(
            f"Remove incremental cache: rm -rf target/debug/incremental ({incremental_gb:.1f}GB)"
        )
    if release_exists and release_gb > 5:
        recommendations.append(
            f"Remove release profile if unused: rm -rf target/release ({release_gb:.1f}GB)"
        )
    if deps_gb > 5:
        recommendations.append(
            f"Dependencies cache large ({deps_gb:.1f}GB) — cargo clean -p <crate> to prune specific deps"
        )
    if debug_gb > 15:
        recommendations.append(
            f"Debug profile {debug_gb:.1f}GB — run 'cargo clean' before workspace builds"
        )

    # Adaptive thresholds based on available disk space (M360+)
    if disk_free_gb is not None:
        if disk_free_gb > 100:
            # Plenty of space — tolerate larger target/
            warn_threshold = 40
            crit_threshold = 80
            disk_context = f"{disk_free_gb:.0f}GB free on disk"
        elif disk_free_gb > 20:
            # Moderate space — standard thresholds
            warn_threshold = 20
            crit_threshold = 50
            disk_context = f"{disk_free_gb:.0f}GB free on disk"
        else:
            # Low space — aggressive thresholds
            warn_threshold = 5
            crit_threshold = 15
            disk_context = f"only {disk_free_gb:.0f}GB free — disk pressure"
    else:
        warn_threshold = 20
        crit_threshold = 50
        disk_context = "disk space unknown"

    if size_gb > crit_threshold:
        status = "critical"
        message = f"target/ is {size_gb:.1f}GB ({disk_context}) — run 'cargo clean' immediately"
        if disk_free_gb is not None and disk_free_gb <= 20:
            recommendations.insert(0, "DISK PRESSURE: cargo clean is urgent — not enough space for builds")
    elif size_gb > warn_threshold:
        status = "warning"
        message = f"target/ is {size_gb:.1f}GB ({disk_context}) — consider 'cargo clean' before workspace builds"
    else:
        status = "healthy"
        message = f"target/ is {size_gb:.1f}GB ({disk_context})"
        if not recommendations:
            recommendations.append("target/ size appropriate for available disk space")

    return {
        "exists": True,
        "size_gb": round(size_gb, 1),
        "status": status,
        "message": message,
        "top_consumers": top_consumers,
        "incremental_cache_gb": round(incremental_gb, 1),
        "deps_cache_gb": round(deps_gb, 1),
        "debug_profile_gb": round(debug_gb, 1),
        "release_profile_gb": round(release_gb, 1),
        "recommendations": recommendations if recommendations else ["target/ size within healthy range"],
    }


def preflight_check(workspace: str = ".") -> dict:
    """Full resource safety preflight.

    Returns combined status. If any check is critical, overall is critical.
    """
    disk = check_disk_space("/")
    mem = check_memory()
    target = check_target_dir(workspace, disk_free_gb=disk["free_gb"])

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
