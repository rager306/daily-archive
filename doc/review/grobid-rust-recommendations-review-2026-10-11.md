# Review: GROBID → Rust — направления закрытия native-пробелов

| Параметр | Значение |
| --- | --- |
| Объект review | Native PDF → scholarly fields/structure → emitted TEI; body alignment, модели, visuals, приёмка и learning |
| Проверенная ревизия | `75331d7727ec2213e59930cddb475a5380019f4e`, ветка `feat/da-parse`, после M518 |
| Граница датированных внешних выводов | 2026-10-11; это дата в имени файла, не дата запуска нового замера |
| Получение внешних материалов | 2026-10-14 по записям сервисов и дочерних отчётов; недатированные current docs не подтверждают состояние на 2026-10-11 |
| Метод | Пять независимых subagents, поисковый MCP Tavily/Exa, чтение первоисточников и локальных source/evidence |
| Статус | Исследовательский review; предложения не реализованы и не приняты как product thresholds |
| Вердикт | Рекомендуется поэтапная доработка существующего native-каскада. Полная готовность к замене Java не доказана |
| Полномочия | Документ не является GSD PLAN, новым GAR/horizon-регистром или разрешением на запуск, promotion, import либо graph writes |

## Вывод

Рекомендуется завершать существующий native-каскад по стадиям, а не переписывать GROBID или менять весь PDF-стек. Для текущих пробелов достаточно сначала проверить occurrence matcher, связать уже имеющиеся header/reference/visual heads с PDF-входом и оценить именно эмиссию. Новый PDF-движок, VLM и сложный learner должны появляться только после доказанного ограничения этого пути.

Выбраны пять направлений. Каждое исследовал отдельный subagent через поисковый MCP Tavily/Exa. Сохранены **50 поисковых вызовов — по 10 на направление**, первичные источники, извлечённые фрагменты, неудачные обращения и ограничения чтения. Корректность, худший расход ресурсов и объём интеграции сравнивались отдельно. Исследование не доказывает локальную производительность какого-либо варианта.

## Сводка рекомендаций

| Направление | Рекомендуемое изменение | Доказательство до принятия |
| --- | --- | --- |
| Body alignment | Независимый маленький oracle и один capped exact linear-space Myers candidate; occurrence multiplicity отдельно от order | Сравнение с greedy на retained inventories; optimality только при завершённом поиске, иначе NotAssessable |
| Header/reference | Подключить существующие CRF/feature/projector stages с единым pinned model–ABI–template–label association | Real-weight same-input decoder proof, независимая renderer ABI проверка, затем emitted fields/links |
| Visuals/tables/formulas | Сначала productive PDF-derived regions/markers в существующие family heads и lifter/emitter | Независимые object/caption/callout positives и false positives; внешний head только при доказанной недостающей capability |
| Приёмка и ресурсы | Расширить existing paired receipts independent semantic scorer и full-backend accounting | Same-input PDF → emission, независимый gold, family/order/link проверки, CPU/memory/failures без success-only отбора |
| Learning | Сохранить immutable table; challenger усложнять только после доказанного headroom | Paired constrained-oracle/predictability experiment, затем logistic против table на untouched holdout |

Body study и header compatibility/wiring могут идти параллельно после отдельного разрешения. Серьёзность здесь относится к границам доказательства native-кандидата, не к доказанному дефекту production Java. Исторические закрытия A-*, H05 и принятые H21 residual не переоткрываются.

## Границы источников и работы

Граница датированных внешних выводов — **2026-10-11**. Дочерние отчёты и ответы сервисов указывают получение 2026-10-14. Явно датированные публикации до границы отделены от текущих недатированных страниц. Последние не подтверждают состояние на 11 октября. Заголовок `Published` поискового сервиса не использовался как дата статьи, если он противоречил arXiv version history или странице выпуска.

