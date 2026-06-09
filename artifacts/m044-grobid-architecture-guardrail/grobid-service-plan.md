# M044 GROBID Service Plan

- Selected image: `grobid/grobid:0.9.0-crf`
- Service URL: `http://127.0.0.1:8070`
- Docker CLI available: true
- Candidate only: true
- Graph writes: disabled
- Production import: disabled
- Fact promotion: disabled

## Health URLs

- `http://127.0.0.1:8070/api/isalive`
- `http://127.0.0.1:8070/api/health`
- `http://127.0.0.1:8070/api/version`

## Run command

```bash
docker run --rm --init --ulimit core=0 -p 8070:8070 grobid/grobid:0.9.0-crf
```
