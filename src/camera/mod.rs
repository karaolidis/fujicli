pub mod devices;
pub mod error;
pub mod ptp;

use std::{cmp::min, io::Cursor, time::Duration};

use anyhow::{anyhow, bail};
use byteorder::{LittleEndian, WriteBytesExt};
use devices::SupportedCamera;
use log::{debug, error, trace};
use ptp::{
    enums::{CommandCode, ContainerType, PropCode, ResponseCode, UsbMode},
    structs::{ContainerInfo, DeviceInfo},
};
use rusb::{GlobalContext, constants::LIBUSB_CLASS_IMAGE};

const SESSION: u32 = 1;

pub struct Usb {
    bus: u8,
    address: u8,
    interface: u8,
}

pub struct Ptp {
    bulk_in: u8,
    bulk_out: u8,
    handle: rusb::DeviceHandle<GlobalContext>,
    transaction_id: u32,
}

pub struct Camera {
    pub r#impl: Box<dyn CameraImpl<GlobalContext>>,
    usb: Usb,
    pub ptp: Ptp,
}

impl Camera {
    pub fn from_device(device: &rusb::Device<GlobalContext>) -> anyhow::Result<Self> {
        for supported_camera in devices::SUPPORTED {
            if let Ok(r#impl) = supported_camera.new_camera(device) {
                let bus = device.bus_number();
                let address = device.address();

                let config_desc = device.active_config_descriptor()?;

                let interface_descriptor = config_desc
                    .interfaces()
                    .flat_map(|i| i.descriptors())
                    .find(|x| x.class_code() == LIBUSB_CLASS_IMAGE)
                    .ok_or(rusb::Error::NotFound)?;

                let interface = interface_descriptor.interface_number();
                debug!("Found interface {interface}");

                let usb = Usb {
                    bus,
                    address,
                    interface,
                };

                let handle = device.open()?;
                handle.claim_interface(interface)?;

                let bulk_in = Self::find_endpoint(
                    &interface_descriptor,
                    rusb::Direction::In,
                    rusb::TransferType::Bulk,
                )?;
                let bulk_out = Self::find_endpoint(
                    &interface_descriptor,
                    rusb::Direction::Out,
                    rusb::TransferType::Bulk,
                )?;
                let transaction_id = 0;

                let mut ptp = Ptp {
                    bulk_in,
                    bulk_out,
                    handle,
                    transaction_id,
                };

                debug!("Opening session");
                let () = r#impl.open_session(&mut ptp, SESSION)?;
                debug!("Session opened");

                return Ok(Self { r#impl, usb, ptp });
            }
        }

        bail!("Device not supported");
    }

    fn find_endpoint(
        interface_descriptor: &rusb::InterfaceDescriptor<'_>,
        direction: rusb::Direction,
        transfer_type: rusb::TransferType,
    ) -> Result<u8, rusb::Error> {
        interface_descriptor
            .endpoint_descriptors()
            .find(|ep| ep.direction() == direction && ep.transfer_type() == transfer_type)
            .map(|x| x.address())
            .ok_or(rusb::Error::NotFound)
    }

    pub fn name(&self) -> &'static str {
        self.r#impl.supported_camera().name
    }

    pub fn vendor_id(&self) -> u16 {
        self.r#impl.supported_camera().vendor
    }

    pub fn product_id(&self) -> u16 {
        self.r#impl.supported_camera().product
    }

    pub fn connected_usb_id(&self) -> String {
        format!("{}.{}", self.usb.bus, self.usb.address)
    }

    fn prop_value_as_scalar(data: &[u8]) -> anyhow::Result<u32> {
        let data = match data.len() {
            1 => anyhow::Ok(u32::from(data[0])),
            2 => anyhow::Ok(u32::from(u16::from_le_bytes([data[0], data[1]]))),
            4 => anyhow::Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]])),
            n => bail!("Cannot parse {n} bytes as scalar"),
        }?;

        Ok(data)
    }

    pub fn get_info(&mut self) -> anyhow::Result<DeviceInfo> {
        let info = self.r#impl.get_info(&mut self.ptp)?;
        Ok(info)
    }

    pub fn get_usb_mode(&mut self) -> anyhow::Result<UsbMode> {
        let data = self
            .r#impl
            .get_prop_value(&mut self.ptp, PropCode::FujiUsbMode);

        let result = Self::prop_value_as_scalar(&data?)?.into();
        Ok(result)
    }

    pub fn get_battery_info(&mut self) -> anyhow::Result<u32> {
        let data = self
            .r#impl
            .get_prop_value(&mut self.ptp, PropCode::FujiBatteryInfo2);

        let data = data?;
        debug!("Raw battery data: {data:?}");

        let utf16: Vec<u16> = data[1..]
            .chunks(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .take_while(|&c| c != 0)
            .collect();

        let utf8_string = String::from_utf16(&utf16)?;
        debug!("Decoded UTF-16 string: {utf8_string}");

        let percentage: u32 = utf8_string
            .split(',')
            .next()
            .ok_or_else(|| anyhow!("Failed to parse battery percentage"))?
            .parse()?;

        Ok(percentage)
    }

    pub fn export_backup(&mut self) -> anyhow::Result<Vec<u8>> {
        self.r#impl.export_backup(&mut self.ptp)
    }

    pub fn import_backup(&mut self, backup: &[u8]) -> anyhow::Result<()> {
        self.r#impl.import_backup(&mut self.ptp, backup)
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        debug!("Closing session");
        if let Err(e) = self.r#impl.close_session(&mut self.ptp, SESSION) {
            error!("Error closing session: {e}");
        }
        debug!("Session closed");

        debug!("Releasing interface");
        if let Err(e) = self.ptp.handle.release_interface(self.usb.interface) {
            error!("Error releasing interface: {e}");
        }
        debug!("Interface released");
    }
}

