# Prior /tmp runbook (preserved, not modified)

`prior_runbook_repair2.tmp.sh` is a byte-identical copy of
`/tmp/s05_t01_repair2_runbook.sh` (the file found on disk when this cleanup
started). It is preserved here because `/tmp` is ephemeral and because the
cleanup brief forbids manufacturing a retroactive capture script.

It is **not** the script that produced `artifacts/parse_compat/s05_t01_repair2_run/`:
it writes its logs to `artifacts/parse_compat/logs`, and that directory does not
exist. The prior run's actual capture script was never saved, so the prior
directory has no script, no per-step argv and no timing metadata. The current
run therefore uses `capture.sh` in this directory, hashed at run time.

The file is kept as-is: no edit, no re-run, no retroactive reconstruction.
