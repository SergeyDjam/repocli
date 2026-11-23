use crate::hdlist;
use crate::rpm::RpmInfo;
use crate::synthesis;
use crate::xml_metadata;
use crate::Args;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::{Command, Stdio};

#[allow(clippy::too_many_arguments)]
pub async fn run(
    media_info_dir: &Path,
    new_packages: &[RpmInfo],
    args: &Args,
    synthesis_suffix: &str,
    synthesis_cmd: &str,
    xml_suffix: &str,
    xml_cmd: &str,
    xml_types: &[&str],
    file_deps: Option<&HashSet<String>>,
) -> Result<()> {
    // Hdlist from the raw headers collected during metadata parsing:
    // each RPM is opened only once.
    if !args.no_hdlist {
        let hdlist_path = media_info_dir.join("hdlist.cz");
        let tmp_path = media_info_dir.join("hdlist.cz.tmp");
        hdlist::build_hdlist_from_packages(new_packages, &tmp_path)?;
        fs::rename(&tmp_path, &hdlist_path)?;
    }

    // Synthesis
    let synth_path = media_info_dir.join(format!("synthesis.hdlist{}", synthesis_suffix));
    let tmp_synth = media_info_dir.join("synthesis.hdlist.tmp");
    {
        let mut child = if synthesis_cmd.is_empty() {
            Command::new("cat")
                .stdin(Stdio::piped())
                .stdout(fs::File::create(&tmp_synth)?)
                .spawn()
                .context("Failed to spawn cat")?
        } else {
            let parts: Vec<&str> = synthesis_cmd.split_whitespace().collect();
            if parts.is_empty() {
                anyhow::bail!("Empty compression command");
            }
            Command::new(parts[0])
                .args(&parts[1..])
                .stdin(Stdio::piped())
                .stdout(fs::File::create(&tmp_synth)?)
                .stderr(Stdio::null())
                .spawn()
                .context("Failed to spawn compression command")?
        };
        {
            let stdin = child.stdin.as_mut().context("Failed to get stdin")?;
            let mut writer = BufWriter::new(stdin);
            synthesis::write_synthesis(&mut writer, new_packages, file_deps)?;
            writer.flush()?;
        }
        child.wait()?;
    }
    fs::rename(&tmp_synth, &synth_path)?;

    // XML files
    if !xml_types.is_empty() {
        for &typ in xml_types {
            let path = media_info_dir.join(format!("{}.xml{}", typ, xml_suffix));
            let tmp_path = media_info_dir.join(format!("{}.xml.tmp", typ));
            let mut child = if xml_cmd.is_empty() {
                Command::new("cat")
                    .stdin(Stdio::piped())
                    .stdout(fs::File::create(&tmp_path)?)
                    .spawn()
                    .context("Failed to spawn cat")?
            } else {
                let parts: Vec<&str> = xml_cmd.split_whitespace().collect();
                if parts.is_empty() {
                    anyhow::bail!("Empty XML compression command");
                }
                Command::new(parts[0])
                    .args(&parts[1..])
                    .stdin(Stdio::piped())
                    .stdout(fs::File::create(&tmp_path)?)
                    .stderr(Stdio::null())
                    .spawn()
                    .context("Failed to spawn XML compression command")?
            };
            {
                let stdin = child.stdin.as_mut().context("Failed to get stdin")?;
                let mut writer = BufWriter::new(stdin);
                match typ {
                    "info" => xml_metadata::write_info_xml(&mut writer, new_packages)?,
                    "files" => xml_metadata::write_files_xml(&mut writer, new_packages)?,
                    "changelog" => xml_metadata::write_changelog_xml(&mut writer, new_packages)?,
                    _ => {}
                }
                writer.flush()?;
            }
            child.wait()?;
            fs::rename(&tmp_path, &path)?;
        }
    }
    Ok(())
}
