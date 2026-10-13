# M517 cycle060, cycle120 and cycle240: independent result review

Reviewed 2026-10-13; final update includes cycle240. Read-only review of frozen inputs and actual captures/results. This reviewer ran no cargo, capture, parser, evaluator binary, config/index/provider mutation or repository edit. Only this report is written under `/tmp/policy-resume-audit/`.

## Verdict

**All three cycles’ calculations and evidence bindings are internally consistent. No scoring or denominator blocker was found in the candidate-only campaign.** Independent reconstruction from all **320 captured documents** (240 training + the same80 validation) matches all eight tables for each cycle, per-document validation scores/selections, training/validation aggregates, disagreements and regression counts.

**The results do not show an overall parse-quality gain.** Ordered-5gram winner reward rises from **0.55 to0.6125**, but mean raw ordered-5gram F1 **falls from0.7126654227954353 to0.6881427908328659**. There are **31 ordering regressions**, including20 documents whose token F1 improves. Seven of the eight metric-specific tables select identical validation arms. Cycles120and240 add training support but change no validation execution or outcome versus cycle060. No deployment, promotion, import, graph-write or production-speedup claim is supported.

## 1. Evidence and reconstruction

Read the requested11:04 checkpoint, `manifest.json`, all three cycles/relevant diagnostic JSON, canonical capture/evaluator provenance, `scope.json`, frozen registry/experiment/legacy DATA, prior independent/readiness reviews and `doc/review/policy-corpus-cycles.md`.

For all240 training PDFs plus all80 validation PDFs, independently checked:

- PDF SHA, byte/source/paper identity and source-family/partition bindings; no train/validation intersection.
- Capture SHA, exporter/furniture hashes, observation/source/page/bookmark bindings, reference hash/character count, article hash/identity and section/body-character evidence.
- Both arm outcomes and structural GROBID profile; all inspected GROBID arms are version0.9.1, grammar `arxiv_pdf_dot`, recorded code `xy_cut`, profile `structural`. TOC uses PyMuPDF1.28.0.
- Raw token and5-token-window counts from actual article `sections.text` and saved `pdftotext` reference. A separate Python Counter calculation matches every reported matches/precision/recall/F1 exactly. No Rust function or emitted score was used as the raw oracle.
- Metric-specific pair winners/rewards, bucket/per-code means, support and learned-cell choice, native DATA choice, selected execution/reward/regret and all summary denominators. Each of the three cycles has8×80 validation rows. Every check matched; no hash, raw score, table, row or aggregate discrepancy.

The full manifest has320 unique PDF hashes and320 IDs:240train/80validation. The final review reconstructs all320 source/capture bindings and raw text scores, then each frozen60/120/240training prefix. All320PDF, capture, observation, reference and successful article hashes were rechecked after240arrived. Historical copied files still match all three frozen hashes. No preflight capture enters reviewed inputs. The corrected manifest and canonical provenance fixes described by the earlier reviewers are in place.

### Frozen identities

| Identity | SHA-256 |
| --- | --- |
| Final manifest | `dc2c75a045bbba8e9d90b4fc90e3b2fa5bdaf07276be573808bac395164936a1` |
| Experiment DATA | `b7dc7fafd3832d95e7275335fb1f4e2dc20a3f1dc1ed21bc8e06ff047755b31e` |
| Native registry | `300b1c2107cfcf683a17ced994535667ef49b416e54759f54bc9d61fcc376f14` |
| Legacy metric DATA | `a27d3852d50ff7f2d4fae585f9210912c7066c697171bfcb32fcb5cad61a29b7` |
| Evaluator source | `e863d68dc480f32a1266c51009defef01f2db39e77f817b767d1d304314dc75e` |
| Shared capture carrier source | `a05d914bb51dab3d2695df9eff3c5aef5c5651f03000f81ccacd0b40070cf64b` |
| Capture source | `f0f90652f7b6c89910e22616681e171962a412aaad308e72b7d1ae6fd793bac9` |
| Evaluator executable | `7730cb76be2b8213dee833d4c03853b285217d0fe89625f0ec980280ae75c875` |
| Capture executable | `f2cd924d0345a971449fab4dca37321188fa25a3385b05c073b84abb5a3dfebc` |
| cycle060 report | `a7b62feea2d7ea275581d3402c28aca31289c35b5e24576b481813f3526e6c38` |
| cycle120 report | `6fe2e784ce7002a9e8bb5c9eb52eacd82fbd52426f1bf3b9e15f56c003fc6d4c` |
| cycle240 report | `24cb0dd3c484c0d0667cc0ad7eafbd960cf078194eb999ad1ccdad45c32ead42` |

