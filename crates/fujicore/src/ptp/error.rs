use std::io;

use thiserror::Error;

use crate::ptp::ResponseCode;

#[derive(Debug, Error)]
pub enum PtpError {
    #[error("PTP response: {0}")]
    Response(ResponseCode),

    #[error("unknown PTP container code 0x{0:04x}")]
    UnknownContainerCode(u16),

    #[error("malformed PTP message: {0}")]
    Malformed(String),

    #[error(transparent)]
    Transport(#[from] rusb::Error),

    #[error(transparent)]
    Io(io::Error),
}

impl From<io::Error> for PtpError {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::UnexpectedEof => Self::Malformed("unexpected end of message".to_owned()),
            _ => Self::Io(err),
        }
    }
}
