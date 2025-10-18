pub mod error;
pub mod hex;
pub mod structs;

use std::{cmp::min, io::Cursor, time::Duration};

use anyhow::bail;
use hex::{CommandCode, ContainerCode, ContainerType, ResponseCode};
use log::{debug, error, trace, warn};
use ptp_cursor::{PtpDeserialize, PtpSerialize};
use rusb::GlobalContext;
use structs::ContainerInfo;

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
        timeout: Duration,
    ) -> anyhow::Result<Vec<u8>> {
        let transaction_id = self.transaction_id;
        self.send_header(code, params, transaction_id, timeout)?;
        if let Some(data) = data {
            self.write(ContainerType::Data, code, data, transaction_id, timeout)?;
        }
        let response = self.receive_response(timeout);
        self.transaction_id += 1;
        response
    }

    fn send_header(
        &self,
        code: CommandCode,
        params: &[u32],
        transaction_id: u32,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let mut payload = Vec::with_capacity(params.len() * 4);
        for p in params {
            p.try_write_ptp(&mut payload)?;
        }

        trace!(
            "Sending PTP command: {:?}, transaction: {:?}, parameters ({} bytes): {:x?}",
            code,
            transaction_id,
            payload.len(),
            payload,
        );
        self.write(
            ContainerType::Command,
            code,
            &payload,
            transaction_id,
            timeout,
        )?;

        Ok(())
    }

    fn receive_response(&self, timeout: Duration) -> anyhow::Result<Vec<u8>> {
        let mut response = Vec::new();
        loop {
            let (container, payload) = self.read(timeout)?;
            match container.kind {
                ContainerType::Data => {
                    trace!("Response received: data ({} bytes)", payload.len());
                    response = payload;
                }
                ContainerType::Response => {
                    trace!("Response received: code {:?}", container.code);

                    if self.transaction_id != container.transaction_id {
                        warn!(
                            "Mismatched transaction_id {}, expecting {}",
                            container.transaction_id, self.transaction_id
                        );
                    }

                    match container.code {
                        ContainerCode::Command(_) | ContainerCode::Response(ResponseCode::Ok) => {}
                        ContainerCode::Response(code) => {
                            bail!(error::Error::Response(code.into()));
                        }
                    }

                    trace!(
                        "Command completed successfully, response payload of {} bytes",
                        response.len(),
                    );
                    return Ok(response);
                }
                _ => {
                    warn!("Unexpected container type: {:?}", container.kind);
                }
            }
        }
    }

    fn write(
        &self,
        kind: ContainerType,
        code: CommandCode,
        payload: &[u8],
        transaction_id: u32,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let container_info = ContainerInfo::new(kind, code, transaction_id, payload.len())?;
        let mut buffer: Vec<u8> = container_info.try_into_ptp()?;

        let first_chunk_len = min(payload.len(), self.chunk_size - ContainerInfo::SIZE);
        buffer.extend_from_slice(&payload[..first_chunk_len]);

        trace!(
            "Writing PTP {kind:?} container, code: {code:?}, transaction: {transaction_id:?}, first payload chunk ({first_chunk_len} bytes)",
        );
        self.handle.write_bulk(self.bulk_out, &buffer, timeout)?;

        for chunk in payload[first_chunk_len..].chunks(self.chunk_size) {
            trace!("Writing additional payload chunk ({} bytes)", chunk.len(),);
            self.handle.write_bulk(self.bulk_out, chunk, timeout)?;
        }

        trace!(
            "Write completed for code {:?}, total payload of {} bytes",
            code,
            payload.len()
        );
        Ok(())
    }

    fn read(&self, timeout: Duration) -> anyhow::Result<(ContainerInfo, Vec<u8>)> {
        let mut stack_buf = [0u8; 8 * 1024];

        let n = self
            .handle
            .read_bulk(self.bulk_in, &mut stack_buf, timeout)?;
        let buf = &stack_buf[..n];
        trace!("Read chunk ({n} bytes)");

        let mut cur = Cursor::new(buf);
        let container_info = ContainerInfo::try_read_ptp(&mut cur)?;

        let payload_len = container_info.payload_len();
        if payload_len == 0 {
            trace!("No payload in container");
            return Ok((container_info, Vec::new()));
        }

        let mut payload = Vec::with_capacity(payload_len);
        if buf.len() > ContainerInfo::SIZE {
            payload.extend_from_slice(&buf[ContainerInfo::SIZE..]);
        }

        while payload.len() < payload_len {
            let remaining = payload_len - payload.len();
            let mut chunk = vec![0u8; min(remaining, self.chunk_size)];
            let n = self.handle.read_bulk(self.bulk_in, &mut chunk, timeout)?;
            trace!("Read additional chunk ({n} bytes)");
            if n == 0 {
                break;
            }
            payload.extend_from_slice(&chunk[..n]);
        }

        trace!(
            "Finished reading container, total payload of {} bytes",
            payload.len(),
        );

        Ok((container_info, payload))
    }
}

impl Drop for Ptp {
    fn drop(&mut self) {
        debug!("Releasing interface");
        if let Err(e) = self.handle.release_interface(self.interface) {
            error!("Error releasing interface: {e}");
        }
        debug!("Interface released");
    }
}
