use std::{
    cmp::min,
    fmt::{Display, Formatter},
    io::Cursor,
    str::FromStr,
    time::Duration,
};

use anyhow::anyhow;
use log::{debug, error, trace, warn};
use ptp_cursor::{PtpDeserialize, PtpSerialize};
use rusb::{GlobalContext, constants::LIBUSB_CLASS_IMAGE};

use crate::ptp::{
    CommandCode, ContainerCode, ContainerInfo, ContainerType, ResponseCode, error,
    transport::PtpTransport,
};

/// Default conservative estimate, overridden by the camera definition.
const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

/// `-d <BUS>.<ADDRESS>` device selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub bus: u8,
    pub address: u8,
}

impl FromStr for Location {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (bus, address) = s
            .split_once('.')
            .ok_or_else(|| anyhow!("Invalid device format: {s}, expected <BUS>.<ADDRESS>"))?;

        Ok(Self {
            bus: bus
                .parse()
                .map_err(|_| anyhow!("Invalid bus number: {bus}"))?,
            address: address
                .parse()
                .map_err(|_| anyhow!("Invalid address: {address}"))?,
        })
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.bus, self.address)
    }
}

pub struct LibusbDevice {
    pub device: rusb::Device<GlobalContext>,
    pub location: Location,
    pub vendor: u16,
    pub product: u16,
}

impl LibusbDevice {
    #[must_use]
    pub fn id(&self) -> String {
        self.location.to_string()
    }

    pub fn open(&self) -> anyhow::Result<Libusb> {
        Libusb::open(&self.device, self.location)
    }
}

pub fn enumerate() -> anyhow::Result<Vec<LibusbDevice>> {
    let mut devices = Vec::new();

    for device in rusb::devices()?.iter() {
        trace!("Found USB device {device:x?}");

        let descriptor = match device.device_descriptor() {
            Ok(descriptor) => descriptor,
            Err(e) => {
                trace!("Failed to read descriptor of {device:x?}: {e}");
                continue;
            }
        };

        devices.push(LibusbDevice {
            location: Location {
                bus: device.bus_number(),
                address: device.address(),
            },
            vendor: descriptor.vendor_id(),
            product: descriptor.product_id(),
            device,
        });
    }

    Ok(devices)
}

pub struct Libusb {
    pub location: Location,
    pub interface: u8,
    pub bulk_in: u8,
    pub bulk_out: u8,
    pub handle: rusb::DeviceHandle<GlobalContext>,
    pub transaction_id: u32,
    pub chunk_size: usize,
}

impl Libusb {
    fn open(device: &rusb::Device<GlobalContext>, location: Location) -> anyhow::Result<Self> {
        let config_descriptor = device.active_config_descriptor()?;
        let interface_descriptor = config_descriptor
            .interfaces()
            .flat_map(|i| i.descriptors())
            .find(|x| x.class_code() == LIBUSB_CLASS_IMAGE)
            .ok_or(rusb::Error::NotFound)?;

        let interface = interface_descriptor.interface_number();
        debug!("Found interface {interface}");

        let handle = device.open()?;
        handle.claim_interface(interface)?;
        debug!("Claimed interface");

        let find_endpoint = |direction: rusb::Direction,
                             transfer_type: rusb::TransferType|
         -> Result<u8, rusb::Error> {
            interface_descriptor
                .endpoint_descriptors()
                .find(|ep| ep.direction() == direction && ep.transfer_type() == transfer_type)
                .map(|x| x.address())
                .ok_or(rusb::Error::NotFound)
        };

        let bulk_in = find_endpoint(rusb::Direction::In, rusb::TransferType::Bulk)?;
        debug!("Found Bulk In endpoint");

        let bulk_out = find_endpoint(rusb::Direction::Out, rusb::TransferType::Bulk)?;
        debug!("Found Bulk Out endpoint");

        Ok(Self {
            location,
            interface,
            bulk_in,
            bulk_out,
            handle,
            transaction_id: 0,
            chunk_size: DEFAULT_CHUNK_SIZE,
        })
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
}

impl PtpTransport for Libusb {
    fn transact(
        &mut self,
        code: u16,
        params: &[u32],
        data: Option<&[u8]>,
    ) -> anyhow::Result<Vec<u8>> {
        let code = CommandCode::try_from(code)
            .map_err(|_| anyhow!("Unsupported PTP operation code 0x{code:04x}"))?;
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

    fn open_session(&mut self, session_id: u32) -> anyhow::Result<()> {
        debug!("Opening PTP session");
        self.transact(CommandCode::OpenSession.into(), &[session_id], None)?;
        Ok(())
    }

    fn close_session(&mut self, _session_id: u32) -> anyhow::Result<()> {
        debug!("Closing PTP session");
        self.transact(CommandCode::CloseSession.into(), &[], None)?;
        Ok(())
    }

    fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    fn set_chunk_size(&mut self, chunk_size: usize) {
        if chunk_size <= ContainerInfo::SIZE {
            warn!("Ignoring implausible chunk size {chunk_size}");
            return;
        }
        self.chunk_size = chunk_size;
    }

    fn id(&self) -> String {
        self.location.to_string()
    }
}

impl Drop for Libusb {
    fn drop(&mut self) {
        if let Err(e) = self.handle.release_interface(self.interface) {
            error!("Failed to release USB interface: {e}");
        }
    }
}
