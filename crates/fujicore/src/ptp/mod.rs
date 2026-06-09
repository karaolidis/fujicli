pub mod container;
pub mod error;
pub mod option;
pub mod props;
pub mod structs;

pub use container::*;
pub use error::PtpError;
pub use props::*;
pub use structs::*;

use std::{
    cmp::min,
    io::{self, Cursor},
    time::Duration,
};

use log::{debug, error, trace, warn};
use ptp_cursor::{PtpDeserialize, PtpSerialize};
use rusb::GlobalContext;

use crate::CoreResult;

pub struct Ptp {
    pub bus: u8,
    pub address: u8,
    pub interface: u8,
    pub bulk_in: u8,
    pub bulk_out: u8,
    pub handle: rusb::DeviceHandle<GlobalContext>,
    pub transaction_id: u32,
    pub chunk_size: usize,
}

impl Ptp {
    pub fn send(
        &mut self,
        code: CommandCode,
        params: &[u32],
        data: Option<&[u8]>,
    ) -> CoreResult<Vec<u8>> {
        let transaction_id = self.transaction_id;

        trace!(
            "PTP tx={transaction_id}: code={code:?}, params={params:?}, data_len={}",
            data.map_or(0, <[u8]>::len)
        );

        trace!("PTP tx={transaction_id}: sending command header");
        let mut payload = Vec::with_capacity(params.len() * 4);
        for p in params {
            p.try_write_ptp(&mut payload).map_err(PtpError::Io)?;
        }
        self.write(ContainerType::Command, code, &payload, transaction_id)?;

        if let Some(data) = data {
            trace!(
                "PTP tx={transaction_id}: sending payload ({} bytes)",
                data.len()
            );
            self.write(ContainerType::Data, code, data, transaction_id)?;
        }

        let response = {
            let mut response: CoreResult<Vec<u8>> = Ok(Vec::new());
            loop {
                trace!("PTP tx={transaction_id}: receiving response");
                let (container, payload) = self.read()?;

                match container.kind {
                    ContainerType::Data => {
                        trace!(
                            "PTP tx={transaction_id}: received data container ({} bytes)",
                            payload.len()
                        );
                        response = Ok(payload);
                    }
                    ContainerType::Response => {
                        trace!(
                            "PTP tx={transaction_id}: received response container (code={:x?})",
                            container.code
                        );

                        if self.transaction_id != container.transaction_id {
                            warn!(
                                "PTP transaction ID mismatch: got {}, expected {}",
                                container.transaction_id, self.transaction_id
                            );
                        }

                        match container.code {
                            ContainerCode::Response(ResponseCode::Ok) => {}
                            ContainerCode::Response(resp) => {
                                response = Err(PtpError::Response(resp).into());
                            }
                            ContainerCode::Command(cmd) => {
                                response = Err(PtpError::Io(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!("PTP response container carried command code {cmd:?}"),
                                ))
                                .into());
                            }
                        }

                        break;
                    }
                    _ => {
                        warn!("Unexpected PTP container type: {:x?}", container.kind);
                    }
                }
            }
            response
        };

        self.transaction_id += 1;
        trace!(
            "PTP tx={transaction_id}: complete with response length {}",
            response.as_ref().map_or(0, std::vec::Vec::len)
        );

