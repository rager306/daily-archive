# M517: восстановление GitNexus без удаления старого индекса

Read-only проверка 2026-10-13T10:57:27.330322+00:00. GitNexus 1.6.12, Node v24.21.0. `analyze`, `clean`, cargo и записи в индекс не запускались. Единственная запись — этот отчёт вне репозитория.

## Вывод

Поддерживаемый путь: построить **новый внешний индекс** с `--skip-fts`; для запаса по диску использовать `GITNEXUS_CONTENT_RETENTION=symbol`. Это оставляет `/root/daily-archive/.gitnexus` на месте, не повышает лимит 16 GiB и сохраняет все структурные символы, связи и процессы. `detect-changes` не использует FTS или сохранённый текст, поэтому эти настройки не делают его результат частичным.

Не повторять обычный incremental analyze поверх текущего файла. На момент проверки метаданные содержали `incrementalInProgress.phase=load-graph`, `toWriteCount=434`, `effectiveWriteCount=992`, `deleteCount=992`. Значит, последний сбой произошёл при записи графа, а не только при построении FTS. Старый файл сохранён физически, но уже **не является доказанно полным индексом**. Родитель дополнительно подтвердил, что он повреждён и не открывается; это не просто stale index. Восстановить его состояние до сбоя без отдельной резервной копии невозможно. Внешний fresh slot не открывает этот старый lbug и не использует его как базу.

## Установленные факты

- Runner `.gitnexus/run.cjs` выбирает глобальный `/usr/bin/gitnexus`, то есть `/usr/lib/node_modules/gitnexus/dist/cli/index.js`. Это не скрытый запуск `npx @latest`.
- `/root/daily-archive/.gitnexus/lbug`: 12,553,383,936 байт = **11.691 GiB** логически, 11.683 GiB выделено на диске. Дополнительно parse-cache ~119.6 MiB, parsedfile-cache ~299.4 MiB, CSV ~64.7 MiB; есть shadow/checkpoint sidecars. Их не трогал.
- Последний завершённый stamp — `2026-10-13T10:00:26.639Z`, HEAD `718be28b…`, 23,316 файлов / 109,200 узлов / 189,261 связей / 515 процессов. Он записан до незавершённого `load-graph`.
- Профиль старого индекса `full`; FTS уже `unavailable`, `skipReason=build-failed`; embeddings=0.
- `.gitnexusrc` и `.gitnexusignore` в корне отсутствуют. Root `.gitignore` не исключает M517 raw captures.
- Свободно **26.18 GiB** на том же разделе при последней проверке; родитель сообщает ~26 GiB. Это меняется во время сборки/capture.
- Существующие индексируемые файлы занимают ~376.2 MiB исходного текста; `data/processed/` даёт ~264.0 MiB. Поэтому исключение только новых M517 captures не объясняет и не устраняет уже раздутый 11.7 GiB файл само по себе.

## Точная команда восстановления (оператор/родитель; здесь НЕ выполнена)

Из `/root/daily-archive`, после завершения Rust edits, финального fmt и согласованной фиксации текущего diff:

```bash
GITNEXUS_STORAGE_PATH=/root/.cache/gitnexus/m517-daily-archive-symbol GITNEXUS_CONTENT_RETENTION=symbol node .gitnexus/run.cjs analyze --index-only --skip-fts --force
```

Условия:

1. Выбрать **отсутствующий либо пустой** полный storage directory. Указанный путь проверен как предложение, индекс в нём не создавался. Не копировать туда старый `lbug` или его metadata: внешний slot имеет проверку ownership и `storagePath`.
2. Не запускать второй analyze одновременно. Запускать detached/background; сохранять полный stdout, stderr и exit code, проверять диск через обычный `df`, не обрезать результат пайпом.
3. `--force` гарантирует fresh full graph/DB, а не накопление на старом файле. На Linux full build создаётся в уникальном `.staging.<uuid>` и публикуется атомарно. Внешний storage и его staging лежат на том же диске, **старый .gitnexus не удаляется и не заменяется**.
4. После успешного analyze GitNexus **перерегистрирует текущий repository path** в `~/.gitnexus/registry.json` на новый storage path. Это штатная запись engine, не ручная правка registry. Не создавать вторую запись с `--name`, это не нужно.
5. На будущих analyze использовать тот же `GITNEXUS_CONTENT_RETENTION=symbol` и `--skip-fts` (или env `GITNEXUS_SKIP_FTS=1`). Без retention env по умолчанию вновь выбирается `full`, и смена профиля вызывает rebuild.