These match the current provenance/source/binary bytes. Starting Git revision718be28b5 predates the uncommitted recipe and is not its identity. All three reports retain `candidate_only=true`, `deployed=false`, `import_eligible=false`, `auto_promote=forbidden`, `graph_write=forbidden`.

## 2. Real denominators

- **Cycle060 train:**60 distinct documents =53 successful full pairs +7 empty-TOC forced GROBID successes.120 arm attempts project to113 training observations:106 successful full-pair arm observations +7 forced GROBID observations. NotApplicable TOC contributes no observation. For `sections`,52decisive full pairs +1tie; other variants have53decisive full pairs.
- **Cycle120 train:**120 distinct documents =107 successful full pairs +13 forced GROBID successes.240 arm attempts project to227 observations, not240independent samples.
- **Cycle240 train:**240 distinct documents =206 successful full pairs +34 forced GROBID successes.480 arm attempts project to446 observations. `sections` has196decisive full pairs +10ties; other metrics have206decisive full pairs. One50–199-page empty-TOC training record (`2501.17399`) has1arm observation, below support4, so candidate uses native there and train coverage is239/240. It adds no learned cell.
- **Fixed validation:**80 distinct documents =68 successful full pairs +12 empty-TOC forced GROBID successes. There are no failed arms or selected NotApplicable outputs in any of the three reviewed cycles. Every selected policy succeeds80/80; success is operational evidence, not reward1.
- Reward/regret/latency denominator is80 assigned documents. Successful-selected text means happen also to have n80 here. Decisive full-pair accuracy is **68**, except `sections`, whose3ties leave **65**.
- All tables have learned coverage79/80(98.75%):67 positive-bookmark short articles +12 empty-TOC records; the587-page `trench-real-analysis` book uses native fallback. Support means are **79 matched documents**, not80. There are no level1/level2 fallback matches in any cycle.
- Forced-selected reward is1 with n12 for native and every candidate. These12 contribute12 reward points to all variants without measuring TOC-vs-GROBID quality. For ordered5gram, headline49/80=0.6125 becomes37/68=0.5441176470588235 when reporting decisive comparisons; native44/80=0.55 becomes32/68=0.47058823529411764. The forced successes do not create the paired delta, but raise both headline levels. Coverage excluding forced records is67/68.

## 3. Selected-policy rewards: same validation values in060,120and240

Reward is a metric-specific **rank**: winner1, loser0, successful tie0.5. It is not the raw metric value. The sums below expose the80-document denominator. Full-pair columns exclude forced successes/ties.

| Metric | Native reward | Candidate reward | Delta | Native decisive correct/n | Candidate decisive correct/n | Candidate regret mean |
| --- | --- | --- | --- | --- | --- | --- |
| `body_chars` | 13/80 = 0.1625 | 80/80 = 1.0 | +0.83750 | 1/68 | 68/68 | 0.0 |
| `chars_per_ms` | 13/80 = 0.1625 | 80/80 = 1.0 | +0.83750 | 1/68 | 68/68 | 0.0 |
| `legacy_yield` | 16/80 = 0.2 | 77/80 = 0.9625 | +0.76250 | 4/68 | 65/68 | 0.0375 |
| `ordered_5gram_f1` | 44/80 = 0.55 | 49/80 = 0.6125 | +0.06250 | 32/68 | 37/68 | 0.3875 |
| `quality_then_latency` | 44/80 = 0.55 | 49/80 = 0.6125 | +0.06250 | 32/68 | 37/68 | 0.3875 |
| `sections` | 63.5/80 = 0.79375 | 63.5/80 = 0.79375 | +0.00000 | 50/65 | 50/65 | 0.1875 |
| `sections_per_ms` | 16/80 = 0.2 | 77/80 = 0.9625 | +0.76250 | 4/68 | 65/68 | 0.0375 |
| `token_f1` | 24/80 = 0.3 | 69/80 = 0.8625 | +0.56250 | 12/68 | 57/68 | 0.1375 |

