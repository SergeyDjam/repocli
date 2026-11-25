# On-disk formats

## RPM package (input)

### Lead

The first 96 bytes:

| Offset | Size | Meaning |
|---:|---:|---|
| 0 | 4 | magic `ed ab ee db` |
| 4 | 1 | major version (frozen at 3 even for rpm6) |
| 5 | 1 | minor version |
| 78 | 2 | signature type (big-endian) |

### Header

Both the signature header and the main header use the same structure:

| Offset | Size | Meaning |
|---:|---:|---|
| 0 | 4 | magic `8e ad e8 01` |
| 4 | 4 | reserved |
| 8 | 4 | index count, big-endian `u32` |
| 12 | 4 | data store size, big-endian `u32` |
| 16 | 16 × n | index entries: tag, type, offset, count (all big-endian `u32`) |
| … | data_size | data store |

Supported data types: `4` INT32, `6` STRING, `7` BIN, `8` STRING_ARRAY,
`9` I18NSTRING. All strings are NUL-terminated.

The signature header starts at offset 96 (when present); the main header follows
the signature header. Some packages insert a few bytes of padding between them,
so `detect()` scans structurally instead of assuming adjacency.

### Tags used by repocli

| Tag | Name |
|---:|---|
| 1000–1005 | NAME, VERSION, RELEASE, EPOCH, SUMMARY, DESCRIPTION |
| 1009 | SIZE (→ `@filesize@` and `@info@`) |
| 1014, 1016, 1020, 1022 | LICENSE, GROUP, URL, ARCH |
| 1044 | SOURCERPM |
| 1047 / 1112 / 1113 | PROVIDE name / flags / version |
| 1048 / 1049 / 1050 | REQUIRE flags / name / version |
| 1053 / 1054 / 1055 | CONFLICT flags / name / version |
| 1090 / 1092 / 1091 | OBSOLETE name / flags / version |
| 1064 | **RPMVERSION** — authoritative format detection |
| 1080–1082 | CHANGELOG time / name / text |
| 1116–1118 | DIRINDEXES / BASENAMES / DIRNAMES |
| 1125 | PAYLOADCOMPRESSOR (gzip / xz / zstd, informational only) |

## hdlist.cz — MDV::Packdrakeng 1.14

Layout produced by `packdrakeng::write_archive()`:

```
[ gzip block: concatenation of all virtual-file payloads ]
[ TOC strings: one name per line ]
[ one 16-byte record per file ]
[ 64-byte trailer ]
```

Each RPM is added as a virtual file of type `f`: the name is `fullname`
(`name-version-release.arch`) and the payload is the raw main header.

TOC records (`coff, csize, off, size`) are big-endian `u32` values. In our
single-block archive `coff` is always 0 and `csize` is the compressed block size.

Trailer:

| Offset | Size | Value |
|---:|---:|---|
| 0 | 4 | `cz[0` |
| 4 | 4 | directory count (0) |
| 8 | 4 | symlink count (0) |
| 12 | 4 | file count |
| 16 | 4 | TOC string length |
| 20 | 40 | decompressor command, NUL-padded (`gzip -d`) |
| 60 | 4 | `0]cz` |

`URPM.xs::open_archive()` checks this trailer, forks the decompressor named in
the command field, and feeds the resulting stream of concatenated headers to
`rpmgi`. URPM does not use the TOC when reading hdlist; the TOC keeps the file
compatible with MDV::Packdrakeng tools.

## synthesis.hdlist.cz

Encoded as `URPM.xs::Pkg_build_info()` from URPM 4.62. One block per package,
fields in this exact order:

```
@provides@<joined by @>\n
@conflicts@<joined by @>\n
@obsoletes@<joined by @>\n
@requires@<joined by @>\n
@summary@<summary>\n
@filesize@<size>\n
@info@<nvra>@<epoch>@<size>@<group>\n
```

Dependency entries follow `print_list_entry()`: `name[op evr]`, where
`op = Fstr[(flags >> 1) & 7]` with `Fstr = ["?0", "<", ">", "?3", "==", "<=", ">=", "?7"]`.
`RPMSENSE_RPMLIB` (bit 24) entries are skipped; `RPMSENSE_FIND_REQUIRES` (bit 14)
entries are kept. The `@info@` line is always last.

The whole file is passed through the configured compression filter:
`--synthesis-filter suffix:command` (default `.cz:xz -1`).

## XML metadata

Root element `<media_info>`, one element per package:

- `<files fn="fullname">` — one file name per line;
- `<info fn="fullname" sourcerpm="…" url="…" license="…">` — description;
- `<changelogs fn="fullname">` — `<log time="…">` with `log_name` / `log_text`.

Each XML file is compressed with `--xml-info-filter` (default `.lzma:xz -1`).

## Auxiliary files

| File | Purpose |
|---|---|
| `old-rpms.lst` | INI-like list with `[Remove]` and `[Keep-in-hdlist]` sections |
| `old-metadata.lst` | obsolete metadata entries for merge mode |
| `UPDATING` | global `flock` lock |
| `MD5SUM` | checksums of published metadata |
| `YYYYMMDD-HHMMSS-<name>` | versioned symlinks to current metadata |