pub trait CameraImpl<P: rusb::UsbContext> {
    fn supported_camera(&self) -> &'static SupportedCamera<P>;

    fn timeout(&self) -> Option<Duration> {
        None
    }

    fn chunk_size(&self) -> usize {
        1024 * 1024
    }

    fn send(
        &self,
        ptp: &mut Ptp,
        code: CommandCode,
        params: Option<&[u32]>,
        data: Option<&[u8]>,
        transaction: bool,
    ) -> anyhow::Result<Vec<u8>> {
        let transaction_id = if transaction {
            Some(ptp.transaction_id)
        } else {
            None
        };

        let params = params.unwrap_or_default();

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
        self.write(ptp, ContainerType::Command, code, &payload, transaction_id)?;

        if let Some(data) = data {
            trace!("Sending PTP data: {} bytes", data.len());
            self.write(ptp, ContainerType::Data, code, data, transaction_id)?;
        }

        let mut data_payload = Vec::new();
        loop {
            let (container, payload) = self.read(ptp)?;
            match container.kind {
                ContainerType::Data => {
                    trace!("Data received: {} bytes", payload.len());
                    data_payload = payload;
                }
                ContainerType::Response => {
                    trace!("Response received: code {:?}", container.code);
                    let code = ResponseCode::try_from(container.code)?;
                    if code != ResponseCode::Ok {
                        bail!(ptp::error::Error::Response(container.code));
                    }
                    trace!(
                        "Command {:?} completed successfully with data payload of {} bytes",
                        code,
                        data_payload.len(),
                    );
                    return Ok(data_payload);
                }
                _ => {
                    debug!("Ignoring unexpected container type: {:?}", container.kind);
                }
            }
        }
    }

    fn write(
        &self,
        ptp: &mut Ptp,
        kind: ContainerType,
        code: CommandCode,
        payload: &[u8],
        // Fuji, for the love of God don't ever write code again.
        transaction_id: Option<u32>,
    ) -> anyhow::Result<()> {
        // Look at what you made me do. Fuck.
        let header_len = ContainerInfo::SIZE
            - if transaction_id.is_none() {
                size_of::<u32>()
            } else {
                0
            };

        let first_chunk_len = min(payload.len(), self.chunk_size() - header_len);
        let total_len = u32::try_from(payload.len() + header_len)?;

        let mut buffer = Vec::with_capacity(first_chunk_len + header_len);
        buffer.write_u32::<LittleEndian>(total_len)?;
        buffer.write_u16::<LittleEndian>(kind as u16)?;
        buffer.write_u16::<LittleEndian>(code as u16)?;
        if let Some(transaction_id) = transaction_id {
            buffer.write_u32::<LittleEndian>(transaction_id)?;
        }

        buffer.extend_from_slice(&payload[..first_chunk_len]);

        trace!(
            "Writing PTP {kind:?} container, code: {code:?}, transaction: {transaction_id:?}, first_chunk: {first_chunk_len} bytes",
        );

        let timeout = self.timeout().unwrap_or_default();
        ptp.handle.write_bulk(ptp.bulk_out, &buffer, timeout)?;

        for chunk in payload[first_chunk_len..].chunks(self.chunk_size()) {
            trace!("Writing additional chunk ({} bytes)", chunk.len(),);
            ptp.handle.write_bulk(ptp.bulk_out, chunk, timeout)?;
        }

        trace!(
            "Write completed for code {:?}, total payload of {} bytes",
            code,
            payload.len()
        );
        Ok(())
    }

