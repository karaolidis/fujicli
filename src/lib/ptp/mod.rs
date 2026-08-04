pub mod container;
pub mod error;
pub mod option;
pub mod props;
pub mod structs;
pub mod transport;

pub use container::*;
pub use props::*;
pub use structs::*;
pub use transport::{Device, PtpTransport, TransportKind};

use log::debug;
use ptp_cursor::{PtpDeserialize, PtpSerialize};

pub struct Ptp {
    pub transport: Box<dyn PtpTransport>,
}

impl Ptp {
    #[must_use]
    pub fn new(transport: Box<dyn PtpTransport>) -> Self {
        Self { transport }
    }

    #[must_use]
    pub fn id(&self) -> String {
        self.transport.id()
    }

    #[must_use]
    pub fn chunk_size(&self) -> usize {
        self.transport.chunk_size()
    }

    pub fn send(
        &mut self,
        code: CommandCode,
        params: &[u32],
        data: Option<&[u8]>,
    ) -> anyhow::Result<Vec<u8>> {
        self.transport.transact(code.into(), params, data)
    }

    pub fn open_session(&mut self, session_id: u32) -> anyhow::Result<()> {
        self.transport.open_session(session_id)
    }

    pub fn close_session(&mut self, session_id: u32) -> anyhow::Result<()> {
        self.transport.close_session(session_id)
    }

    pub fn get_info(&mut self) -> anyhow::Result<DeviceInfo> {
        debug!("Retrieving device info");
        let response = self.send(CommandCode::GetDeviceInfo, &[], None)?;
        let info = DeviceInfo::try_from_ptp(&response)?;
        Ok(info)
    }

    pub fn get_prop_raw(&mut self, prop: impl Into<u16>) -> anyhow::Result<Vec<u8>> {
        let prop = prop.into();
        debug!("Getting device prop: 0x{prop:04x}");
        let response = self.send(CommandCode::GetDevicePropValue, &[u32::from(prop)], None)?;
        Ok(response)
    }

    pub fn set_prop_raw(&mut self, prop: impl Into<u16>, value: &[u8]) -> anyhow::Result<Vec<u8>> {
        let prop = prop.into();
        debug!("Setting device prop: 0x{prop:04x}");
        let response = self.send(
            CommandCode::SetDevicePropValue,
            &[u32::from(prop)],
            Some(value),
        )?;
        Ok(response)
    }

    pub fn get_prop<T: PtpDeserialize>(&mut self, code: impl Into<u16>) -> anyhow::Result<T> {
        let bytes = self.get_prop_raw(code)?;
        let value = T::try_from_ptp(&bytes)?;
        Ok(value)
    }

    pub fn set_prop<T: PtpSerialize>(
        &mut self,
        code: impl Into<u16>,
        value: &T,
    ) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_raw(code, &bytes)?;
        Ok(())
    }
}
