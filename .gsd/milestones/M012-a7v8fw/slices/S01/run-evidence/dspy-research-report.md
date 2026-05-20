# DSPy compatibility research report

## Verdict

DSPy is conceptually compatible with the `daily-archive` extraction/evaluation boundaries, but **not ready for production runtime activation or positive KG import**. The safe near-term path is an optional/dev prototype that keeps DSPy behind an explicit boundary, preserves `ExtractionPatch` as the authoritative schema, disables optimizers fail-closed, and does not call external LMs in the production pipeline.

## Sources consulted

### Local DSPy source

- `/root/vendor-source/dspy/pyproject.toml`
- `/root/vendor-source/dspy/dspy/__init__.py`
- `/root/vendor-source/dspy/dspy/predict/predict.py`
- `/root/vendor-source/dspy/dspy/predict/chain_of_thought.py`
- `/root/vendor-source/dspy/dspy/primitives/module.py`
- `/root/vendor-source/dspy/dspy/primitives/example.py`
- `/root/vendor-source/dspy/dspy/primitives/prediction.py`
- `/root/vendor-source/dspy/dspy/signatures/signature.py`
- `/root/vendor-source/dspy/dspy/evaluate/evaluate.py`
- `/root/vendor-source/dspy/dspy/evaluate/metrics.py`
- `/root/vendor-source/dspy/dspy/adapters/json_adapter.py`
- `/root/vendor-source/dspy/dspy/teleprompt/*`

### Local daily-archive source/context

- `pyproject.toml`
- `src/arxiv_archive/dspy_extraction.py`
- `src/arxiv_archive/evaluation.py`
- `tests/test_dspy_extraction_boundary.py`
- M011 final recommendation and guards

### GitNexus context

- GitNexus repo: `dspy`
- Queried concepts: signatures, modules, optimizers, evaluation metrics, best practices.
- Relevant symbols/processes: `Signature`, `Predict`, `ChainOfThought`, `Module`, `Evaluate`, `MIPROv2`, `GEPA`, `BootstrapFewShot`, optimizer `.compile(...)` flows.

### External/current best-practice research

- DSPy official docs: `https://dspy.ai/`, evaluation, optimization, RAG, production/observability pages.
- Search query used by parent: `DSPy 2026 best practices evaluation first optimizers production observability typed extraction RAG`.
- Additional RAG/evaluation sources from 2026-oriented search results emphasized evaluation-first workflows, groundedness, component-level testing, and observability.

## Package/API findings

- Local DSPy source is version `3.2.1` and supports Python `>=3.10,<3.15`, so Python 3.13 is theoretically compatible.
- Top-level `import dspy` exposes broad runtime surfaces including predictors, modules, evaluate, adapters, clients, and teleprompt/optimizers.
- `Signature` defines typed input/output contracts via `InputField` and `OutputField`.
- `Predict` can be instantiated without an LM, but calling it requires configured `dspy.settings.lm` or predictor LM and otherwise fails with `ValueError`.
- `ChainOfThought` adds a reasoning output field and is unsafe for the current no-raw/no-rationale artifact boundary unless carefully suppressed.
- `Module` can support deterministic no-LM modules if `forward` avoids `Predict`/LM calls.
- `Evaluate` can run local program/metric loops, but must use `num_threads=1` for deterministic probes and must not use `save_as_csv`/`save_as_json` unless explicitly scoped.
- Built-in DSPy metrics are QA-oriented and are not sufficient for scientific KG import readiness.
- `JSONAdapter` can help structure output, but all outputs must still be coerced into and validated as `ExtractionPatch`.

## Optimizer findings

DSPy optimizers/teleprompters are powerful and high-risk for this project:

- Public optimizer classes include `BootstrapFewShot`, `BootstrapFewShotWithRandomSearch`, `MIPROv2`, `GEPA`, `COPRO`, `SIMBA`, `BetterTogether`, and others.
- Optimizers execute through `.compile(...)` and can run many LM calls and trace/program iterations.
- MIPROv2 and GEPA can be trace-heavy and artifact-heavy.
- MIPROv2 no longer provides an interactive permission gate as a safety boundary.