Минимальный вариант без смены retention:

```bash
GITNEXUS_STORAGE_PATH=/root/.cache/gitnexus/m517-daily-archive-full node .gitnexus/run.cjs analyze --index-only --skip-fts --force
```

Он сохраняет полный source text и старый индекс; поддерживается тем же кодом. Но первый вариант меньше по диску: `symbol` не пишет содержимое File nodes, при этом оставляет snippets символов. Если цель — только complete graph review перед commit, полный текст файла в DB не нужен.

## Дополнительное узкое исключение raw captures

`.gitnexusignore` официально поддерживается README и общей scanner ignore service. Если capture продолжает писать raw files во время индексирования, в **новый root file `/root/daily-archive/.gitnexusignore`** можно записать:

```gitignore
# Candidate parser payloads are evidence, not source code.
/artifacts/learning/policy-corpus-m517/captures/
/artifacts/learning/policy-corpus-m517/preflight-captures/
```

Это необязательное дополнение, а не замена fresh rebuild. **В уточнённом scope родитель запретил source/config edits, поэтому сейчас этот файл не создавать: основной внешний symbol + skip-fts путь не требует ни одного source/config edit.** Не исключать весь `artifacts/`, `data/`, manifest/config/report или `crates/`.

Последствия:

- Исключаются только дерево per-document candidate payloads и сохранённый невалидный preflight. Файлы не удаляются, не перестают tracked/trackable в git, не меняются production stores или freeze manifests.
- Graph теряет File/Folder nodes для JSON/TXT/stdout/stderr этих деревьев; Rust functions/imports/call graph остаются в индексе. Корневые `manifest.json`, recipe provenance, summary/report, `historical/`, `data/policies/` и examples остаются видимыми.
- Из измеренного preflight исключается ~105.6 MiB / 770 raw файлов; часть превышает 512 KiB и уже не сканируется. Текущие authoritative captures растут, поэтому окончательная экономия не измерена.
- Это также убирает динамическую freshness-помеху от raw writes. Если другие индексируемые manifest/report ещё меняются, final analyze всё равно надо делать после их финальной записи.
- Проверил предлагаемые patterns через **установленный GitNexus dependency `ignore@7.0.9`**, без создания файла: 2 raw пути ignored=true, manifest/historical pair/source example/policy YAML ignored=false, exit 0.
- `.gitnexusrc` НЕ поддерживает ключи `storagePath`, `contentRetention`, `skipFts` или `exclude`. Для них использовать указанные env/CLI и `.gitnexusignore`; неизвестные rc keys отвергаются.

## Полный detect-changes перед commit

Новый код сейчас untracked. В 1.6.12 `scope=all` выполняет `git diff HEAD -U0`; **untracked файлы не входят**. Поэтому после последнего fmt и финальных artifacts надо выполнить обычный согласованный `git add -A` (не commit), затем final analyze и проверки:

```bash
GITNEXUS_STORAGE_PATH=/root/.cache/gitnexus/m517-daily-archive-symbol node .gitnexus/run.cjs status --json
node .gitnexus/run.cjs detect-changes --scope all --repo .
```

Критерии:

- analyze exit=0; новый `gitnexus.json` `contentRetention=symbol`, `capabilities.fts.skipReason=disabled-by-flag`, нет `incrementalInProgress`;
- `status --json` **без `--repo`**: `status=up-to-date`, `index.incompleteReasons=[]`, runner identity current, contentDrift current. Вариант `status --repo .` — registry report, не working-tree freshness proof; с symbol он отдельно отмечает sourceUnavailable, хотя graph queryable.
- detect exit=0, нет error, `partial` или `truncated` caveat. Changed count должен быть ненулевым и включать source symbols новых examples/tests, а не только старую application code.
- Не задавать `--limit`: это дополнительно режет listing. Backend жёстко ограничивает listing 1000 symbols; CLI `--limit 0` этот предел НЕ снимает. CLI formatter показывает лишь 15 строк даже при полном backend, поэтому `... and N more` сам по себе не равен `truncated`. Важно отсутствие специального truncation caveat и фактический changed_count <=1000.
- Если >1000 symbols, supported global-limit override/pagination у `detect_changes` нет. Не обходить его vendor patch: потребуются отдельные небольшие проверяемые diff/commit части. Текущие 13 modified source/doc files и несколько новых examples/tests вряд ли достигают предела, но это до проверки только ожидание.
- Повторить после любых последующих source edits. FTS не участвует: backend мапит diff ranges по `filePath/startLine/endLine`, затем query `STEP_IN_PROCESS`; ни поиск по body text, ни BM25 не вызываются.

