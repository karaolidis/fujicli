use std::{fmt, io};

use crate::camera::ptp::hex::ResponseCode;

#[derive(Debug)]
pub enum Error {
    Response(u16),
    Malformed(String),
    Usb(rusb::Error),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Response(r) => {
                let name = ResponseCode::try_from(r)
                    .map_or_else(|_| "Unknown".to_string(), |c| format!("{c:?}"));
                write!(f, "{name} (0x{r:04x})")
            }
            Self::Usb(ref e) => write!(f, "USB error: {e}"),
            Self::Io(ref e) => write!(f, "IO error: {e}"),
            Self::Malformed(ref e) => write!(f, "{e}"),
        }
    }
}

impl ::std::error::Error for Error {
    fn cause(&self) -> Option<&dyn (::std::error::Error)> {
        match *self {
            Self::Usb(ref e) => Some(e),
            Self::Io(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusb::Error> for Error {
    fn from(e: rusb::Error) -> Self {
        Self::Usb(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::UnexpectedEof => {
                Self::Malformed("Unexpected end of message".to_string())
            }
            _ => Self::Io(e),
        }
    }
}
