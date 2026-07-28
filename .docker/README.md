# Parser sidecars (GROBID)

GROBID TEI output is consumed by the Rust adapter `da_adapters::GrobidParser`
(`crates/da-adapters/src/grobid_parser.rs`). Python `research_graph.*` paths
are frozen under `legacy/` and are not on the runtime path.

## Quick start

From repo root:

```bash
# 1) Ensure env defaults exist (see ../.env.example)
cp -n .env.example .env  # if needed; do not overwrite secrets

# 2) Start GROBID CRF (HTTP :8070)
docker compose -f .docker/docker-compose.yml --env-file .env up -d grobid

# 3) Wait until alive
curl -sS http://127.0.0.1:8070/api/isalive
# expected: true
```

Stop:

```bash
docker compose -f .docker/docker-compose.yml --env-file .env down
```

## Env keys

| Key | Default | Meaning |
|-----|---------|---------|
| `GROBID_URL` | `http://127.0.0.1:8070` | Base URL for isalive + processFulltext |
| `GROBID_HOST_PORT` | `8070` | Host port published by compose |
| `GROBID_IMAGE` | `grobid/grobid:0.9.0-crf` | Image (M033/M044 pilot) |
| `GROBID_CONTAINER_NAME` | `daily-archive-grobid` | Container name |
| `GROBID_JAVA_OPTS` | `-Xmx4g` | JVM heap |
| `GROBID_AUTO_START` | `true` | Allow runtime to `compose up -d grobid` when down |
| `GROBID_START_TIMEOUT_SECONDS` | `180` | Wait for isalive after start |
| `ODL_AUTO_INSTALL` | `false` | Best-effort `uv pip install opendataloader-pdf` |
| `HYBRID_AUTO_START_CONTAINERS` | `true` | Hybrid path may ensure GROBID before live call |

GROBID has **no API key** in the default CRF image. Do not invent secrets.

## Hybrid runtime behavior

- The Rust `GrobidParser` (`da_adapters::GrobidParser::from_env()`) probes
  `GROBID_URL/api/isalive` on construction and before each parse.
- If auto-start is enabled, the CLI / use-case layer may `compose up -d grobid`
  and wait up to `GROBID_START_TIMEOUT_SECONDS`.
- GROBID TEI XML is parsed by `grobid_parser.rs::extract_sections` into
  `ParsedArticle`/`Section` domain types (see `da-domain::article`).
- Do not treat sidecar TEI/markdown as graph truth — the `RuleBasedExtractor`
  consumes parsed sections and emits canonical `Entity` nodes into Samyama
  Graph.

## Health probe (Rust)

```bash
# Quick health check via the Rust CLI
cargo run -p da-cli -- health

# Or directly test the parser adapter
cargo run -p da-cli --example eval_extract -- 2507.19457
```

Legacy Python probe (frozen, not on runtime path):

```bash
# legacy/scripts/ — for reference only, requires legacy/.venv
```
