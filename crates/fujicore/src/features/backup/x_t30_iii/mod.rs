//! Read and write the recipes inside an X-T30 III backup file (firmware 1.10).
//!
//! This covers what PTP can't reach: Monochromatic Color and Portrait Enhancer of the custom slots, ISO /
//! ISO AUTO and other menu items, and the film simulation dial positions FS1-FS3 with their
//! recipes. The result is imported with `backup import`.

mod decode;
mod edit;
mod tables;
#[cfg(test)]
mod tests;

use serde_json::{Value, json};
use thiserror::Error;

pub use edit::SlotEdit;
use tables::{FS_COUNT, MAGIC, MODEL, SIZE, SLOTS};

#[derive(Debug, Error)]
pub enum BackupFileError {
    #[error("not an X-T30 III backup ({SIZE} bytes, starting with FUJIFILMX-BACKUP0100 X-T30 III)")]
    NotXT30III,

    #[error("custom slot must be 1..={SLOTS}, got {0}")]
    Slot(usize),

    #[error("film simulation dial position must be FS1..=FS{FS_COUNT}, got {0}")]
    Fs(usize),

    #[error("unknown FS recipe setting {0:?}")]
    UnknownKey(String),

    #[error("invalid value {value:?} for {key}")]
    InvalidValue { key: String, value: String },

    #[error("{key} would read back as {got} instead of {wanted}; nothing was written")]
    ReadBack {
        key: String,
        wanted: String,
        got: String,
    },

    #[error("backup contains a value this layout does not know")]
    Corrupt,
}

fn check(data: &[u8]) -> Result<(), BackupFileError> {
    let model = MAGIC.len()..MAGIC.len() + MODEL.len();
    if data.len() == SIZE && data.starts_with(MAGIC) && data.get(model) == Some(MODEL) {
        Ok(())
    } else {
        Err(BackupFileError::NotXT30III)
    }
}

fn check_slot(slot: usize) -> Result<(), BackupFileError> {
    (1..=SLOTS)
        .contains(&slot)
        .then_some(())
        .ok_or(BackupFileError::Slot(slot))
}

/// `{"custom": [C1..C7], "filmSimulationDial": [FS1..FS3]}`.
pub fn recipes(data: &[u8]) -> Result<Value, BackupFileError> {
    check(data)?;
    let custom: Vec<_> = (1..=SLOTS).map(|n| decode::decode_slot(data, n)).collect();
    let dial: Vec<_> = (1..=FS_COUNT).map(|n| decode::decode_fs(data, n)).collect();
    Ok(json!({ "custom": custom, "filmSimulationDial": dial }))
}

pub fn edit_slot(data: &[u8], slot: usize, edit: &SlotEdit) -> Result<Vec<u8>, BackupFileError> {
    check(data)?;
    check_slot(slot)?;
    edit::edit_slot(data, slot, edit)
}

/// Sets FS`n` recipe values (decoder keys and spellings), optionally starting from a copy of
/// custom slot `from_slot`. Fails without writing anything if a value does not read back.
pub fn edit_fs(
    data: &[u8],
    n: usize,
    from_slot: Option<usize>,
    sets: &[(String, String)],
) -> Result<Vec<u8>, BackupFileError> {
    check(data)?;
    if !(1..=FS_COUNT).contains(&n) {
        return Err(BackupFileError::Fs(n));
    }
    if let Some(slot) = from_slot {
        check_slot(slot)?;
    }
    edit::edit_fs(data, n, from_slot, sets)
}
