use std::fmt;

use thiserror::Error;

use crate::{
    UsbId,
    features::{image::ImageError, simulation::SimulationError},
    input::OptionError,
    ptp::{DevicePropCode, PtpError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    BackupManagement,
    SimulationParsing,
    SimulationManagement,
    RenderManagement,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::BackupManagement => "backup management",
            Self::SimulationParsing => "simulation parsing",
            Self::SimulationManagement => "simulation management",
            Self::RenderManagement => "image rendering",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("device {0} is not supported")]
    DeviceUnsupported(UsbId),

    #[error("no PTP imaging interface found on USB device")]
    NoImagingInterface,

    #[error("camera does not support {0}")]
    Unsupported(Capability),

    #[error("failed to parse value of device prop {prop}: {reason}")]
    DeviceInfoMalformed {
        prop: DevicePropCode,
        reason: String,
    },

    #[error("payload size {0} bytes exceeds PTP u32 limit")]
    PayloadTooLarge(usize),

    #[error(transparent)]
    Ptp(#[from] PtpError),

    #[error(transparent)]
    Usb(#[from] rusb::Error),

    #[error(transparent)]
    Option(#[from] OptionError),

    #[error(transparent)]
    Simulation(#[from] SimulationError),

    #[error(transparent)]
    Image(#[from] ImageError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl CoreError {
    #[must_use]
    pub const fn is_disconnect(&self) -> bool {
        matches!(
            self,
            Self::Usb(_) | Self::NoImagingInterface | Self::Ptp(PtpError::Transport(_))
        )
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
