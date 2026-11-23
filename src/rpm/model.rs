//! Package metadata shared by every format plugin.

#[derive(Debug, Clone, Default)]
pub struct RpmInfo {
    pub path: std::path::PathBuf,
    pub raw_header: Vec<u8>,
    pub name: String,
    pub version: String,
    pub release: String,
    pub arch: String,
    pub epoch: Option<u32>,
    pub summary: String,
    pub description: String,
    pub url: String,
    pub license: String,
    pub sourcerpm: String,
    /// RPMTAG_SIZE - used for @filesize@ and the @info@ line.
    pub size: u64,
    /// RPMTAG_GROUP (I18NSTRING) - last field of the @info@ line.
    pub group: String,
    pub provides: Vec<String>,
    pub requires: Vec<String>,
    pub conflicts: Vec<String>,
    pub obsoletes: Vec<String>,
    pub files: Vec<String>,
    pub changelog_time: Vec<i64>,
    pub changelog_name: Vec<String>,
    pub changelog_text: Vec<String>,
}

impl RpmInfo {
    pub fn fullname(&self) -> String {
        format!(
            "{}-{}-{}.{}",
            self.name, self.version, self.release, self.arch
        )
    }
}
