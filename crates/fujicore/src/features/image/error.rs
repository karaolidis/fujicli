use std::io;

use thiserror::Error;

use exiftool::ExifToolError;

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("not a Fujifilm MakerNotes header")]
    NotFujifilm,

    #[error("malformed MakerNote field at offset {offset}")]
    MalformedField { offset: u64 },

    #[error("MakerNotes value is not valid for this setting: {0}")]
    Value(#[from] serde_json::Error),

    #[error(transparent)]
    Exiftool(#[from] ExifToolError),

    #[error(transparent)]
    Io(#[from] io::Error),
}
