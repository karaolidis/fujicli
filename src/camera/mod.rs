pub mod devices;
pub mod error;
pub mod ptp;

use std::{io::Cursor, time::Duration};

use anyhow::{anyhow, bail};
use byteorder::{LittleEndian, WriteBytesExt};
use devices::SupportedCamera;
use log::{debug, error};
use ptp::{
    Ptp,
    enums::{CommandCode, PropCode, UsbMode},
    structs::DeviceInfo,
};
use rusb::{GlobalContext, constants::LIBUSB_CLASS_IMAGE};

const SESSION: u32 = 1;

pub struct Camera {
    r#impl: Box<dyn CameraImpl<GlobalContext>>,
    ptp: Ptp,
}

impl Camera {
    pub fn from_device(device: &rusb::Device<GlobalContext>) -> anyhow::Result<Self> {
        for supported_camera in devices::SUPPORTED {
            if let Ok(r#impl) = supported_camera.new_camera(device) {
                let bus = device.bus_number();
                let address = device.address();

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

                let chunk_size = r#impl.chunk_size();

                let mut ptp = Ptp {
                    bus,
                    address,
                    interface,
                    bulk_in,
                    bulk_out,
                    handle,
                    transaction_id,
                    chunk_size,
                };

                debug!("Opening session");
                let () = r#impl.open_session(&mut ptp, SESSION)?;
                debug!("Session opened");

                return Ok(Self { r#impl, ptp });
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
        format!("{}.{}", self.ptp.bus, self.ptp.address)
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
    }
}

pub trait CameraImpl<P: rusb::UsbContext> {
    fn supported_camera(&self) -> &'static SupportedCamera<P>;

    fn timeout(&self) -> Duration {
        Duration::default()
    }

    fn chunk_size(&self) -> usize {
        1024 * 1024
    }

    fn open_session(&self, ptp: &mut Ptp, session_id: u32) -> anyhow::Result<()> {
        debug!("Sending OpenSession command");
        _ = ptp.send(
            CommandCode::OpenSession,
            Some(&[session_id]),
            None,
            true,
            self.timeout(),
        )?;
        Ok(())
    }

    fn close_session(&self, ptp: &mut Ptp, _: u32) -> anyhow::Result<()> {
        debug!("Sending CloseSession command");
        _ = ptp.send(CommandCode::CloseSession, None, None, true, self.timeout())?;
        Ok(())
    }

    fn get_info(&self, ptp: &mut Ptp) -> anyhow::Result<DeviceInfo> {
        debug!("Sending GetDeviceInfo command");
        let response = ptp.send(CommandCode::GetDeviceInfo, None, None, true, self.timeout())?;
        debug!("Received response with {} bytes", response.len());
        let info = DeviceInfo::try_from(response.as_slice())?;
        Ok(info)
    }

    fn get_prop_value(&self, ptp: &mut Ptp, prop: PropCode) -> anyhow::Result<Vec<u8>> {
        debug!("Sending GetDevicePropValue command for property {prop:?}");
        let response = ptp.send(
            CommandCode::GetDevicePropValue,
            Some(&[prop as u32]),
            None,
            true,
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());
        Ok(response)
    }

    fn export_backup(&self, ptp: &mut Ptp) -> anyhow::Result<Vec<u8>> {
        const HANDLE: u32 = 0x0;

        debug!("Sending GetObjectInfo command for backup");
        let response = ptp.send(
            CommandCode::GetObjectInfo,
            Some(&[HANDLE]),
            None,
            true,
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());

        debug!("Sending GetObject command for backup");
        let response = ptp.send(
            CommandCode::GetObject,
            Some(&[HANDLE]),
            None,
            true,
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());

        Ok(response)
    }

    fn import_backup(&self, ptp: &mut Ptp, buffer: &[u8]) -> anyhow::Result<()> {
        debug!("Preparing ObjectInfo header for backup");

        let mut header1 = vec![0u8; 1012];
        let mut cursor = Cursor::new(&mut header1[..]);
        cursor.write_u32::<LittleEndian>(0x0)?;
        cursor.write_u16::<LittleEndian>(0x5000)?;
        cursor.write_u16::<LittleEndian>(0x0)?;
        cursor.write_u32::<LittleEndian>(u32::try_from(buffer.len())?)?;

        let header2 = vec![0u8; 64];

        debug!("Sending SendObjectInfo command for backup");
        let response = ptp.send_many(
            CommandCode::SendObjectInfo,
            Some(&[0x0, 0x0]),
            Some(&[&header1, &header2]),
            true,
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());

        debug!("Sending SendObject command for backup");
        let response = ptp.send(
            CommandCode::SendObject,
            Some(&[0x0]),
            Some(buffer),
            true,
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());

        Ok(())
    }
}
