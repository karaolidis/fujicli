use std::io;

use thiserror::Error;

use crate::ptp::ResponseCode;

#[derive(Debug, Error)]
pub enum PtpError {
    #[error("PTP response: {0}")]
    Response(ResponseCode),

    #[error("unknown PTP container code 0x{0:04x}")]
    UnknownContainerCode(u16),

    #[error(transparent)]
    Transport(#[from] rusb::Error),

    #[error(transparent)]
    Io(#[from] io::Error),
}