Required fail-closed policy:

1. Do not call `dspy.configure(lm=...)` in production paths.
2. Do not instantiate `dspy.LM` in production paths.
3. Do not import/use `dspy.teleprompt`, optimizer classes, or `.compile(...)` in production extraction paths.
4. Reject `optimizer_config`, `optimizer_name`, or similar runtime requests with explicit errors.
5. Keep optimizers dev-only and separately approved.
6. Preserve static tests that ban optimizer/runtime storage references in the boundary.

## Compatibility with daily-archive boundaries

The current project already has a safe DSPy-shaped local boundary:

- `DspyExtractionInput`
- `DspyExtractionOutput`
- `BaselineDspyExtractionModule`
- `dspy_extraction_signature_spec`

The current safe pattern:

- no hard `dspy` import;
- no external LM calls;
- no optimizer runtime;
- no persistence;
- callable returns `ExtractionPatch`;
- malformed outputs fail closed;
- diagnostics are IDs/counts/statuses, not raw text;
- schema validity and groundedness proxy remain project-owned.

This should remain the production boundary until an explicit future decision changes it.

## 2026 best-practice alignment

Current DSPy/RAG best-practice research supports the project's existing caution:

- Evaluation first: define dev sets and metrics before optimization.
- Component-level evaluation: retrieval, grounding, extraction schema validity, and final output correctness separately.
- Optimizers only after reliable metrics and budget caps exist.
- Observability/tracing is useful but risky because prompts/responses/traces can leak raw source text.
- Typed signatures/JSON output help structure, but do not replace local validation.

For `daily-archive`, the safe 2026-aligned chain is:

```text
DSPy candidate output -> parse/coerce -> ExtractionPatch -> validate_extraction_patch -> groundedness/evidence-path metrics -> redacted diagnostics
```

## Minimal no-LM probe design

A future isolated probe should:

1. Install DSPy only in an optional/dev environment.
2. Import DSPy and read version.
3. Define a small signature with synthetic non-paper inputs only.
4. Instantiate `dspy.Predict` but do not configure an LM.
5. Assert calling it fails closed with no LM configured.
6. Define a deterministic `dspy.Module` that returns a synthetic `dspy.Prediction` without LM calls.
7. Run `dspy.Evaluate` over one synthetic example with `num_threads=1`, no file saving, and a synthetic metric.
8. Confirm no optimizer imports, `.compile(...)`, file writes, raw text, production import, or LadybugDB writes.

## Risks/blockers

- Current environment import probe fails because `cloudpickle` is missing.
- DSPy dependency footprint includes `openai`, `litellm`, `diskcache`, `gepa[dspy]`, `json-repair`, `cloudpickle`, and related runtime surfaces.
- Top-level import exposes clients and teleprompt/optimizer namespaces.
- Chain-of-thought/rationale outputs can violate artifact redaction boundaries.
- MLflow/observability traces can log prompts, responses, examples, and model traces unless filtered.
- `Evaluate` and optimizers can write artifacts if configured.
- DSPy structured output does not prove scientific fact correctness.

## Go/no-go

### Go

- Go for a future isolated optional/dev prototype.
- Go for a no-LM import/evaluate dry-run once dependencies are explicitly installed in a dev context.
- Go for mapping DSPy signatures/predictions to `ExtractionPatch` only behind validation and no-import guards.

### No-go

- No-go for production runtime DSPy dependency now.
- No-go for DSPy optimizers now.
- No-go for positive KG import or trusted fact creation.
- No-go for production LadybugDB writes.

## Preconditions before activation

- Optional dependency plan approved.
- Minimal no-LM probe passes.
- Static optimizer/runtime guards pass.
- `ExtractionPatch` remains authoritative.
- Metrics/dev fixtures exist for extraction and groundedness.
- Observability redaction policy prevents prompt/response/raw text leakage.
- Explicit project decision permits any optimizer or external LM path.

## Safety flags

- raw_text_included: false
- chunk_text_included: false
- embeddings_included: false
- vectors_included: false
- secrets_included: false
- optimizer_enabled: false
- production_import_attempted: false
- ladybugdb_written: false