Native regret means are0.8375(`body_chars`,`chars_per_ms`),0.8(`legacy_yield`,`sections_per_ms`),0.45(`ordered_5gram_f1`,`quality_then_latency`),0.1875(`sections`),0.7(`token_f1`). For `sections`,63.5 reward points =50decisive successes +12forced successes +1.5tie points. Regret is0.1875 rather than1−0.79375 because the oracle also gets0.5, not1, on each of the3ties.

Against native, non-section tables change67 executions, all GROBID→TOC. Improvements/regressions in selected reward are:

- body/char-throughput:67/0.
- legacy/section-throughput:64/3.
- token F1:56/11.
- ordered5gram and quality-then-latency:36/31.
- sections:0/0; no execution changes on validation.

## 4. Raw quality disagrees with winner-count reward

Seven non-section tables are byte-identical within each cycle. Cycle060 seal `dae50764f5bd8e7bf94cf1e99049ae864ac2429b6688f7f686137b8690fde817` chooses TOC in both positive-bookmark page buckets and GROBID for empty TOC. They all select **68TOC/12GROBID** on validation. Native and `sections` select **1TOC/79GROBID**. These seven reports are not seven different improved policies; they score the same selected outputs under different rank criteria.

| Successful selected diagnostic (n80 each) | Native / sections | Every non-section candidate | Difference |
| --- | --- | --- | --- |
| Mean token F1 |0.7989084818096662|0.8714077328580746|+0.0724992510484084|
| Mean local ordered5gram F1 |0.7126654227954353|0.6881427908328659|−0.0245226319625694|
| Mean section-text characters |61570.7875|93231.2|+31660.4125|
| Mean chars/ms (per-document mean) |10.240068619179524|54.017322358649345|+43.77725373946982|
| Mean measured selected wall time, ms |6018.05|2398.1|−3619.95|

Token F1:56gains,11losses,13unchanged. Ordered5gram:36gains,31losses,13unchanged. Ordered5gram gains sum **+6.290747139043705**, mean **+0.17474297608454736** across36improved documents. Losses sum **−8.252557696049287**, mean **−0.26621153858223506** across31regressions. The31larger losses outweigh36gains; the same policy wins more documents yet has a lower average raw score. This follows from frequency-of-wins training/aggregation, not a score arithmetic defect.

Train-to-validation selected-reward gap is large for the ordered variants at060. Adding training documents changes the training average, not validation outputs:

| Metric | Train reward060 | Train reward120 | Train reward240 | Train−validation gap060 | Gap120 | Gap240 |
| --- | --- | --- | --- | --- | --- | --- |
| `body_chars` | 1 | 0.991666666667 | 0.991666666667 | 0 | -0.00833333333333 | -0.00833333333333 |
| `chars_per_ms` | 0.966666666667 | 0.975 | 0.979166666667 | -0.0333333333333 | -0.025 | -0.0208333333333 |
| `legacy_yield` | 0.933333333333 | 0.925 | 0.933333333333 | -0.0291666666667 | -0.0375 | -0.0291666666667 |
| `ordered_5gram_f1` | 0.8 | 0.683333333333 | 0.675 | 0.1875 | 0.0708333333333 | 0.0625 |
| `quality_then_latency` | 0.8 | 0.691666666667 | 0.683333333333 | 0.1875 | 0.0791666666667 | 0.0708333333333 |
| `sections` | 0.825 | 0.808333333333 | 0.779166666667 | 0.03125 | 0.0145833333333 | -0.0145833333333 |
| `sections_per_ms` | 0.933333333333 | 0.925 | 0.929166666667 | -0.0291666666667 | -0.0375 | -0.0333333333333 |
| `token_f1` | 0.916666666667 | 0.883333333333 | 0.895833333333 | 0.0541666666667 | 0.0208333333333 | 0.0333333333333 |

