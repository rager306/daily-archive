# Parser sidecars (GROBID / OpenDataLoader)

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

# 4) (Optional) warm OpenDataLoader workspace container
docker compose -f .docker/docker-compose.yml --env-file .env --profile odl up -d opendataloader

# 5) Host Python ODL library (preferred for hybrid runtime)
uv pip install opendataloader-pdf
```

Stop:

```bash
docker compose -f .docker/docker-compose.yml --env-file .env --profile odl down
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

- Default hybrid path is **injectable** and fail-closed.
- When live mode is enabled, the parser probes `GROBID_URL/api/isalive`.
- If down and auto-start is enabled, it runs compose for `grobid` and waits.
- OpenDataLoader is **library mode** (import `opendataloader_pdf`); the compose `odl` profile is optional workspace only.
- `hybrid_claimed_success` still requires body evidence (M212), not merely “container up”.

## Health probe

```bash
uv run python -c "from research_graph.infrastructure.corpus.parsing.sidecar_services import probe_parser_sidecars; import json; print(json.dumps(probe_parser_sidecars(), indent=2))"
```
