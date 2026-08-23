# OpenDataLoader PDF Runbook

## Verdict

`ready_for_hybrid_probe` for bounded M033 research.

## Preferred command

```bash
. /root/vendor-source/opendataloader-pdf/python/opendataloader-pdf/.venv-py313-build-check/bin/activate
opendataloader-pdf-hybrid --port 5002
opendataloader-pdf --hybrid docling-fast --hybrid-url http://127.0.0.1:5002 -f json,markdown,html,text <pdf>
```

## Fallbacks

```bash
opendataloader-pdf -f json,markdown,html,text <pdf>
java -Djava.awt.headless=true -jar /root/vendor-source/opendataloader-pdf/java/opendataloader-pdf-cli/target/opendataloader-pdf-cli-0.0.0.jar -f json,markdown,html,text <pdf>
```

## Cache dependency

- `/root/.cache/huggingface/hub/models--docling-project--docling-layout-heron` snapshot `8f39ad3c0b4c58e9c2d2c84a38465abf757272d8` (~164M).
- `/root/.cache/huggingface/hub/models--docling-project--docling-models` snapshot `fc0f2d45e2218ea24bce5045f58a389aed16dc23` (~342M).
- If cache is absent, hybrid may require network/model downloads.

## Safety

No graph import, no LadybugDB writes, no production import.