A reduced gap alone is not improved validation generalization: validation selection, output and reward did not change. Training ordered5gram token/5gram F1 gains therefore do not establish the same gain on validation.

## 5. All31 ordering-regression examples

All31rows select native GROBID reward1 versus candidate TOC reward0 under `ordered_5gram_f1` and `quality_then_latency`. All come from `grammar_route;p=1_49;b=1;q=ge700;d=_`, support100arm observations/50documents at060,200/100at120and390/195at240. All31deltas and identities agree with diagnostics and raw text. **20token F1 gains coexist with5gram loss;11lose both.** Values below are rounded6decimals; JSON/raw reconstruction retains full precision. SHA prefixes identify the full hashes in reports.

| Paper ID | Source SHA prefix | Token native | Token candidate |5gram native|5gram candidate|5gram delta|
| --- | --- | --- | --- | --- | --- | --- |
| `2508.00083` | `024719fb2681` | 0.821663 | 0.875408 | 0.773196 | 0.407527 | -0.365669 |
| `2506.06261` | `07f02bcc20a1` | 0.839149 | 0.783771 | 0.678270 | 0.529284 | -0.148985 |
| `2505.15471` | `15f53bb95f04` | 0.743700 | 0.922560 | 0.677221 | 0.435476 | -0.241745 |
| `2408.15971` | `1aa0e704df22` | 0.876395 | 0.933588 | 0.821965 | 0.683831 | -0.138134 |
| `2409.05001` | `1d6d9322f1b7` | 0.848445 | 0.923189 | 0.774962 | 0.531566 | -0.243396 |
| `2402.17753` | `218188e1d66a` | 0.802968 | 0.906796 | 0.720088 | 0.458390 | -0.261697 |
| `2511.21689` | `26700ad59305` | 0.799467 | 0.601002 | 0.733968 | 0.557996 | -0.175972 |
| `2504.12951` | `27f45fe5e837` | 0.817841 | 0.898890 | 0.785838 | 0.488768 | -0.297070 |
| `2404.13050` | `3257fbd91a1d` | 0.798748 | 0.913579 | 0.744434 | 0.590282 | -0.154152 |
| `2506.04625` | `3610cf5c5208` | 0.846915 | 0.925152 | 0.778306 | 0.637184 | -0.141122 |
| `2505.19443` | `3d643da95be9` | 0.788586 | 0.692909 | 0.739105 | 0.348306 | -0.390799 |
| `2502.14932` | `54a92b07d56d` | 0.772348 | 0.924651 | 0.662133 | 0.478934 | -0.183199 |
| `2509.24219` | `5b978b328396` | 0.882175 | 0.601828 | 0.782672 | 0.332615 | -0.450057 |
| `2601.15709` | `5f8f262e7ad8` | 0.872375 | 0.876628 | 0.822262 | 0.519671 | -0.302591 |
| `2505.09970` | `622b5a58ca8a` | 0.838039 | 0.915817 | 0.764347 | 0.551996 | -0.212351 |
| `2410.13610` | `6687965239e6` | 0.889791 | 0.946353 | 0.838424 | 0.431974 | -0.406450 |
| `2411.07228` | `6d69040be09e` | 0.853797 | 0.958282 | 0.784287 | 0.728442 | -0.055845 |
| `2510.05445` | `731b0a6b6981` | 0.862040 | 0.934627 | 0.797574 | 0.546060 | -0.251515 |
| `2501.07811` | `765715449d83` | 0.842469 | 0.786805 | 0.742331 | 0.491368 | -0.250963 |
| `2507.09177` | `791813a6fba7` | 0.728512 | 0.871606 | 0.493571 | 0.455117 | -0.038453 |
| `2505.03906` | `7a6e71be0782` | 0.881558 | 0.479508 | 0.854438 | 0.239889 | -0.614549 |
| `2405.00516` | `7e4adafe89e1` | 0.837282 | 0.865883 | 0.785616 | 0.539917 | -0.245700 |
| `2401.07128` | `837e71dc6d3c` | 0.788312 | 0.911148 | 0.737154 | 0.455900 | -0.281253 |
| `2502.01142` | `8a2f320a4d67` | 0.832726 | 0.877605 | 0.698766 | 0.415235 | -0.283532 |
| `2403.06095` | `8c22cbd987ff` | 0.832519 | 0.829035 | 0.720056 | 0.376375 | -0.343681 |
| `2601.13247` | `8dbe05fe1a03` | 0.738025 | 0.933486 | 0.669459 | 0.442417 | -0.227042 |
| `2512.12806` | `90898c44fbb7` | 0.868008 | 0.684468 | 0.790240 | 0.389644 | -0.400596 |
| `2509.10397` | `937a27c3cfb9` | 0.785153 | 0.916925 | 0.748582 | 0.540329 | -0.208253 |
| `2506.18019` | `96466540e7e4` | 0.706505 | 0.637890 | 0.505181 | 0.419074 | -0.086107 |
| `2503.13964` | `a097cc71a8b0` | 0.759631 | 0.608621 | 0.656983 | 0.335136 | -0.321847 |
| `2504.21716` | `a285d8bd4d6a` | 0.873792 | 0.496368 | 0.808405 | 0.278571 | -0.529833 |

