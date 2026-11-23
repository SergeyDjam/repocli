//! Synthesis writer, matching `URPM.xs::Pkg_build_info()` from URPM 4.62.
//!
//! Field order is significant: dependencies first, then summary/filesize,
//! with the `@info@` line last. Dependency entries are joined with '@' and
//! already carry their `[op evr]` suffix (see `URPM::print_list_entry`).

use anyhow::Result;
use std::collections::HashSet;
use std::io::Write;

use crate::rpm::RpmInfo;

fn write_list<W: Write>(writer: &mut W, name: &str, values: &[String]) -> std::io::Result<()> {
    if values.is_empty() {
        return Ok(());
    }
    writeln!(writer, "@{}@{}", name, values.join("@"))
}

pub fn write_synthesis<W: Write>(
    writer: &mut W,
    packages: &[RpmInfo],
    _file_deps: Option<&HashSet<String>>,
) -> Result<()> {
    for pkg in packages {
        write_list(writer, "provides", &pkg.provides)?;
        write_list(writer, "conflicts", &pkg.conflicts)?;
        write_list(writer, "obsoletes", &pkg.obsoletes)?;
        write_list(writer, "requires", &pkg.requires)?;

        if !pkg.summary.is_empty() {
            writeln!(writer, "@summary@{}", pkg.summary)?;
        }
        if pkg.size != 0 {
            writeln!(writer, "@filesize@{}", pkg.size)?;
        }

        let epoch = pkg.epoch.unwrap_or(0);
        writeln!(
            writer,
            "@info@{}@{}@{}@{}",
            pkg.fullname(),
            epoch,
            pkg.size,
            pkg.group
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_urpm_field_order() {
        let pkg = RpmInfo {
            name: "zstd".to_string(),
            version: "1.5.6".to_string(),
            release: "1".to_string(),
            arch: "x86_64".to_string(),
            size: 12345,
            group: "Development/Tools".to_string(),
            summary: "Fast compression".to_string(),
            provides: vec!["zstd[== 1.5.6-1]".to_string()],
            requires: vec!["libc[>= 2.0]".to_string(), "libz[>= 1.0]".to_string()],
            ..Default::default()
        };

        let mut out = Vec::new();
        write_synthesis(&mut out, &[pkg], None).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(
            text,
            concat!(
                "@provides@zstd[== 1.5.6-1]\n",
                "@requires@libc[>= 2.0]@libz[>= 1.0]\n",
                "@summary@Fast compression\n",
                "@filesize@12345\n",
                "@info@zstd-1.5.6-1.x86_64@0@12345@Development/Tools\n"
            )
        );
    }
}
