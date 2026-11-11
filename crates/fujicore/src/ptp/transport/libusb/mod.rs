use std::{cmp::min, io::Cursor, num::NonZeroU32, time::Duration};

use log::{debug, error, trace, warn};
use ptp_cursor::{PtpDeserialize, PtpSerialize};
use rusb::{GlobalContext, constants::LIBUSB_CLASS_IMAGE};

use crate::{
    CoreError, CoreResult, UsbId,
    ptp::{CommandCode, ContainerCode, ContainerInfo, ContainerType, PtpError, ResponseCode},
    usb::FUJIFILM_VENDOR_ID,
};

use super::SESSION;

pub struct Device {
    inner: rusb::Device<GlobalContext>,
    usb_id: UsbId,
    serial: Option<String>,
}

pub fn devices() -> CoreResult<Vec<Device>> {
    let mut devices = Vec::new();

    for device in rusb::devices()?.iter() {
        let descriptor = device.device_descriptor()?;
        let usb_id = UsbId {
            vendor: descriptor.vendor_id(),
            product: descriptor.product_id(),
        };

        if usb_id.vendor != FUJIFILM_VENDOR_ID {
            continue;
        }

        let serial = read_serial(&device, &descriptor);
        trace!("Found USB device {device:x?} ({usb_id}, serial {serial:?})");
        devices.push(Device {
            inner: device,
            usb_id,
            serial,
        });
    }

    Ok(devices)
}

fn read_serial(
    device: &rusb::Device<GlobalContext>,
    descriptor: &rusb::DeviceDescriptor,
) -> Option<String> {
    device
        .open()
        .and_then(|handle| handle.read_serial_number_string_ascii(descriptor))
        .inspect_err(|e| debug!("Could not read serial number of {device:x?}: {e}"))
        .ok()
}

impl Device {
    #[must_use]
    pub const fn usb_id(&self) -> UsbId {
        self.usb_id
    }

    #[must_use]
    pub fn serial(&self) -> Option<&str> {
        self.serial.as_deref()
    }

    pub fn open(&self, chunk_size: NonZeroU32) -> CoreResult<Transport> {
        let config_descriptor = self.inner.active_config_descriptor()?;
        let interface_descriptor = config_descriptor
            .interfaces()
            .flat_map(|i| i.descriptors())
            .find(|x| x.class_code() == LIBUSB_CLASS_IMAGE)
            .ok_or(CoreError::NoImagingInterface)?;

        let interface = interface_descriptor.interface_number();
        debug!("Found interface {interface}");

        let find_endpoint = |direction: rusb::Direction| {
            interface_descriptor
                .endpoint_descriptors()
                .find(|ep| {
                    ep.direction() == direction && ep.transfer_type() == rusb::TransferType::Bulk
                })
                .map(|x| x.address())
                .ok_or(rusb::Error::NotFound)
        };

        let bulk_in = find_endpoint(rusb::Direction::In)?;
        let bulk_out = find_endpoint(rusb::Direction::Out)?;
        debug!("Found bulk endpoints");

        let handle = self.inner.open()?;
        handle.claim_interface(interface)?;
        debug!("Claimed interface");

        let mut interface = Interface {
            handle,
            number: interface,
            bulk_in,
            bulk_out,
            transaction_id: 0,
            chunk_size: chunk_size.get() as usize,
        };

        debug!("Opening PTP session");
        interface.transact(CommandCode::OpenSession, &[SESSION], None)?;

        Ok(Transport(interface))
    }
}

pub struct Transport(Interface);

impl Transport {
    pub fn command(&mut self, code: CommandCode, params: &[u32]) -> CoreResult<()> {
        self.0.transact(code, params, None)?;
        Ok(())
    }

    pub fn read(&mut self, code: CommandCode, params: &[u32]) -> CoreResult<Vec<u8>> {
        self.0.transact(code, params, None)
    }

    pub fn write(&mut self, code: CommandCode, params: &[u32], data: &[u8]) -> CoreResult<()> {
        self.0.transact(code, params, Some(data))?;
        Ok(())
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        debug!("Closing PTP session");
        if let Err(e) = self.0.transact(CommandCode::CloseSession, &[], None) {
            error!("Failed to close PTP session: {e}");
        }
    }
}

struct Interface {
    handle: rusb::DeviceHandle<GlobalContext>,
    number: u8,
    bulk_in: u8,
    bulk_out: u8,
    transaction_id: u32,
    chunk_size: usize,
}

impl Interface {
    fn transact(
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
            p.try_write_ptp(&mut payload).map_err(PtpError::from)?;
        }
        self.write_container(ContainerType::Command, code, &payload, transaction_id)?;

        if let Some(data) = data {
            trace!(
                "PTP tx={transaction_id}: sending payload ({} bytes)",
                data.len()
            );
            self.write_container(ContainerType::Data, code, data, transaction_id)?;
        }

        let response = {
            let mut response: CoreResult<Vec<u8>> = Ok(Vec::new());
            loop {
                trace!("PTP tx={transaction_id}: receiving response");
                let (container, payload) = self.read_container()?;

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
                                response = Err(PtpError::Malformed(format!(
                                    "response container carried command code {cmd:?}"
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

    fn write_container(
        &self,
        kind: ContainerType,
        code: CommandCode,
        payload: &[u8],
        transaction_id: u32,
    ) -> CoreResult<()> {
        let container_info = ContainerInfo::new(kind, code, transaction_id, payload.len())?;
        let mut buffer: Vec<u8> = container_info.try_into_ptp().map_err(PtpError::from)?;

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

    fn read_container(&self) -> CoreResult<(ContainerInfo, Vec<u8>)> {
        let mut stack_buf = [0u8; 8 * 1024];

        let n = self
            .handle
            .read_bulk(self.bulk_in, &mut stack_buf, Duration::ZERO)
            .map_err(PtpError::Transport)?;
        let buf = &stack_buf[..n];
        trace!("PTP read: initial chunk ({n} bytes)");

        let mut cur = Cursor::new(buf);
        let container_info = ContainerInfo::try_read_ptp(&mut cur).map_err(PtpError::from)?;

        if (container_info.total_len as usize) < ContainerInfo::SIZE {
            return Err(PtpError::Malformed(format!(
                "container total length {} is below the {}-byte header",
                container_info.total_len,
                ContainerInfo::SIZE
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
}

impl Drop for Interface {
    fn drop(&mut self) {
        if let Err(e) = self.handle.release_interface(self.number) {
            error!("Failed to release USB interface: {e}");
        }
    }
}