The largest loss is `2505.03906`:5gram0.8544384346166083→0.23988948368157487, delta−0.6145489509350335; token0.8815580286168522→0.4795083382246314. `2410.13610` illustrates the order-vs-token conflict: token0.8897913025598848→0.9463529411764706, but5gram0.8384243546230588→0.43197439035872326.

These are measured losses in the selected-output diagnostic. One cause is directly visible in the worst outlier `2505.03906`: the saved TOC article (`14dee3fccd7edafb9036e7a9bf2e3559b2dfd82417f2f25554a67033945435f0`) has31sections totaling169870characters, versus42958GROBID section characters (`90238421d914495acd25b51b2177f9f039efdc32dd20e3a000eff37d1b6e14b6`). Thirty of31TOC sections belong to9exact-text duplicate groups; there are only10distinct section text payloads. For example, “Selected LLMs”, “Benchmark Design”, “Evaluation Metrics”, “Experimental Results” and “Experimental Setup” each repeat exactly the same5104character text withpage_start=page_end=4. Another5sections repeat a5813character payload onpage8. Each of the9duplicate groups shares its page interval. This artifact proves text-volume overcounting in this document; `body_chars` still awardsTOC1because it counts every repeated section. No content was deduplicated or rescaled in this review or frozen scoring.

Parent then supplied `validation-toc-exact-repetition.json`; its68successful TOC rows were independently checked against the saved articles. **24/68articles repeat an exact section-text payload**, adding771926section characters beyond one copy per distinct payload. Ten of those24repeat only empty payloads and add0characters; **14articles have positive extra duplicate characters**. For example, `2505.15471` has21sections but19distinct text payloads, while both total and unique-payload sums remain62508characters. The number is a diagnostic of repeated output, not a deduplicated score or proven rate of distinct PDF content loss; no scoring input changed. Of the31ordering regressions,20have any repeated exact payload and10have positive duplicate-character overhead. This broader observation and the outlier do not diagnose the causes of all31losses or prove a global reading-order failure. Additional grounded output/layout inspection would be needed for column swaps or other mechanisms. No parser or feature tuning is proposed during this freeze.

## 6. Metric disagreement

All28pairwise diagnostic rows are independently reconstructed. Winner disagreement uses **both-decisive intersection n68 or65**, not80; execution disagreement uses all80selected documents. Counts are unchanged across060/120/240.

