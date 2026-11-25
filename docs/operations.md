# Operations guide

## Typical workflows

### Build a media from scratch

```bash
repocli --clean /srv/repo/media/main/release
```

Creates (inside `media_info/`):

- `hdlist.cz`
- `synthesis.hdlist.cz`
- `info.xml.lzma`, `files.xml.lzma`, `changelog.xml.lzma`
- `MD5SUM`
- versioned symlinks (`YYYYMMDD-HHMMSS-<name>`) if previous ones existed

### Merge new packages into an existing media

```bash
repocli --merge /srv/repo/media/main/release
```

The tool reads the existing `synthesis.hdlist.cz` and XML files, removes
obsolete entries listed in `old-metadata.lst`, and adds only the new packages.
Unchanged RPM headers are reused, so this is the fastest path for regular
updates.

### Incremental refresh

```bash
repocli /srv/repo/media/main/release
```

`old-rpms.lst` schedules removal of packages no longer present. Use
`--no-clean-old-rpms` to keep the RPM files while still updating metadata, or
`--only-clean-old-rpms` to perform only the cleanup.

### Handle broken packages

```bash
repocli --no-bad-rpm /srv/repo/media/main/release
```

Unreadable packages are reported on stderr and skipped instead of aborting the
whole run.

### Custom compression

```bash
repocli --synthesis-filter '.cz:gzip -1' \
        --xml-info-filter '.lzma:xz -1' \
        /srv/repo/media/main/release
```

The filter format is `suffix:command`. An empty command disables compression.
Defaults are `.cz:xz -1` and `.lzma:xz -1`.

## media_info directory

| File | Meaning |
|---|---|
| `hdlist.cz` | Packdrakeng container with one raw RPM header per package |
| `synthesis.hdlist.cz` | Textual dependency/summary index (compressed) |
| `info.xml.lzma` | Descriptions and source URLs |
| `files.xml.lzma` | File lists |
| `changelog.xml.lzma` | Changelogs |
| `MD5SUM` | Checksums of the published files |
| `old-rpms.lst` | `[Remove]` / `[Keep-in-hdlist]` schedules |
| `old-metadata.lst` | Obsolete metadata entries (merge mode) |
| `UPDATING` | Lock file; removed on exit |
| `*.tmp` | Temporary files removed by signal handler |

Versioned symlinks are created only when a previous versioned symlink existed
or when `--versioned true` is passed.

## Exit codes

| Code | Meaning |
|---:|---|
| 0 | Success |
| 1 | Fatal error (bad RPM without `--no-bad-rpm`, lock failure, I/O error) |

## Troubleshooting

| Symptom | Check |
|---|---|
| `no *.rpm found` | Add `--allow-empty-media` or point to the correct directory |
| `another repocli already running` | Remove stale `media_info/UPDATING` (only when no process is active) |
| `bad rpm <path>` | Run `cargo run --bin rpm_diag -- <path>`; use `--no-bad-rpm` to continue |
| urpmi does not see packages | Compare `hdlist.cz` with a reference build; verify the trailer with a hex dump (`cz[0` … `0]cz`) |
| Missing Version/Group/Size in `urpmq -ai` | Check `synthesis.hdlist.cz` — `@info@` must be the last field and carry `nvra@epoch@size@group` |
| Slow run | `REPOCLI_TIMING=1` to locate the phase; see [development.md](development.md) |

## Performance expectations

Measured on x86_64 with 97 packages: about 0.18–0.30 s depending on the output
filter configuration; roughly 325–550 packages/s. Extrapolated to 5000 packages:
9–15 s. `sys` time should be small; large `sys` time indicates a regression in
file handling.
