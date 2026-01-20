pub mod container;
pub mod error;
pub mod fuji;
pub mod props;
pub mod structs;

pub use container::*;
pub use props::*;
pub use structs::*;

use std::{cmp::min, io::Cursor, time::Duration};

use anyhow::anyhow;
use log::{debug, error, trace, warn};
use ptp_cursor::{PtpDeserialize, PtpSerialize};
use rusb::GlobalContext;

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
    ) -> anyhow::Result<Vec<u8>> {
        let transaction_id = self.transaction_id;

        trace!(
            "PTP tx={transaction_id}: code={code:?}, params={params:?}, data_len={}",
            data.map_or(0, <[u8]>::len)
        );

        trace!("PTP tx={transaction_id}: sending command header");
        let mut payload = Vec::with_capacity(params.len() * 4);
        for p in params {
            p.try_write_ptp(&mut payload)?;
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
            let mut response: anyhow::Result<Vec<u8>> = Ok(Vec::new());
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
                            ContainerCode::Command(_)
                            | ContainerCode::Response(ResponseCode::Ok) => {}
                            ContainerCode::Response(code) => {
                                response = Err(anyhow!(error::Error::Response(code.into())));
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
            response.as_ref().map(std::vec::Vec::len).unwrap_or(0)
        );

        response
    }
    fn write(
        &self,
        kind: ContainerType,
        code: CommandCode,
        payload: &[u8],
        transaction_id: u32,
    ) -> anyhow::Result<()> {
        let container_info = ContainerInfo::new(kind, code, transaction_id, payload.len())?;
        let mut buffer: Vec<u8> = container_info.try_into_ptp()?;

        let first_chunk_len = min(payload.len(), self.chunk_size - ContainerInfo::SIZE);
        buffer.extend_from_slice(&payload[..first_chunk_len]);

        trace!(
            "PTP write: {kind:?} container, code={code:?}, tx={transaction_id}, chunk_size={first_chunk_len}",
        );
        self.handle
            .write_bulk(self.bulk_out, &buffer, Duration::ZERO)?;

        for chunk in payload[first_chunk_len..].chunks(self.chunk_size) {
            trace!("PTP write: additional chunk ({} bytes)", chunk.len());
            self.handle
                .write_bulk(self.bulk_out, chunk, Duration::ZERO)?;
        }

        Ok(())
    }

    fn read(&self) -> anyhow::Result<(ContainerInfo, Vec<u8>)> {
        let mut stack_buf = [0u8; 8 * 1024];

        let n = self
            .handle
            .read_bulk(self.bulk_in, &mut stack_buf, Duration::ZERO)?;
        let buf = &stack_buf[..n];
        trace!("PTP read: initial chunk ({n} bytes)");

        let mut cur = Cursor::new(buf);
        let container_info = ContainerInfo::try_read_ptp(&mut cur)?;

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
                .read_bulk(self.bulk_in, &mut chunk, Duration::ZERO)?;
            trace!("PTP read: additional chunk ({n} bytes)");
            if n == 0 {
                break;
            }
            payload.extend_from_slice(&chunk[..n]);
        }

        Ok((container_info, payload))
    }

    pub fn open_session(&mut self, session_id: u32) -> anyhow::Result<()> {
        debug!("Opening PTP session");
        self.send(CommandCode::OpenSession, &[session_id], None)?;
        Ok(())
    }

    pub fn close_session(&mut self, _: u32) -> anyhow::Result<()> {
        debug!("Closing PTP session");
        self.send(CommandCode::CloseSession, &[], None)?;
        Ok(())
    }

    pub fn get_info(&mut self) -> anyhow::Result<DeviceInfo> {
        debug!("Retrieving device info");
        let response = self.send(CommandCode::GetDeviceInfo, &[], None)?;
        let info = DeviceInfo::try_from_ptp(&response)?;
        Ok(info)
    }

    pub fn get_prop_raw(&mut self, prop: DevicePropCode) -> anyhow::Result<Vec<u8>> {
        debug!("Getting device prop: {prop:?}");
        let response = self.send(CommandCode::GetDevicePropValue, &[prop.into()], None)?;
        Ok(response)
    }

    pub fn set_prop_raw(&mut self, prop: DevicePropCode, value: &[u8]) -> anyhow::Result<Vec<u8>> {
        debug!("Setting device prop: {prop:?}");
        let response = self.send(CommandCode::SetDevicePropValue, &[prop.into()], Some(value))?;
        Ok(response)
    }

    pub fn get_prop<T: PtpDeserialize>(&mut self, code: DevicePropCode) -> anyhow::Result<T> {
        let bytes = self.get_prop_raw(code)?;
        let value = T::try_from_ptp(&bytes)?;
        Ok(value)
    }

    pub fn set_prop<T: PtpSerialize>(
        &mut self,
        code: DevicePropCode,
        value: &T,
    ) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
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