| Metric pair | Different pair winners / both-decisive n | Different candidate executions /80 |
| --- | --- | --- |
| `body_chars` / `chars_per_ms` | 0/68 | 0/80 |
| `body_chars` / `legacy_yield` | 3/68 | 0/80 |
| `body_chars` / `ordered_5gram_f1` | 31/68 | 0/80 |
| `body_chars` / `quality_then_latency` | 31/68 | 0/80 |
| `body_chars` / `sections` | 51/65 | 67/80 |
| `body_chars` / `sections_per_ms` | 3/68 | 0/80 |
| `body_chars` / `token_f1` | 11/68 | 0/80 |
| `chars_per_ms` / `legacy_yield` | 3/68 | 0/80 |
| `chars_per_ms` / `ordered_5gram_f1` | 31/68 | 0/80 |
| `chars_per_ms` / `quality_then_latency` | 31/68 | 0/80 |
| `chars_per_ms` / `sections` | 51/65 | 67/80 |
| `chars_per_ms` / `sections_per_ms` | 3/68 | 0/80 |
| `chars_per_ms` / `token_f1` | 11/68 | 0/80 |
| `legacy_yield` / `ordered_5gram_f1` | 32/68 | 0/80 |
| `legacy_yield` / `quality_then_latency` | 32/68 | 0/80 |
| `legacy_yield` / `sections` | 50/65 | 67/80 |
| `legacy_yield` / `sections_per_ms` | 0/68 | 0/80 |
| `legacy_yield` / `token_f1` | 12/68 | 0/80 |
| `ordered_5gram_f1` / `quality_then_latency` | 0/68 | 0/80 |
| `ordered_5gram_f1` / `sections` | 32/65 | 67/80 |
| `ordered_5gram_f1` / `sections_per_ms` | 32/68 | 0/80 |
| `ordered_5gram_f1` / `token_f1` | 20/68 | 0/80 |
| `quality_then_latency` / `sections` | 32/65 | 67/80 |
| `quality_then_latency` / `sections_per_ms` | 32/68 | 0/80 |
| `quality_then_latency` / `token_f1` | 20/68 | 0/80 |
| `sections` / `sections_per_ms` | 50/65 | 67/80 |
| `sections` / `token_f1` | 48/65 | 67/80 |
| `sections_per_ms` / `token_f1` | 12/68 | 0/80 |

Important interpretations:

- body_chars wins and token F1 wins differ on11/68; body_chars and ordered5gram differ on31/68. Maximizing text volume does not establish either token correctness or ordered retention.
- token F1 and ordered5gram winners differ on20/68. They choose identical learned actions despite this disagreement because coarse-cell majority picks the same arm.
- ordered5gram and quality-then-latency have identical validation winners. No validation full pair lies within the frozen0.02quality margin, so the latency tiebreak is not exercised there. At120, one **training** document `2510.14545` has5gram difference0.013369745685025802: ordered chooses GROBID, quality-then-latency chooses TOC. At240,4training full pairs lie within the0.02margin, and the tiebreak changes2winners: `2510.14545` and `2503.08026` (difference0.005575735770481205), both GROBID→TOC. The other2 (`2410.14052`, `2501.13299`) already favorTOC. These change training rewards/gaps but not learned actions or validation choice.
- legacy_yield and exact sections_per_ms happen to agree on all validation winners. They are not the same formula: legacy uses integer floor(section_count×1000/max(ms,1)) then integer floor(body_chars×1000/max(ms,1)) as secondary tiebreak. This is not char-primary yield.

## 7. Coarse cells and forced-success support

| Exact cell | Train documents060→120→240 | Arm observations060→120→240 | Non-section learned route | Validation matches |
| --- | --- | --- | --- | --- |
| `p=1_49;b=0;q=ge700;d=_` |7→13→33|7→13→33|`xy_cut`→captured GROBID|12|
| `p=1_49;b=1;q=ge700;d=_` |50→100→195|100→200→390|`page_outline`→TOC|67|
| `p=50_199;b=1;q=ge700;d=_` |3→7→11|6→14→22|`page_outline`→TOC|0|
| `p=50_199;b=0;q=ge700;d=_` |0→0→1|0→0→1|No learned cell; below floor4|0|

