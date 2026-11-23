use crate::lock;
use anyhow::Result;
use serde_ini::de::from_str;
use serde_ini::ser::to_string;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct OldRpms {
    pub remove: BTreeMap<String, u64>,
    pub keep_in_hdlist: BTreeMap<String, u64>,
    pub lock: Option<lock::LockGuard>,
}

pub fn read_old_rpms(file: &Path, nolock: bool) -> Result<Option<OldRpms>> {
    if !file.exists() {
        return Ok(None);
    }
    let lock = if !nolock {
        Some(lock::global_lock(&file.with_extension("lst.lock"))?)
    } else {
        None
    };
    let content = fs::read_to_string(file)?;
    let ini: BTreeMap<String, BTreeMap<String, String>> = from_str(&content)?;
    let mut old = OldRpms::default();
    for (section, map) in ini {
        let target = match section.as_str() {
            "Remove" => &mut old.remove,
            "Keep-in-hdlist" => &mut old.keep_in_hdlist,
            _ => continue,
        };
        for (pkg, value) in map {
            let timestamp = value.parse::<u64>().unwrap_or(0);
            target.insert(pkg, timestamp);
        }
    }
    old.lock = lock;
    Ok(Some(old))
}

pub fn write_old_rpms(old: &OldRpms, file: &Path) -> Result<()> {
    if old.lock.is_none() {
        return Ok(());
    }
    let mut ini = BTreeMap::new();
    if !old.remove.is_empty() {
        let mut map = BTreeMap::new();
        for (pkg, ts) in &old.remove {
            map.insert(pkg.clone(), ts.to_string());
        }
        ini.insert("Remove".to_string(), map);
    }
    if !old.keep_in_hdlist.is_empty() {
        let mut map = BTreeMap::new();
        for (pkg, ts) in &old.keep_in_hdlist {
            map.insert(pkg.clone(), ts.to_string());
        }
        ini.insert("Keep-in-hdlist".to_string(), map);
    }
    let content = to_string(&ini)?;
    fs::write(file, content)?;
    Ok(())
}

pub fn filter_out_old_rpms(
    _rpms_dir: &Path,
    old: &mut OldRpms,
    rpms_list: &mut Vec<String>,
) -> Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    old.keep_in_hdlist.retain(|_pkg, &mut ts| ts >= now);
    let to_remove: Vec<_> = old.remove.keys().cloned().collect();
    rpms_list.retain(|rpm| !to_remove.contains(rpm));
    Ok(())
}

pub fn clean_old_rpms(rpms_dir: &Path, old: &mut OldRpms) -> Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    old.remove.retain(|pkg, &mut ts| {
        let expired = ts < now;
        if expired {
            let path = rpms_dir.join(format!("{}.rpm", pkg));
            if path.exists() {
                println!("[OLD-RPMS] removing rpm file {}", pkg);
                let _ = std::fs::remove_file(&path);
            }
        }
        !expired
    });
    Ok(())
}