Rust-код, ADR и старые ревью прочитаны как текущие локальные ограничения, не внешние «практики 2026». ADR-078/080 от 2026-10-12 не выдаются за опубликованную до границы внешнюю практику. В root-сессии discovery MCP отказал из-за устаревшей конфигурации; обычные MCP вызовы дочерних сессий работали. Обход через ключи или прямые API не применялся.

Работа — только исследование. Этот сводный review перенесён в `doc/review/`; дополнительные отчёты и raw receipts остались под `/tmp/grobid-rust-gap-audit/research-2026/` и не включены в репозиторий. Не запускались cargo, тесты проекта, Java, PDF parsing/inference или новый корпусный замер. Не менялись код, константы, thresholds, веса, словари, frozen M517, vendors, GSD-планы или индекс. На завершение исследования и перед оформлением review HEAD был `75331d7727ec2213e59930cddb475a5380019f4e`, дерево репозитория было чистым; перенос добавляет только этот документ и ссылку в review index. Действуют `deployed=false`, `import_eligible=false`, `auto_promote=forbidden`, `graph_write=forbidden`.

## 1. Body: точное сопоставление и доказательство вхождений

**Выбор: маленький независимый oracle + один ограниченный по работе exact linear-space Myers candidate.** Сначала исследовать сохранённые упорядоченные token inventories. Не предполагать, что reference TEI содержит надёжную page mapping. Алгоритм нужен не для «исправления PDF», а для проверки, какую долю неопределённости создаёт текущий matcher.

Статический контрпример уже известен: `[a,b,c]` против `[b,c,a]` даёт текущему greedy одно совпадение, хотя допустимы два. Это опровергает его заявление о максимуме, но не доказывает масштаб эффекта на корпусе.

Разделить три значения при неизменных scope/tokenization/normalization:

- `U = Σ min(count_oracle(t), count_native(t))`: количество совпадающих токенов с учётом кратности, без порядка.
- `L`: длина точного LCS, **только если поиск завершился**; оба индекса строго растут, вхождения не переиспользуются.
- `H`: корректное greedy/anchored сопоставление — нижняя оценка, `H ≤ L ≤ U`.

`U−L` показывает несовместимость совпадающего token inventory с объявленным монотонным порядком. Это не автоматически вина native: order/scope oracle тоже может отличаться. `U−H` дополнительно содержит промахи алгоритма. Даже точный LCS не устанавливает физическую идентичность повторяющихся вхождений; нужны независимые source ranges/геометрия. Split/merge токенов требует отдельного witness и принятой семантики, а не `contains`.

Myers в точном варианте имеет `O((n+m)D)` времени и линейную память реконструкции. Большой `D` оставляет квадратичный худший случай. Поэтому cap должен считать работу, включая equality comparisons, а не только время. Незавершённый поиск возвращает typed incomplete/NotAssessable, не «приближённый PASS». Полная DP-матрица 100k×100k требует около 40 GB только под 32-bit scores; Hirschberg экономит память, но не квадратичную работу. Sparse Hunt–Szymanski также может вырасти квадратично из-за повторов.

**Не брать default diff как oracle.** В `imara-diff 0.1.8` upstream прямо называет `Myers` и `Histogram` неминимальными; `MyersMinimal` отключает ранние эвристические остановки. Наличие пригодного deterministic work-cap API ещё нужно проверить по закреплённому исходнику. Это кандидат, не выбранная зависимость. Exact page/block DP становится разумнее только при независимой page/region mapping. Forced rare anchors дают условную точность внутри gaps, не глобальный максимум.

**Решающий будущий эксперимент:** сравнить greedy и exact candidate на сохранённых трёх PDF inventories, raw/admitted/XY-Cut, сохранив scope и показав `U/H/L`, completion, witness, CPU/RSS и причины cap. Малые примеры проверить независимым exhaustive oracle по индексным парам. Если `L` остаётся низким, greedy не объясняет весь результат. Если exact не укладывается в согласованный бюджет, сохранить неизвестность и исследовать одну меньшую область. Численные лимиты здесь не выбраны.