`sections` selects `xy_cut` in all three cells. The support threshold4is **total arm observations**, not four independent PDFs or support per arm. A full-pair cell can qualify with2documents; the06050–199-page cell qualifies with3documents/6observations and is not tested by any validation document. The empty-TOC cell qualifies with7single-arm successes and no TOC comparison. All paired arms in060/120/240succeeded, so failures do not influence reviewed cells.

Ordering TOC win rate in the dominant positive-bookmark short-article cell is **38/50=0.76** at060, **64/100=0.64** at120and **120/195=0.6153846153846154** at240; GROBID rates are12/50=0.24,36/100=0.36and75/195=0.38461538461538464. The coarse cell still chooses TOC for all67validation members, including every31ordering regression. The50–199-page cell ordering win rates are3/3,5/7then8/11forTOC; no validation document tests it. Strong support is not per-document feature separation or quality guarantees.

All320inspected feature vectors have empty `domain` and high quality≥0.7. At06058/60train and77/80validation text qualities equal1.0; at240226/240training qualities equal1.0. Only page bands and presence/absence of a bookmark distinguish learned cells here. There is no evaluated evidence for low-text-quality/scan/nonempty-domain cells, or for the numerous different layouts inside the same short bookmarked high-quality bucket.

The existing lookup chain can drop domain and then bookmark presence; its last key uses `b=0`, the same string used by the measured empty-TOC cell. A future sparse positive-bookmark exact bucket could therefore reuse forced GROBID support without paired quality evidence. **That potential did not occur in060,120or240:** all79learned matches are exact; thebook falls through to native. Do not report the earlier review's potential fallback as an observed contributor to these31losses, or interpret coverage as paired comparative evidence in every covered record.

Support averages rise because the same67short/12empty-TOC matches consult enlarged cells: mean observation support85.87341772151899→171.59493670886076→335.7721518987342(n79), distinct-document support43.46835443037975→86.78481012658227→170.39240506329114(n79). They are averages of matched-cell counts repeated across validation documents, not totals of independent new validation evidence.

## 8. Cycle120 and cycle240 comparison

Frozen manifest/config/native-registry/metric/evaluator/shared-source IDs are identical. The training SHA list120contains the exact060prefix;240contains the exact120prefix and all240manifest training hashes. Validation SHA lists remain the same80with zero training intersection. Each validation source, capture hash, reference, measured feature vector, raw output/hash/score and pair winner is unchanged. All eight metrics have **zero new validation decision/execution/outcome changes** from060to120and120to240. Regression sets and raw deltas are the same;120diagnostic adds native/candidate reward and cell fields, so bytewise diagnostic rows differ without a semantic regression.

Tables retain the same three cell keys/actions, but support and seals change. Non-section120seal `4588aa94e4938a03891bcfa00047802781dffbed882398660fdbaaa56035e4be` includes13/200/14support. `sections` seal `bf57a2b547e378e7f740c439f4a25ce6ae71aa488df3d279ae071d28353c269a` also changes support only. Cycle240still has the same3learned cell keys/actions, support33/390/22and non-section seal `4a6e1a3eed590728fd7011cbf7e4f0353e761cfc7fa14eed95a49c0768c8d86f`; `sections` seal is `d362b718bbea63fd483c3a8afe61c5ed7eaebc29166165af77894e8b47793532`. Neither120nor240is a new draw of validation PDFs or an independent repeated speed/quality measurement. No statistical-stability or representative-feature-sufficiency conclusion follows from unchanged actions.

## 9. Baseline and result limits

