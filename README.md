# repocli

Rust replacement for `genhdlist2` — the tool that generates repository metadata
(`hdlist.cz`, `synthesis.hdlist.cz`, XML info) for urpmi-based distributions
(ROSA, ALT Linux, Mageia).

The tool reads RPM packages directly: RPM headers are parsed in pure Rust with
no calls to `genhdlist2`, `librpm`, or the system `rpm` utility.

## Features

- **RPM format support:** rpm3, rpm4, **rpm5** (primary target), rpm6 via a
  plugin registry. Format is detected from `RPMTAG_RPMVERSION` (tag 1064), not
  from the frozen 3.0 lead.
- **`hdlist.cz`:** written in the MDV::Packdrakeng 1.14 container format —
  a gzip block of concatenated RPM headers plus a TOC and the `cz[0 … 0]cz`
  trailer. Verified to be readable by `urpmq`.
- **`synthesis.hdlist.cz`:** field order and encoding match
  `URPM.xs::Pkg_build_info()` from URPM 4.62 (`@provides@`, `@requires@`,
  `@filesize@`, `@info@`, and so on).
- **XML metadata:** `info.xml`, `files.xml`, `changelog.xml` with the same
  structure as the reference implementation.
- **Incremental updates:** existing `hdlist.cz` is reused via
  `old-rpms.lst` / `old-metadata.lst`; merge mode combines new and existing
  metadata and removes duplicates.
- **Repository housekeeping:** `MD5SUM`, versioned symlinks, global `UPDATING`
  lock, `--no-bad-rpm` error handling.
- **Performance:** ~550 packages/s on x86_64; about 9 s extrapolated for 5000
  packages (target: ≤15 s).

## Build and test

```bash
cargo fmt --all
cargo build --release
cargo test --release
```

Cross-compilation CI is supported: the `--help` execution test is skipped when
the Cargo host and target triples differ; the binary-existence check still runs.

## Usage

```bash
# Generate metadata for a repository
repocli /path/to/rpms

# Merge new metadata with an existing media_info/
repocli --merge /path/to/rpms

# Rebuild from scratch without XML or MD5SUM
repocli --clean --xml-info false --no-md5sum /path/to/rpms
```

Main options:

| Option | Meaning |
|---|---|
| `--clean` | Ignore the existing hdlist, rebuild from scratch |
| `--merge` | Merge new metadata with the existing metadata |
| `--no-bad-rpm` | Skip unreadable packages instead of aborting |
| `--no-md5sum` | Do not generate `MD5SUM` |
| `--no-hdlist` | Do not generate `hdlist.cz` |
| `--allow-empty-media` | Allow a directory without `.rpm` files |
| `--file-deps <file>` | Mark file provides required by other media |
| `--synthesis-filter <suffix:cmd>` | Synthesis compression filter (default `.cz:xz -1`) |
| `--xml-info <auto\|true\|false>` | XML generation mode (default `auto`) |
| `--xml-info-filter <suffix:cmd>` | XML compression filter (default `.lzma:xz -1`) |
| `--versioned <auto\|true\|false>` | Versioned symlinks (default `auto`) |
| `--media_info-dir <dir>` | Output directory (default `<rpms>/media_info`) |
| `-q`, `-v` | Quiet / verbose (repeatable) |

## Project layout

```
src/
  main.rs              CLI and orchestration
  rpm/
    mod.rs             RpmReader trait + plugin registry
    detect.rs          format detection via RPMTAG_RPMVERSION
    header.rs          safe tag-based header decoder
    universal.rs       shared metadata/raw_header reader
    rpm3.rs … rpm6.rs  format plugins (thin adapters)
  packdrakeng.rs       hdlist.cz container writer/reader
  synthesis.rs         synthesis.hdlist writer
  xml_metadata.rs      info/files/changelog XML writers
  incremental.rs, merge.rs, old_rpms.rs, old_metadata.rs,
  compression.rs, hdlist.rs, lock.rs, md5sum.rs,
  utils.rs, versioning.rs
tests/
  fixtures/            real RPM packages (rpm4/rpm5/rpm6)
  integration.rs       CLI smoke tests
```

## Reference sources

The implementation follows the authoritative ROSA/urpmi sources, not the
experimental Go prototype:

- `URPM-4.62/URPM.xs` — `open_archive()`, `Pkg_build_info()`, `pack_list()`,
  `print_list_entry()`, `pack_header()`, `build_header()`
- `urpmi-7.32.3` — reference genhdlist2 invocation and XML format
- `urpm-tools-2.3.5`, spec and patch files in the parent directory
- MDV::Packdrakeng 1.14 container format

See also `TZ_repocli-migration.md`, `CHANGELOG.md`,
`SUMMARY_genhdlist2-migration.md` and `SUMMARY_repocli-work.md`.

## License

GPLv3
