"""Models registry: Python API for models.yaml.

Per M049 (Models Registry Foundation) and M048 patterns-review 01
(ActiveGraph-style deterministic work_id).

Public API:
    load_models_registry(path=None) -> ModelsRegistry
    get_model(registry, model_id) -> ModelSpec
    get_model_for_binding(registry, binding_id) -> ModelSpec
    compute_work_id(model_id, binding_id, *, input_data, prompt_data,
                   tool_version=None, policy_version=None, run_id=None)
        -> str  # deterministic sha256 hex digest

Thread-safety: load_models_registry caches in module-level dict keyed by
resolved path. Multiple calls with same path return same ModelsRegistry
instance.

NOT in scope (deferred to M050/M051):
- Cache layer for resolved LLM responses (deferred to M050 worker pool).
- Hash content for arbitrary binary data (deferred to M050 fingerprint
  helpers, per M048 patterns-review 01 SG-E).
"""

from __future__ import annotations

import hashlib
import json
import os
import threading
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import yaml

# Default location of models.yaml (repo root).
DEFAULT_MODELS_YAML = Path("models.yaml")


@dataclass(frozen=True)
class ModelSpec:
    """Single model entry from models.yaml, after YAML coercion.

    All fields are required by the schema; missing values are an error
    surfaced by validate_models_yaml.py, not here.
    """

    id: str
    provider: str
    endpoint: str
    model_name: str
    tool_version: str
    policy_version: str


@dataclass(frozen=True)
class BindingSpec:
    """Single binding entry from models.yaml (maps usage to model)."""

    binding_id: str
    model_id: str
    description: str


@dataclass(frozen=True)
class ModelsRegistry:
    """Parsed models.yaml with fast lookup by id and binding_id."""

    schema_version: str
    models: dict[str, ModelSpec]
    bindings: dict[str, BindingSpec]

    def model_ids(self) -> list[str]:
        return list(self.models.keys())

    def binding_ids(self) -> list[str]:
        return list(self.bindings.keys())


# Module-level cache. Thread-safe via lock.
_CACHE: dict[str, ModelsRegistry] = {}
_CACHE_LOCK = threading.Lock()


def _coerce_version(value: Any) -> str:
    """Coerce version value to string. YAML may parse YYYY-MM-DD as date."""
    if value is None:
        return ""
    if isinstance(value, str):
        return value
    # date / datetime / other
    if hasattr(value, "isoformat"):
        return value.isoformat()
    return str(value)


def _load_registry_from_path(path: Path) -> ModelsRegistry:
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"models.yaml top-level must be a mapping, got {type(payload).__name__}")

    schema_version = str(payload.get("schema_version", ""))

    models: dict[str, ModelSpec] = {}
    for entry in payload.get("models", []):
        if not isinstance(entry, dict):
            continue
        models[entry["id"]] = ModelSpec(
            id=entry["id"],
            provider=entry["provider"],
            endpoint=entry["endpoint"],
            model_name=entry["model_name"],
            tool_version=_coerce_version(entry["tool_version"]),
            policy_version=_coerce_version(entry["policy_version"]),
        )

    bindings: dict[str, BindingSpec] = {}
    for entry in payload.get("bindings", []):
        if not isinstance(entry, dict):
            continue
        bindings[entry["binding_id"]] = BindingSpec(
            binding_id=entry["binding_id"],
            model_id=entry["model_id"],
            description=entry.get("description", ""),
        )

    return ModelsRegistry(
        schema_version=schema_version,
        models=models,
        bindings=bindings,
    )


def load_models_registry(path: str | os.PathLike[str] | None = None) -> ModelsRegistry:
    """Load and cache models.yaml. Idempotent for same path."""
    resolved = Path(path) if path is not None else DEFAULT_MODELS_YAML
    key = str(resolved.resolve()) if resolved.exists() else str(resolved)

    with _CACHE_LOCK:
        cached = _CACHE.get(key)
        if cached is not None:
            return cached

    if not resolved.exists():
        raise FileNotFoundError(f"models.yaml not found at {resolved}")

    registry = _load_registry_from_path(resolved)

    with _CACHE_LOCK:
        _CACHE[key] = registry
    return registry


def reset_cache() -> None:
    """Clear the in-process registry cache. Tests use this."""
    with _CACHE_LOCK:
        _CACHE.clear()


def get_model(registry: ModelsRegistry, model_id: str) -> ModelSpec:
    """Look up a model by id. Raises KeyError on unknown."""
    if model_id not in registry.models:
        raise KeyError(
            f"model_id={model_id!r} not in registry; "
            f"available: {sorted(registry.models.keys())}"
        )
    return registry.models[model_id]


def get_model_for_binding(registry: ModelsRegistry, binding_id: str) -> ModelSpec:
    """Resolve a binding_id to its model. Raises KeyError on unknown."""
    if binding_id not in registry.bindings:
        raise KeyError(
            f"binding_id={binding_id!r} not in registry; "
            f"available: {sorted(registry.bindings.keys())}"
        )
    binding = registry.bindings[binding_id]
    return get_model(registry, binding.model_id)


def _canonicalize(data: Any) -> str:
    """Canonical JSON string for deterministic hashing.

    - Sorts dict keys
    - Skips None values
    - Uses compact separators
    - Stable across Python versions
    """
    return json.dumps(data, sort_keys=True, default=str, separators=(",", ":"), ensure_ascii=False)


def compute_work_id(
    model_id: str,
    binding_id: str,
    *,
    input_data: Any,
    prompt_data: Any,
    tool_version: str | None = None,
    policy_version: str | None = None,
    run_id: str | None = None,
) -> str:
    """Deterministic sha256 hex digest for an LLM call.

    Formula:
        work_id = sha256(model_id || binding_id || input_data ||
                         prompt_data || tool_version || policy_version ||
                         run_id)

    Defaults: tool_version and policy_version default to the
    registry's value for the model; run_id defaults to "".
    input_data and prompt_data are JSON-serialized canonically.

    Same inputs produce same work_id (idempotency contract for
    cache reuse, per M048 patterns-review 01 SG-E).
    """
    if tool_version is None or policy_version is None:
        # Lazy-load registry to fill in defaults.
        registry = load_models_registry()
        model = get_model(registry, model_id)
        if tool_version is None:
            tool_version = model.tool_version
        if policy_version is None:
            policy_version = model.policy_version

    parts = [
        str(model_id),
        str(binding_id),
        _canonicalize(input_data),
        _canonicalize(prompt_data),
        str(tool_version or ""),
        str(policy_version or ""),
        str(run_id or ""),
    ]
    joined = "\x1f".join(parts)  # unit separator, not allowed in user input
    return hashlib.sha256(joined.encode("utf-8")).hexdigest()


__all__ = [
    "DEFAULT_MODELS_YAML",
    "ModelSpec",
    "BindingSpec",
    "ModelsRegistry",
    "load_models_registry",
    "reset_cache",
    "get_model",
    "get_model_for_binding",
    "compute_work_id",
]
