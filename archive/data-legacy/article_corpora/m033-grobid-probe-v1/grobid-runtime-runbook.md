# M033 S02 GROBID Runtime Runbook

## Selected path

Use the GROBID CRF Docker service for bounded S02 research:

```bash
docker run --rm --init --ulimit core=0 -p 8070:8070 grobid/grobid:0.9.0-crf
```

Health checks:

```text
http://127.0.0.1:8070/api/isalive
http://127.0.0.1:8070/api/health
http://127.0.0.1:8070/api/version
```

## Why Docker for S02

Vendored GROBID source requires OpenJDK 21+ for native builds. The local runtime is Java 17, while Docker daemon is available on x86_64 Linux with enough memory. Docker keeps the probe bounded and avoids host JDK changes.

## CRF vs full image

- `grobid/grobid:0.9.0-crf`: smaller/faster CRF-only image, sufficient for API and TEI contract shape research.
- `grobid/grobid:0.9.0-full`: larger DL image, better for bibliography/citation quality comparison, but not required for this first bounded probe.

## Safety boundary

All GROBID outputs in this slice are candidate parser evidence only. They do not imply graph readiness, import eligibility, production parser adoption, or LadybugDB writes.
