pub mod enums;
pub mod error;
pub mod read;
pub mod structs;

use std::{cmp::min, time::Duration};

use anyhow::bail;
use byteorder::{LittleEndian, WriteBytesExt};
use enums::{CommandCode, ContainerCode, ContainerType, ResponseCode};
use log::{debug, error, trace};
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
        params: Option<&[u32]>,
        data: Option<&[u8]>,
        transaction: bool,
        timeout: Duration,
    ) -> anyhow::Result<Vec<u8>> {
        let (params, transaction_id) = self.prepare_send(params, transaction);
        self.send_header(code, params, transaction_id, timeout)?;
        if let Some(data) = data {
            self.write(ContainerType::Data, code, data, transaction_id, timeout)?;
        }
        self.receive_response(timeout)
    }

    pub fn send_many(
        &mut self,
        code: CommandCode,
        params: Option<&[u32]>,
        data: Option<&[&[u8]]>,
        transaction: bool,
        timeout: Duration,
    ) -> anyhow::Result<Vec<u8>> {
        let (params, transaction_id) = self.prepare_send(params, transaction);
        self.send_header(code, params, transaction_id, timeout)?;
        if let Some(data) = data {
            self.write_many(ContainerType::Data, code, data, transaction_id, timeout)?;
        }
        self.receive_response(timeout)
    }

    fn prepare_send<'a>(
        &mut self,
        params: Option<&'a [u32]>,
        transaction: bool,
    ) -> (&'a [u32], Option<u32>) {
        let params = params.unwrap_or_default();
        let transaction_id = if transaction {
            let transaction_id = Some(self.transaction_id);
            self.transaction_id += 1;
            transaction_id
        } else {
            None
        };
        (params, transaction_id)
    }

    fn send_header(
        &self,
        code: CommandCode,
        params: &[u32],
        transaction_id: Option<u32>,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let mut payload = Vec::with_capacity(params.len() * 4);
        for p in params {
            payload.write_u32::<LittleEndian>(*p).ok();
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
                    match container.code {
                        ContainerCode::Command(_) | ContainerCode::Response(ResponseCode::Ok) => {}
                        ContainerCode::Response(code) => {
                            bail!(error::Error::Response(code as u16));
                        }
                    }

                    trace!(
                        "Command completed successfully, response payload of {} bytes",
                        response.len(),
                    );
                    return Ok(response);
                }
                _ => {
                    debug!("Ignoring unexpected container type: {:?}", container.kind);
                }
            }
        }
    }

    fn write(
        &self,
        kind: ContainerType,
        code: CommandCode,
        payload: &[u8],
        transaction_id: Option<u32>,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let container_info = ContainerInfo::new(kind, code, transaction_id, payload.len())?;
        let mut buffer: Vec<u8> = container_info.try_into()?;

        let first_chunk_len = min(payload.len(), self.chunk_size - container_info.len());
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

    fn write_many(
        &self,
        kind: ContainerType,
        code: CommandCode,
        parts: &[&[u8]],
        transaction_id: Option<u32>,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        if parts.is_empty() {
            return self.write(kind, code, &[], transaction_id, timeout);
        }

        if parts.len() == 1 {
            return self.write(kind, code, parts[0], transaction_id, timeout);
        }

        let total_len: usize = parts.iter().map(|c| c.len()).sum();
        let container_info = ContainerInfo::new(kind, code, transaction_id, total_len)?;
        let mut buffer: Vec<u8> = container_info.try_into()?;

        let first = parts[0];
        let first_part_chunk_len = min(first.len(), self.chunk_size - container_info.len());
        buffer.extend_from_slice(&first[..first_part_chunk_len]);

        trace!(
            "Writing PTP {kind:?} container, code: {code:?}, transaction: {transaction_id:?}, first payload part chunk ({first_part_chunk_len} bytes)",
        );
        self.handle.write_bulk(self.bulk_out, &buffer, timeout)?;

        for chunk in first[first_part_chunk_len..].chunks(self.chunk_size) {
            trace!(
                "Writing additional payload part chunk ({} bytes)",
                chunk.len(),
            );
            self.handle.write_bulk(self.bulk_out, chunk, timeout)?;
        }

        for part in &parts[1..] {
            trace!("Writing additional payload part");
            for chunk in part.chunks(self.chunk_size) {
                trace!(
                    "Writing additional payload part chunk ({} bytes)",
                    chunk.len(),
                );
                self.handle.write_bulk(self.bulk_out, chunk, timeout)?;
            }
            trace!(
                "Write completed for part, total payload of {} bytes",
                part.len()
            );
        }

        trace!("Write completed for code {code:?}, total payload of {total_len} bytes");
        Ok(())
    }

    fn read(&self, timeout: Duration) -> anyhow::Result<(ContainerInfo, Vec<u8>)> {
        let mut stack_buf = [0u8; 8 * 1024];

        let n = self
            .handle
            .read_bulk(self.bulk_in, &mut stack_buf, timeout)?;
        let buf = &stack_buf[..n];
        trace!("Read chunk ({n} bytes)");

        let container_info = ContainerInfo::try_from(buf)?;

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