        response
    }

    fn write(
        &self,
        kind: ContainerType,
        code: CommandCode,
        payload: &[u8],
        transaction_id: u32,
    ) -> CoreResult<()> {
        let container_info = ContainerInfo::new(kind, code, transaction_id, payload.len())?;
        let mut buffer: Vec<u8> = container_info.try_into_ptp().map_err(PtpError::Io)?;

        let first_chunk_len = min(payload.len(), self.chunk_size - ContainerInfo::SIZE);
        buffer.extend_from_slice(&payload[..first_chunk_len]);

        trace!(
            "PTP write: {kind:?} container, code={code:?}, tx={transaction_id}, chunk_size={first_chunk_len}",
        );
        self.handle
            .write_bulk(self.bulk_out, &buffer, Duration::ZERO)
            .map_err(PtpError::Transport)?;

        for chunk in payload[first_chunk_len..].chunks(self.chunk_size) {
            trace!("PTP write: additional chunk ({} bytes)", chunk.len());
            self.handle
                .write_bulk(self.bulk_out, chunk, Duration::ZERO)
                .map_err(PtpError::Transport)?;
        }

        Ok(())
    }

    fn read(&self) -> CoreResult<(ContainerInfo, Vec<u8>)> {
        let mut stack_buf = [0u8; 8 * 1024];

        let n = self
            .handle
            .read_bulk(self.bulk_in, &mut stack_buf, Duration::ZERO)
            .map_err(PtpError::Transport)?;
        let buf = &stack_buf[..n];
        trace!("PTP read: initial chunk ({n} bytes)");

        let mut cur = Cursor::new(buf);
        let container_info = ContainerInfo::try_read_ptp(&mut cur).map_err(PtpError::Io)?;

        if (container_info.total_len as usize) < ContainerInfo::SIZE {
            return Err(PtpError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "PTP container total length {} is below the {}-byte header",
                    container_info.total_len,
                    ContainerInfo::SIZE
                ),
            ))
            .into());
        }

        let payload_len = container_info.payload_len();
        if payload_len == 0 {
            return Ok((container_info, Vec::new()));
        }

        let mut payload = Vec::with_capacity(payload_len);
        if buf.len() > ContainerInfo::SIZE {
            payload.extend_from_slice(&buf[ContainerInfo::SIZE..]);
        }

        while payload.len() < payload_len {
            let remaining = payload_len - payload.len();
            let mut chunk = vec![0u8; min(remaining, self.chunk_size)];
            let n = self
                .handle
                .read_bulk(self.bulk_in, &mut chunk, Duration::ZERO)
                .map_err(PtpError::Transport)?;
            trace!("PTP read: additional chunk ({n} bytes)");
            if n == 0 {
                break;
            }
            payload.extend_from_slice(&chunk[..n]);
        }

        Ok((container_info, payload))
    }

    pub fn open_session(&mut self, session_id: u32) -> CoreResult<()> {
        debug!("Opening PTP session");
        self.send(CommandCode::OpenSession, &[session_id], None)?;
        Ok(())
    }

    pub fn close_session(&mut self, _: u32) -> CoreResult<()> {
        debug!("Closing PTP session");
        self.send(CommandCode::CloseSession, &[], None)?;
        Ok(())
    }

    pub fn get_info(&mut self) -> CoreResult<DeviceInfo> {
        debug!("Retrieving device info");
        let response = self.send(CommandCode::GetDeviceInfo, &[], None)?;
        let info = DeviceInfo::try_from_ptp(&response).map_err(PtpError::Io)?;
        Ok(info)
    }

    pub fn get_prop_raw(&mut self, prop: impl Into<u16>) -> CoreResult<Vec<u8>> {
        let prop = prop.into();
        debug!("Getting device prop: 0x{prop:04x}");
        self.send(CommandCode::GetDevicePropValue, &[u32::from(prop)], None)
    }

    pub fn set_prop_raw(&mut self, prop: impl Into<u16>, value: &[u8]) -> CoreResult<Vec<u8>> {
        let prop = prop.into();
        debug!("Setting device prop: 0x{prop:04x}");
        self.send(
            CommandCode::SetDevicePropValue,
            &[u32::from(prop)],
            Some(value),
        )
    }

    pub fn get_prop<T: PtpDeserialize>(&mut self, code: impl Into<u16>) -> CoreResult<T> {
        let bytes = self.get_prop_raw(code)?;
        let value = T::try_from_ptp(&bytes).map_err(PtpError::Io)?;
        Ok(value)
    }

    pub fn set_prop<T: PtpSerialize>(&mut self, code: impl Into<u16>, value: &T) -> CoreResult<()> {
        let bytes = value.try_into_ptp().map_err(PtpError::Io)?;
        self.set_prop_raw(code, &bytes)?;
        Ok(())
    }
}

impl Drop for Ptp {
    fn drop(&mut self) {
        if let Err(e) = self.handle.release_interface(self.interface) {
            error!("Failed to release USB interface: {e}");
        }
    }
}