    fn read(&self, ptp: &mut Ptp) -> anyhow::Result<(ContainerInfo, Vec<u8>)> {
        let timeout = self.timeout().unwrap_or_default();

        let mut stack_buf = [0u8; 8 * 1024];
        let n = ptp.handle.read_bulk(ptp.bulk_in, &mut stack_buf, timeout)?;
        let buf = &stack_buf[..n];

        trace!("Read {n} bytes from bulk_in");

        let container_info = ContainerInfo::parse(buf)?;
        if container_info.payload_len == 0 {
            trace!("No payload in container");
            return Ok((container_info, Vec::new()));
        }

        let payload_len = container_info.payload_len as usize;
        let mut payload = Vec::with_capacity(payload_len);
        if buf.len() > ContainerInfo::SIZE {
            payload.extend_from_slice(&buf[ContainerInfo::SIZE..]);
        }

        while payload.len() < payload_len {
            let remaining = payload_len - payload.len();
            let mut chunk = vec![0u8; min(remaining, self.chunk_size())];
            let n = ptp.handle.read_bulk(ptp.bulk_in, &mut chunk, timeout)?;
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

    fn open_session(&self, ptp: &mut Ptp, session_id: u32) -> anyhow::Result<()> {
        debug!("Sending OpenSession command");
        _ = self.send(
            ptp,
            CommandCode::OpenSession,
            Some(&[session_id]),
            None,
            true,
        )?;
        Ok(())
    }

    fn close_session(&self, ptp: &mut Ptp, _: u32) -> anyhow::Result<()> {
        debug!("Sending CloseSession command");
        _ = self.send(ptp, CommandCode::CloseSession, None, None, true)?;
        Ok(())
    }

    fn get_info(&self, ptp: &mut Ptp) -> anyhow::Result<DeviceInfo> {
        debug!("Sending GetDeviceInfo command");
        let response = self.send(ptp, CommandCode::GetDeviceInfo, None, None, true)?;
        debug!("Received response with {} bytes", response.len());
        let info = DeviceInfo::try_from(response.as_slice())?;
        Ok(info)
    }

    fn get_prop_value(&self, ptp: &mut Ptp, prop: PropCode) -> anyhow::Result<Vec<u8>> {
        debug!("Sending GetDevicePropValue command for property {prop:?}");
        let response = self.send(
            ptp,
            CommandCode::GetDevicePropValue,
            Some(&[prop as u32]),
            None,
            true,
        )?;
        debug!("Received response with {} bytes", response.len());
        Ok(response)
    }

    fn export_backup(&self, ptp: &mut Ptp) -> anyhow::Result<Vec<u8>> {
        const HANDLE: u32 = 0x0;

        debug!("Sending GetObjectInfo command for backup");
        let response = self.send(ptp, CommandCode::GetObjectInfo, Some(&[HANDLE]), None, true)?;
        debug!("Received response with {} bytes", response.len());

        debug!("Sending GetObject command for backup");
        let response = self.send(ptp, CommandCode::GetObject, Some(&[HANDLE]), None, true)?;
        debug!("Received response with {} bytes", response.len());

        Ok(response)
    }

    fn import_backup(&self, ptp: &mut Ptp, buffer: &[u8]) -> anyhow::Result<()> {
        debug!("Preparing ObjectInfo header for backup");
        let mut obj_info = vec![0u8; 1088];
        let mut cursor = Cursor::new(&mut obj_info[..]);
        cursor.write_u32::<LittleEndian>(0x0)?;
        cursor.write_u16::<LittleEndian>(0x5000)?;
        cursor.write_u16::<LittleEndian>(0x0)?;
        cursor.write_u32::<LittleEndian>(u32::try_from(buffer.len())?)?;

        debug!("Sending SendObjectInfo command for backup");
        let response = self.send(
            ptp,
            CommandCode::SendObjectInfo,
            Some(&[0x0, 0x0]),
            Some(&obj_info),
            true,
        )?;
        debug!("Received response with {} bytes", response.len());

        debug!("Sending SendObject command for backup");
        let response = self.send(
            ptp,
            CommandCode::SendObject,
            Some(&[0x0]),
            Some(buffer),
            false,
        )?;
        debug!("Received response with {} bytes", response.len());

        Ok(())
    }
}
