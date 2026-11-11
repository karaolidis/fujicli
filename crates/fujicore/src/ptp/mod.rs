pub mod container;
pub mod error;
pub mod option;
pub mod props;
pub mod structs;
pub mod transport;

pub use container::*;
pub use error::PtpError;
pub use props::*;
pub use structs::*;

use log::debug;
use ptp_cursor::{PtpDeserialize, PtpSerialize};

use crate::CoreResult;
use transport::Transport;

pub struct Ptp {
    transport: Transport,
}

impl Ptp {
    pub(crate) const fn new(transport: Transport) -> Self {
        Self { transport }
    }

    /// Run an operation without a data phase.
    pub fn command(&mut self, code: CommandCode, params: &[u32]) -> CoreResult<()> {
        self.transport.command(code, params)
    }

    /// Run an operation whose data phase flows from the camera.
    pub fn read(&mut self, code: CommandCode, params: &[u32]) -> CoreResult<Vec<u8>> {
        self.transport.read(code, params)
    }

    /// Run an operation whose data phase flows to the camera.
    pub fn write(&mut self, code: CommandCode, params: &[u32], data: &[u8]) -> CoreResult<()> {
        self.transport.write(code, params, data)
    }

    pub fn get_info(&mut self) -> CoreResult<DeviceInfo> {
        debug!("Retrieving device info");
        let response = self.read(CommandCode::GetDeviceInfo, &[])?;
        let info = DeviceInfo::try_from_ptp(&response).map_err(PtpError::from)?;
        Ok(info)
    }

    pub fn get_prop_raw(&mut self, prop: impl Into<u16>) -> CoreResult<Vec<u8>> {
        let prop = prop.into();
        debug!("Getting device prop: 0x{prop:04x}");
        self.read(CommandCode::GetDevicePropValue, &[u32::from(prop)])
    }

    pub fn set_prop_raw(&mut self, prop: impl Into<u16>, value: &[u8]) -> CoreResult<()> {
        let prop = prop.into();
        debug!("Setting device prop: 0x{prop:04x}");
        self.write(CommandCode::SetDevicePropValue, &[u32::from(prop)], value)
    }

    pub fn get_prop<T: PtpDeserialize>(&mut self, code: impl Into<u16>) -> CoreResult<T> {
        let bytes = self.get_prop_raw(code)?;
        let value = T::try_from_ptp(&bytes).map_err(PtpError::from)?;
        Ok(value)
    }

    pub fn set_prop<T: PtpSerialize>(&mut self, code: impl Into<u16>, value: &T) -> CoreResult<()> {
        let bytes = value.try_into_ptp().map_err(PtpError::from)?;
        self.set_prop_raw(code, &bytes)
    }
}
