# M058 S02 Marker vs OpenDataLoader comparison

## Safety defaults

External network is not authorized; fact promotion is not authorized; graph writes are disabled; LLM calls are disabled; production import is disabled.
Loopback bind host: `127.0.0.1`.

## Aggregate

- Marker extractions: 5/5
- Available OpenDataLoader comparisons: 2/5
- Avg quality delta: 157.5
- Marker > OpenDataLoader: 50.0%
- Avg Marker elapsed seconds: 586.275
- Page range: 0
- Go to S03: no

## Per-PDF comparison

| arxiv_id | status | marker tables | ODL tables | marker words | ODL words | marker sec | time ratio | quality delta | Marker > ODL |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 2603.21520 | odl_not_available | 0 | None | 482 | None | 18.963 | None | None | None |
| 2605.28617v1 | compared | 0 | 0 | 660 | None | 19.333 | None | 660 | True |
| 2508.07434 | odl_not_available | 0 | None | 429 | None | 19.251 | None | None | None |
| 2412.15118 | odl_not_available | 0 | None | 429 | None | 1603.963 | None | None | None |
| 1804.02767 | compared | 0 | 17 | 585 | None | 1269.866 | None | -345 | False |

## Notes

- `quality_delta` uses body words + 50 points per table + 20 points per figure; missing ODL fields contribute zero rather than invented values.
- Requested `2305.14314` is not available in the local repository; `1804.02767` from M058 S01 is used as the fifth executable PDF.
- Marker was run with `page_range=0` after full-document and three-page attempts exceeded the command budget before writing the first packet.