## Диск и ограничения оценки

Новый graph-only keyword-search-disabled DB не наследует свободные pages/старые FTS tables 11.7 GiB файла. Retention symbol убирает ~376 MiB полного file-text surface (symbol snippets остаются), skip-fts убирает keyword tables. Есть ещё parser caches (~419 MiB в старом slot) и CSV staging. Поэтому отдельный compact index выглядит **реалистичным при ~28 GiB free**, а старый индекс копировать не требуется.

Новый окончательный размер и пик диска **не измерены**: я не запускал analyze. Даже default cap16GiB — ceiling одного файла, а не общий disk budget; если новый файл неожиданно вырос бы до cap, это оставило бы лишь ~11.5 GiB, плюс расход sidecars/CSV. Если у родителя floor15GiB, остановить background job до нарушения floor, сохранить stdout и не удалять live старый индекс. Фиксированная оценка «будет N GiB» была бы необоснованной.

Альтернативы, которые НЕ рекомендую:

- `GITNEXUS_LBUG_MAX_DB_SIZE=34359738368`: официально поддерживается, но это разрешает рост до32GiB без compaction, не решает dirty write-state и при данном disk budget опасно. Не изменял default16GiB.
- `--repair-fts`: dirty index его отвергает; текущий сбой помечен load-graph.
- Один `--skip-fts` на incremental поверх старого файла: не compaction. Dirty marker штатно force-rebuild, quarantine может двигать старые sidecars, успешный publish заменит старый lbug. Это не строгая сохранность старого directory.
- `clean`, удаление sidecars, ручная правка metadata/registry, vendor patch: не нужны.
- Глобальное `GITNEXUS_MAX_FILE_SIZE` снижение или broad exclude `data/processed/`: сокращает покрытие сверх задания. Узкое M517 exclude допустимо, но не надо скрывать source code ради прохождения gate.

## Проверенные ссылки на installed source

Все пути ниже под `/usr/lib/node_modules/gitnexus/`:

- `README.md:680–687`: external storage, retention, maxDBSize16GiB.
- `README.md:734–746`, `dist/config/ignore-service.js:354–405`: `.gitnexusignore`, gitignore syntax, precedence.
- `dist/cli/index.js:47–76`: `--force`, `--skip-fts`, `--index-only`; `:312–320` detect CLI flags.
- `dist/core/run-analyze.js:942–1012`: dirty-state full recovery и quarantine sidecars.
- `dist/core/run-analyze.js:1649–1778`, `:3433–3576`: atomic full DB staging, publish, failure cleanup.
- `dist/core/content-retention.js:43–75`, `dist/core/lbug/csv-generator.js:258–291`: graph сохраняется, File text не пишется при symbol, snippets сохраняются.
- `dist/storage/storage-resolver.js:197–249`, `:497–507`: env path precedence; tool-side registered-storage resolution.
- `dist/storage/repo-manager.js:769–909`, `dist/core/run-analyze.js:3355–3363`: штатная registry update.
- `dist/mcp/local/local-backend.js:813–845`, `:4486–4815`: git diff scopes, structural mapping, partial/truncated semantics и cap1000.
- `dist/cli/tool.js:48–94`, `:290–308`: CLI exit1 при partial/error; `truncated` может иметь exit0.
- `dist/cli/detect-changes-format.js:14–21`, `:64–86`: специальные caveats и display-only first15.
- `dist/cli/analyze-config.js:52–113`: allowlisted rc keys, неизвестные keys fail-closed.

Допущения: новый storage path свободен; GitNexus default 512KiB source-size cap оставляется; current graph scope не pdg (старый checkpoint так и строился); commit ещё не сделан. Продуктовые constants, провайдеры и hard locks не менялись.
