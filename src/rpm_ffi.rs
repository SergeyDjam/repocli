//! Backward-compatible re-export.
//!
//! `RpmInfo` now lives in `src/rpm/model.rs` (TZ v1.2, stage 1).
//! This module can be removed once all callers use `crate::rpm::RpmInfo`.

pub use crate::rpm::model::RpmInfo;
