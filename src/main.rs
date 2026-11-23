mod compression;
mod hdlist;
mod incremental;
mod lock;
mod md5sum;
mod merge;
mod old_metadata;
mod old_rpms;
mod packdrakeng;
mod rpm;
mod rpm_ffi;
mod rpm_reader;
mod synthesis;
mod utils;
mod versioning;
mod xml_metadata;

use crate::rpm::{RpmInfo, RpmReaderRegistry};
use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Parser)]
#[command(name = "repocli", version, about, long_about = None)]
struct Args {
    #[arg(long)]
    clean: bool,
    #[arg(long)]
    no_bad_rpm: bool,
    #[arg(long)]
    no_md5sum: bool,
    #[arg(long)]
    no_clean_old_rpms: bool,
    #[arg(long)]
    only_clean_old_rpms: bool,
    #[arg(long)]
    merge: bool,
    #[arg(long)]
    nolock: bool,
    #[arg(long)]
    no_hdlist: bool,
    #[arg(long)]
    allow_empty_media: bool,
    #[arg(long)]
    file_deps: Option<PathBuf>,
    #[arg(long, default_value = ".cz:xz -1")]
    synthesis_filter: String,
    #[arg(long, default_value = "auto")]
    xml_info: String,
    #[arg(long, default_value = ".lzma:xz -1")]
    xml_info_filter: String,
    #[arg(long, default_value = "auto")]
    versioned: String,
    #[arg(long)]
    media_info_dir: Option<PathBuf>,
    #[arg(short, long, action = clap::ArgAction::Count)]
    quiet: u8,
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    dir: PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(e) = run(args).await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

async fn run(args: Args) -> Result<()> {
    std::env::set_var("LC_ALL", "C");
    let rpms_dir = &args.dir;
    let media_info_dir = args
        .media_info_dir
        .clone()
        .unwrap_or_else(|| rpms_dir.join("media_info"));
    fs::create_dir_all(&media_info_dir)
        .with_context(|| format!("Cannot create {}", media_info_dir.display()))?;

    let lock_path = media_info_dir.join("UPDATING");
    let _lock = if !args.nolock {
        Some(lock::global_lock(&lock_path)?)
    } else {
        None
    };
    let _cleanup = CleanupOnSignal::new(&media_info_dir, _lock.as_ref());

    let rpms = utils::find_rpms(rpms_dir)?;
    if rpms.is_empty() && !args.allow_empty_media {
        anyhow::bail!("no *.rpm found in {}", rpms_dir.display());
    }

    let old_rpms_file = media_info_dir.join("old-rpms.lst");
    let mut old_rpms = old_rpms::read_old_rpms(&old_rpms_file, args.nolock)?;
    if let Some(ref mut old) = old_rpms {
        let mut rpms_list = rpms
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        old_rpms::filter_out_old_rpms(rpms_dir, old, &mut rpms_list)?;
        if !args.no_clean_old_rpms {
            old_rpms::clean_old_rpms(rpms_dir, old)?;
            old_rpms::write_old_rpms(old, &old_rpms_file)?;
        }
        if args.only_clean_old_rpms {
            return Ok(());
        }
    }

    let (synthesis_suffix, synthesis_cmd) = split_filter(&args.synthesis_filter);
    let (xml_suffix, xml_cmd) = split_filter(&args.xml_info_filter);

    let gen_xml = matches!(args.xml_info.as_str(), "auto" | "true");
    let xml_types: Vec<&str> = if gen_xml {
        vec!["info", "files", "changelog"]
    } else {
        vec![]
    };

    let file_deps = if let Some(path) = &args.file_deps {
        Some(utils::read_file_deps(path)?)
    } else {
        None
    };

    let t_metadata = std::time::Instant::now();
    let registry = RpmReaderRegistry::default();
    let new_packages = if !args.only_clean_old_rpms {
        let mut pkgs = Vec::with_capacity(rpms.len());
        for rpm_path in &rpms {
            match read_package(rpm_path, &registry) {
                Ok(info) => pkgs.push(info),
                Err(e) => {
                    if args.no_bad_rpm {
                        eprintln!("bad rpm {}: {}", rpm_path.display(), e);
                    } else {
                        return Err(e).context(format!("bad rpm {}", rpm_path.display()));
                    }
                }
            }
        }
        pkgs
    } else {
        vec![]
    };

    report_time("metadata", t_metadata);

    let t_pipeline = std::time::Instant::now();
    if !args.only_clean_old_rpms {
        if args.merge {
            merge::run(
                &media_info_dir,
                &new_packages,
                &args,
                &synthesis_suffix,
                &synthesis_cmd,
                &xml_suffix,
                &xml_cmd,
                &xml_types,
                file_deps.as_ref(),
            )
            .await?;
        } else {
            incremental::run(
                &media_info_dir,
                &new_packages,
                &args,
                &synthesis_suffix,
                &synthesis_cmd,
                &xml_suffix,
                &xml_cmd,
                &xml_types,
                file_deps.as_ref(),
            )
            .await?;
        }
    }

    report_time("pipeline", t_pipeline);

    let t_final = std::time::Instant::now();
    let mut final_files = vec![format!("synthesis.hdlist{}", synthesis_suffix)];
    if !args.no_hdlist {
        final_files.push("hdlist.cz".to_string());
    }
    for typ in &xml_types {
        final_files.push(format!("{}.xml{}", typ, xml_suffix));
    }

    if args.versioned != "false" {
        let existed = versioning::remove_previous_versioned_symlinks(&media_info_dir)?;
        if args.versioned == "true" || (args.versioned == "auto" && existed) {
            versioning::create_versioned_symlinks(&media_info_dir, &final_files)?;
        }
    }

    if !args.no_md5sum {
        md5sum::generate(&media_info_dir, &final_files)?;
    }
    report_time("finalize", t_final);

    Ok(())
}

fn report_time(label: &str, start: std::time::Instant) {
    if std::env::var_os("REPOCLI_TIMING").is_some() {
        eprintln!("[timing] {}: {:?}", label, start.elapsed());
    }
}

/// Route rpm5 packages to the tag-based reader; keep the legacy path for
/// other formats until their plugins are implemented (TZ v1.2, stage 2).
fn read_package(path: &std::path::Path, registry: &RpmReaderRegistry) -> Result<RpmInfo> {
    // Every supported format is backed by the shared tag-based reader;
    // the registry dispatches on RPMTAG_RPMVERSION.
    registry.read(path)
}

fn split_filter(spec: &str) -> (String, String) {
    if let Some((suffix, cmd)) = spec.split_once(':') {
        (suffix.to_string(), cmd.to_string())
    } else {
        (spec.to_string(), String::new())
    }
}

struct CleanupOnSignal<'a> {
    dir: &'a Path,
    lock: Option<&'a lock::LockGuard>,
}

impl<'a> CleanupOnSignal<'a> {
    fn new(dir: &'a Path, lock: Option<&'a lock::LockGuard>) -> Self {
        let s = Self { dir, lock };
        let dir_path = s.dir.to_path_buf();
        let lock_path = s.lock.map(|l| l.path().to_path_buf());
        ctrlc::set_handler(move || {
            let _ = std::fs::remove_file(dir_path.join("hdlist.cz.tmp"));
            let _ = std::fs::remove_file(dir_path.join("synthesis.hdlist.tmp"));
            let _ = std::fs::remove_file(dir_path.join("files.xml.tmp"));
            let _ = std::fs::remove_file(dir_path.join("info.xml.tmp"));
            let _ = std::fs::remove_file(dir_path.join("changelog.xml.tmp"));
            if let Some(ref p) = lock_path {
                let _ = std::fs::remove_file(p);
            }
            std::process::exit(1);
        })
        .expect("Error setting Ctrl-C handler");
        s
    }
}