1. **Native DATA counterfactual, not literal production dispatch.** Frozen native rules choose page_outline when pages≥50andbookmarks≥1, GROBID structural below0.4quality, otherwise xy_cut. On validation these yield79`xy_cut`and1`page_outline`. The frozen map aliases`xy_cut`and`grobid_structural`to the measured Java structural arm. No independent XY-Cut arm was executed. Native-only and candidate consult the same captured features, which makes the counterfactual comparison valid under this map. `source_cli.rs::run_parse_pdf` still consults/stamps grammar but directly invokes GROBID at2218–2231; the reported native TOC choice is not the literal current command's dispatch. No runtime default changed.
2. **Reference scope.** Candidate is concatenated `sections.text`; reference is full-PDF `pdftotext`. Header/abstract/citations/table fields outside sections are not scored. For an arm storing bibliography separately, loss of reference recall can reflect projection scope rather than missing bibliography. Token normalization keeps only ASCII alphabetic lowercase tokens; numbers/non-ASCII/math are not evaluated. Token F1 is a multiset measure, not order. Fivegram F1 is multiset overlap of local ordered windows, not global order, semantic accuracy, entity extraction, publisher truth or KG readiness.
3. **Captured time, not production capacity.**2398.1vs6018.05ms is one per-arm wall-clock sample including measured arm setup/export/probe/parse/write on a shared host with other work. It suggests different offline captured costs, but proves no controlled production speedup, batch throughput or failure behavior. Reusing the captures in120and240cannot reproduce timing or independence.
4. **Adaptive diagnostic corpus.** The320cohort is SHA-sorted/subsampled under6,000,000bytes andsource-family uniqueness, excluding historical IDs; it is not a representative random sample. The80validation set is repeatedly inspected and explicitly not an untouched final test. Lower training gap or unchanged validation actions is not statistical assurance, especially with one dominant cell.
5. **Failure/route scope.** All320GROBID arms and all274applicable TOC arms succeed. The other46TOC outcomes are measured empty-TOC NotApplicable. Candidate failures, selected NotApplicable, both-failed, malformed evidence and low-quality/unknown routes remain important contracts but are not empirically exercised by these outcomes. `unobserved_routes=[]`means the map covered these consulted routes, not every possible route/configuration has evidence.
6. **Metric adoption.** All8variants were predeclared with min_support4,5-token grams andmargin0.02; none was chosen after looking at validation. Reward1forbody/throughput does not mean perfectly retained text. No threshold, map, metric or support change should be mixed into these results; any later experiment must carry a new explicit recipe/identity.

## 10. Closeout attention and blockers

- **No blocker to frozen candidate-only continuation found.** No source/scoring defect, source-hash drift or denominator discrepancy appeared in this review. This is a read/independent-calculation result, not a cargo/test/architecture/commit/GSD-closeout verdict.
- **Blocking any broad quality/deploy claim:** persistent raw ordered5gram mean loss and31document regressions, adaptive rather than final test, narrow feature support and counterfactual baseline. Candidate-only locks must stay closed. A rank-reward increase must not be summarized as raw F1 improvement.
- **Documentation stale at this review:** `doc/review/policy-corpus-cycles.md` still states “no evaluated cycle yet” even though060/120/240outputs exist. It already states the scope limits correctly; parent should replace the status and add actual results/cross-cycle/regression findings before closeout. Reviewer does not edit it.
- **Cycle240 is now included and independently verified.** All three actual result files are present. This review verifies artifacts/aggregate arithmetic only; parent owns final architecture/UAT/commit/GSD gates. It does not claim those gates passed.
- **Review communication limitation:** explicit parent sends repeatedly returned pending-message-limit errors (208then249/250unfinished, limit20). Progress notes delivered; this file is the durable substantive report. No provider/config change is attempted to resolve the harness limit.

### Graph/source navigation

Graph first: GitNexus `list_repos`, named `context` and structural Cypher for evaluation/scoring/selection/table/native/fallback symbols. Bound repository`daily-archive`, external index`/root/.cache/gitnexus/m517-daily-archive-symbol`, symbol retention, indexed2026-10-13T11:07:18.025Zat718be28b59cd3e52baded2f1245a8b7dfee12b01. Graph capability is available; FTS is intentionally disabled. Status content drift is capture-process/new-result artifact updates, not a basis for changing the index. Some generic-name contexts (`evaluate`,`run`,`consult`) report lower-bound receiver/interface resolution; no absent caller is treated as proof of safety. Named source slices confirm relevant behavior; no edit requires impact analysis.

All assumptions are stated above. **No constants were changed.**