D511 остаётся в силе: изменение P1-семантики требует новой identity и отдельно разрешённой S03/S05 re-emission. Нельзя переписать старый NotAssessable новым баллом.

Источники: [Myers, 1986](https://doi.org/10.1007/BF01840446), [Hirschberg, 1975](https://www.ics.uci.edu/~dan/pubs/p341-hirschberg.pdf), [imara-diff 0.1.8](https://docs.rs/imara-diff/0.1.8/imara_diff/enum.Algorithm.html). Это зрелые основания, не новые алгоритмы 2026 года.

## 2. Header/reference: совместимость моделей и сборка стадий

**Выбор: существующий scalar CRF, features и projectors; сначала header, затем independently accepted reference stage.** Общий fulltext-блокер не нужен как prerequisite этой работы. Java остаётся production/fallback до принятия соответствующего native-кандидата.

Одна stage association должна связывать точные model bytes/digest с feature ABI, embedded template digest, ordered raw labels, tokenizer/normalizer и versioned semantic mapping. Не создавать второй конфигурационный источник для шаблонов. Generic decoder может оставаться generic, но vocabulary конкретной semantic stage должен быть закрытым.

Это особенно важно в 2026: [GROBID 0.9.0, 7 апреля](https://github.com/grobidOrg/grobid/releases/tag/0.9.0) обновил Wapiti до 1.5.1 и переобучил header/segmentation/fulltext. [0.9.1, 4 августа](https://github.com/grobidOrg/grobid/releases/tag/0.9.1) исправил mixed-engine/flavor выбор и добавил linking-aware `affiliation_linked`. Даты подтверждены release API, а не поисковым сниппетом. Это не просьба заменить исторический oracle: каждая серия фиксирует свой effective release, family digest и structural/rich request dialect. Новая sentence two-pointer re-alignment работает против того же canonical text и не решает независимое body alignment.

Конкретный неподходящий fulltext artifact с 2,066,170 observations против cap 2,000,000 и `<section_marker>` вместо `<item>` не лечится механическим повышением cap или переименованием label. Нужны доказанная совместимость и resource envelope именно выбранных bytes. Ни лимит, ни mapping здесь не меняются.

Ресурсная часть может быть проще без смены модели: immutable prepared model/lookup, разделяемые между jobs; у каждого job — свой bounded scratch. Текущий `map_input` действительно заново строит весь observation `BTreeMap` на sequence invocation (`scalar_crf.rs:336–347`). Устранение повторной подготовки — обоснованный кандидат оптимизации, но выигрыш не измерен. Budget должен учитывать resident models, preparation peak и concurrent features/decode/emission, не только размер файла.

Отсутствие optional date не должно автоматически уничтожать независимо подтверждённые title/authors **в новом отдельно принятом stage contract**. Frozen atom contract менять задним числом нельзя; product completeness может по-прежнему отклонять неполный документ. Upstream header использует optional normalized date, но это лишь основание рассмотреть такой контракт.

**Решающий будущий эксперимент:** одинаковые feature-row bytes и real weights подать pinned upstream Wapiti и Rust; сравнить labels и явно определённый final raw path score. Затем отдельно проверить renderer ABI на независимых layout inputs, затем поля/связи по независимому gold. Rust cumulative alpha scores нельзя приравнивать к Wapiti probabilities или всем per-label scores. Decoder parity не равна PDF quality.

Источники: два GROBID release выше, [Wapiti manual](https://wapiti.limsi.fr/manual.html), [HeaderParser@0.9.0](https://raw.githubusercontent.com/grobidOrg/grobid/0.9.0/grobid-core/src/main/java/org/grobid/core/engines/HeaderParser.java).

## 3. Visuals, tables, formulas: нужные входы, а не второй движок

**Выбор: pdf-inspector + source/lattice identities + уже существующие family heads/lifter/emitter.** Отсутствие Figure/Table/Formula/Callout в 12-PDF E2E прежде всего требует проверить productive PDF-derived regions, feature rows и marker inventory. Библиотечные decoders уже существуют; их fixture PASS не доказывает подключение к PDF-пути.

Различать object extent, caption span, printed label и target identity. Ближайший rectangle не достаточен при floats/двух колонках. Повторяющийся «Figure 1» не является unique ID. Пара region/callout должна иметь source-bound occurrence identity; ambiguous/no-target сохраняется как typed результат. Проверка, что target ID существует, не проверяет правильный объект назначения.

**Полезное датированное направление 2026:** [PaddleOCR-VL-1.5 / PP-DocLayoutV3, 29 января](https://arxiv.org/html/2601.21957v1) соединяет polygon/mask localization и reading-order head, затем распознаёт отдельные регионы. Это довод в пользу одного изолированного layout/geometry или cell/formula donor, если native geometry недостаточна. Его public score не подтверждает scientific TEI, citation identity или Rust runtime. Дата также подтверждена [официальным постом Baidu](https://ernie.baidu.com/blog/posts/paddleocr-vl-1.5/).

Для born-digital сохранять independently reliable native text. Layout detector может добавлять geometry/order без перегенерации текста VLM. Для image-only/смешанных областей OCR — отдельная capability; whole-page Mixed label не разрешает удалить native текст. Pixel buffers растут с area×DPI², pairwise order — с количеством регионов; это требует отдельного ограничения и измерения.

Локализация текстовой формулы и её номера не требует image-to-LaTeX. Formula locator, LaTeX transcription и equation-callout target — разные обещания. Cell metrics уместны только когда продукт обещает cells/spans, а не одну caption.

**Решающий будущий эксперимент:** на уже разрешённых same-input PDF с независимыми positives связать имеющиеся heads и проверить actual emitted objects, caption relation, IDs, inline targets и false positives. Если проблема в inputs/composition — остановиться здесь. Если доказана конкретная missing geometry/cell/image capability — добавить ровно один opt-in head через текущий candidate seam. Второй low-level PDF engine обсуждать лишь при доказанном decode/render inability, как требует D498.

Docling/MinerU/Paddle/olmOCR — offline comparators или условные candidate producers, не новый semantic authority. Rust wrapper над C++/Python/ONNX не равен pureRust; модельные права и runtime boundary требуют отдельного решения. MinerU2.5 и olmOCR2 здесь классифицированы как 2025 основания, несмотря на противоречащие даты retrieval wrappers.

## 4. Приёмка и ресурсы: проверять выход и полный backend

**Выбор: расширить существующие paired receipts, не строить новую benchmark platform.** До scoring закрепить required stage/classes, independent gold, denominators, matching/order policy, family floors/noninferiority margins и resource budget. Числа должны принять владельцы проекта; этот отчёт их не задаёт.

[OmniDocBench](https://github.com/opendatalab/OmniDocBench) в апреле 2026 изменил matching/evaluator: v1.6 вводит MGAM на prediction side, v1.7 — следующие обновления. Датированное сообщение «GT unchanged» относится к matching; release одновременно добавляет страницы/правит annotations. Поэтому pin нужны dataset bytes **и** evaluator revision. Общий `Overall` усредняет text/table/formula и **не включает reading order**. В current code область order отличается от оригинальной статьи. Нельзя переносить старую scope или current defaults в project acceptance без проверки.

Считать отдельно:

- document macro и occurrence/field micro;
- required-population coverage, включая known-gold omissions/refusals, и conditional scores;
- title/authors/affiliation links, complete references, citation→bibliography identity;
- body occurrence multiplicity, permitted order и paragraph/section structure;
- обязательные captions/visual targets и table/formula topology;
- crashes/timeouts/NotAssessable/exclusions с причинами.

Правильная XML-ссылка на неправильную bibliography entry должна проваливать independent scorer. Нужны negative controls: drop обязательного поля, duplicate/reorder occurrence, valid-but-wrong target. Существующие serializer guards оставить. Java TEI — comparator, не непогрешимый gold и никогда не native SUT input. Сам [GROBID evaluator](https://grobid.readthedocs.io/en/latest/End-to-end-evaluation/) признаёт ограничения publisher XML/body absolute evaluation.

H05 Java CPU уже закрыт. Недостаёт same-output/same-input paired comparison полного native и Java route. По [cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html) считать backend-tree CPU, включая JVM/pdfalto/helper processes; отдельно показывать process RSS и charged memory peak — это не одно значение. Cold prepare, warm parse, queue wait, failure cost, thread/concurrency allocation и scratch/filesystem policy должны совпадать по условиям. Scorer не входит в parser interval. Structural profile без network consolidation отличается от rich enrichment. 12 PDF не устанавливают надёжный p99.

OmniDocBench/MORE datasets описаны как research-only/noncommercial, несмотря на code license badges. Их можно изучать как методику, но download/adoption/права не разрешены этим исследованием. Ни один найденный benchmark не заменяет scholarly target/author relation oracle.

## 5. Learning: сначала доказать предсказуемый запас качества

**Выбор: сохранить immutable table и native baseline.** Сначала выяснить, есть ли среди уже admissible candidates quality-safe лучший вариант для части документов и можно ли предсказать его по доступным до решения features.

Минимальный experiment сравнивает текущие rules, existing table, один regularized logistic challenger и диагностический constrained oracle. Oracle выбирает лучший admissible outcome по независимому gold; он показывает headroom, но не deployable. Если headroom нет — улучшать стадии, не learner. Если headroom есть, но features не разделяют победителей — это не довод немедленно добавлять LLM. Shallow bounded GBDT получает смысл только после incremental holdout gain над logistic.

Парные offline outcomes всех admissible actions дают full-information evaluation. При single-arm deterministic logs IPS/DR не создают отсутствующий counterfactual support; [VW OPE](https://vowpalwabbit.org/docs/vowpal_wabbit/python/latest/tutorials/off_policy_evaluation.html) объясняет partial-feedback проблему. Production exploration не требуется и здесь запрещена. Router после исполнения всех кандидатов платит сумму route costs — это не экономный predictive routing.

Quality/order/conservation/cost остаются отдельными hard gates, не свободно компенсируемым reward. Runtime выдаёт только `candidate_id` из admitted input set. Unsupported/uncertain cases возвращают baseline, не самодельный TEI. Training/tuning/grouped splits и final holdout разделяются; pages/revisions/template families не независимые samples.

M517 заморожен: рост token F1 при снижении ordered 5-gram не является общим выигрышем. Не retune/dedup validation ради нового зелёного вывода. Для нового исследования нужна собственная versioned evaluation identity и untouched holdout.

ruVLLM → SONA → MicroLoRA/EWC остаётся законным независимым training/critic/ablation направлением через ports/adapters. Не делать его обязательным parser dependency. GEPA — prompt optimizer, не готовая parser policy; его selection score не independent test. [BAAR, 2026 preprint](https://arxiv.org/html/2602.21227v1) поддерживает разделение hard budgets и soft rewards, но не доказывает необходимость RL для конечного parser selector. Его результаты, как и [CARROT](https://arxiv.org/abs/2502.03261), на daily-archive не переносились.

## Порядок отдельно разрешаемой работы

1. Зафиксировать scope и independent scoring semantics. Малые exact oracle/negative controls — до большого замера.
2. **Параллельные независимые направления:** body matcher study на retained inventories; header real-weight/renderer proof и candidate wiring. Reference после своего stage proof; не ждать изменения fulltext cap.
3. Productive visual/caption/callout inputs в существующий каскад; внешний head только по измеренной missing capability.
4. Paired PDF→emission quality и full backend resources на допустимом diagnostic scope. После freeze — eligible holdout и failure/reliability evidence, не success-only подвыборка.
5. Selector headroom experiment как отдельная learning линия. Promotion/Java retirement — только затем и только отдельным human decision.

Ни одна строка этого порядка не является GSD PLAN или разрешением запускать работы. Stage/class acceptance не равна всему продукту; component PASS, public leaderboard и `GSD Complete` не снимают права import/graph/deployment.

## Предположения и неизменённые параметры

Предполагается, что retained token inventories/разрешённые PDF доступны для отдельно утверждённого experiment и можно получить независимые family oracles. Надёжная reference page mapping, compatible fulltext artifact, model rights, точные budgets и product thresholds **не установлены**. Если эти условия отсутствуют, соответствующий вывод остаётся NotAssessable/unsupported.

**Константы, пороги, models и runtime не изменены. Численные новые бюджеты не выбраны.** Предложения об optional fields, scorer и versioning требуют отдельного принятия. Более быстрый prepared lookup, exact-matcher recovery и меньшая CPU/RSS пока не измерены.

## Основания review и дополнительные материалы

### Связанные документы репозитория

- [Предыдущее техническое ревью и последующие исправления](grobid-absorption-audit-2026-09-21.md): читать датированные корректировки, а не только исходную RED-таблицу.
- [Review index и поправки H05/H21](README.md): H05 Java CPU уже закрыт; судьбы H21 residual приняты отдельно от продуктовой приёмки.
- [Исходное направление поэтапной замены](../research/m463-grobid-rust-ruvector-platform.md) и [ADR-075](../adr/ADR-075-scholarly-parse-plane-staged-platform.md): behavioral replacement по стадии/классу, не построчный перенос Java.
- [M516 readiness](M516-S12-release-readiness.md): quality/cost/rights и human decision не заменяются исследовательской рекомендацией.
- [Scalar CRF proof scope](../parse/scalar-crf.md) и [native visual families](../parse/native-visual-families.md): существующие компоненты и границы их доказательства.
- Текущие source references: [body matcher](../../crates/da-parse/src/native_body_text.rs), [native E2E composition](../../crates/da-cli/examples/native_route_e2e.rs), [CRF input mapping](../../crates/da-domain/src/scalar_crf.rs).

### Локальные материалы исходного исследования

Следующие файлы находятся в `/tmp/grobid-rust-gap-audit/research-2026/`, **не перенесены вместе с review и не являются постоянными tracked evidence**. Внешние первоисточники приведены непосредственно в соответствующих разделах выше; повторного исполнения или замера при переносе не было.

| Локальный файл | Содержимое |
| --- | --- |
| `body-alignment.md` | Алгоритмы, оптимальность, caps, independent tiny oracle |
| `native-models.md` | Pinning, stage assembly, Wapiti/renderer/semantic proof |
| `pdf-structure.md` | Existing heads, geometry/IDs, условные visual donors |
| `eval-resources.md` | Independent semantic scoring, denominators, paired backend accounting |
| `learning-policy.md` | Table/logistic/GBDT/bandit/RL сравнение и headroom experiment |
| `source-claim-map.json` | Register claims, даты, passages и ограничения |
| `research-receipt.json` | MCP counts, hashes исходного summary/reports/evidence, workers, HEAD и bindings локальных inputs до оформления review |

Исходный `recommendations-summary.md` перенесён в этот review. Его hash в историческом `research-receipt.json` относится к тексту **до оформления**, не к текущему файлу; старый receipt не переписывался. Дополнительные материалы предыдущего анализа: `/tmp/grobid-rust-gap-audit/reanalysis/reanalysis-summary.md` и `reanalysis-receipt.json`.

На завершение исходного исследования все пять subagents завершены; git-read checks дали exit 0, HEAD unchanged и пустой porcelain output. Bound documents/source/frozen artifacts были сравнены по bytes/SHA-256 без изменений. Это историческая проверка до переноса, не утверждение о чистом дереве после добавления review. Неудачные/обрезанные external reads сохранены локально и не трактовались как доказательство отсутствия capability.
