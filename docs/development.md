# Development guide

## Prerequisites

- Rust toolchain (edition 2021; CI uses 1.95+).
- Standard build tools; `xz` is used as an external compression filter.
- For integration checks against urpmi: `urpmq` and an urpmi media registered
  under the name used in the examples.

## Everyday commands

```bash
cargo fmt --all
cargo build --release
cargo test --release
cargo clippy --all-targets -- -D warnings
```

Integration tests that spawn the built binary are skipped automatically when
`HOST != TARGET` (cross-compilation). The binary-existence test always runs.

## Coding rules

1. **No Cyrillic in source code**, including comments. Chat and Markdown docs
   are Russian. CI check: `grep -P '[\x{0400}-\x{04FF}]' src/**/*.rs`.
2. Clippy is run with `-D warnings`. Use targeted `#[allow(...)]` only when the
   alternative would change a reference-compatible API (for example the
   nine-argument `run()` functions inherited from genhdlist2).
3. Every change must be verified with a real `cargo build --release`; do not
   paste uncompiled code into documentation or chat.
4. Add a `CHANGELOG.md` entry for every behavioural change, including measured
   performance numbers.
5. No calls to `genhdlist2`, `librpm`, or the `rpm` binary in the runtime path.
   Rust readers are mandatory; external `xz` is a configurable filter.

## Adding a new RPM format plugin

1. Create `src/rpm/rpmN.rs` implementing `RpmReader`:

   ```rust
   pub struct RpmNReader;

   impl RpmReader for RpmNReader {
       fn format(&self) -> RpmFormat { RpmFormat::RpmN }
       fn read(&self, path: &Path) -> Result<RpmInfo> {
           universal::read_file(path) // or version-specific logic
       }
   }
   ```

2. Add the variant to `RpmFormat` in `detect.rs` and extend `classify()` to map
   its `RPMVERSION` major number to it.
3. Register the plugin in `RpmReaderRegistry::default()`.
4. Add a real package to `tests/fixtures/` and extend `reads_all_fixture_formats`.

No other module needs changing: `main.rs` dispatches exclusively through the
registry.

## Working with formats

- `src/rpm/header.rs` is the only place that interprets the binary header.
  Keep it bounds-checked; never index a slice with an unvalidated tag offset.
- `universal.rs` owns the tag-to-field mapping. When adding a tag, define a
  named constant with the upstream RPM name.
- Output layouts must follow the authoritative sources described in
  [formats.md](formats.md). Do not invent new `@field@` names in synthesis:
  unknown names break urpmi.
- The xml writer uses `quick_xml`; each file entry ends with `\n`, and the
  `<info>` element carries `sourcerpm`, `url`, `license` attributes.

## Diagnostics

| Tool | Purpose |
|---|---|
| `REPOCLI_TIMING=1 repocli …` | prints phase timings: `metadata`, `pipeline`, `finalize`, `merge:synthesis`, `merge:hdlist`, `merge:xml` |
| `cargo run --bin rpm_diag -- <rpm>` | dumps lead, signature and header index entries |
| `cargo run --bin rpm_parse_correct -- <rpm>` | heuristic parser reference (legacy) |
| `cargo run --bin compare_headers -- <rpm>` | compares reference and raw headers |
| `tests/fixtures/*.rpm` | real rpm4/rpm5/rpm6 packages for regression tests |

## Performance workflow

Profile before optimizing; the observed bottlenecks in this project were:

1. A header scanner that walked the whole RPM byte-by-byte — fixed by stopping
   after the signature and main headers (6,838 s → 0,749 s on 97 packages).
2. External `xz -7 -T0` on synthesis and XML — changed to `xz -1`
   (0,749 s → 0,323 s).
3. Per-header gzip at the default level — changed to level 1
   (0,323 s → 0,176 s).

Do not reintroduce `rayon` or threaded compressors without a `strace -c` profile:
the previous implementation spent 99.7% of its time in `futex`.

## Verification against urpmi

After changing output formats:

```bash
repocli /path/to/rpms/media
urpmq -f --media <media> --list
urpmq -f --media <media> -ai <package>
```

The first command must list every package; the second must show correct
Version, Group, Size and Summary. This validates hdlist, synthesis and XML at
the same time.

## References

See the parent directory for the authoritative sources: `URPM-4.62`,
`urpmi-7.32.3`, `urpm-tools-2.3.5`, spec and patch files. The Go prototype
`go-genhdlist` is explicitly **not** authoritative.
