# Python-era cold archive index

Latest pack: `python-era-cold-20260804T043526Z/`

| What | Where now |
|------|-----------|
| Frozen Python stack | `legacy.tar.zst` |
| Milestone artifacts | `artifacts__*.tar.zst` |
| GSD exec evidence | `gsd__exec.tar.zst` |
| state-manifest 43MB | `gsd__state-manifest.json.tar.zst` |
| Old gsd.db backups | `gsd__gsd.db.backup-v*.tar.zst` |

Live GSD DB + phases **not** archived (Tier 0+1 only).

Restore example:

```bash
zstd -dc archive/python-era-cold-20260804T043526Z/legacy.tar.zst | tar -xvf -
```
