pub mod devices;

use std::{error::Error, fmt, io::Cursor, time::Duration};

use anyhow::{anyhow, bail};
use byteorder::{LittleEndian, WriteBytesExt};
use devices::SupportedCamera;
use libptp::{DeviceInfo, StandardCommandCode};
use log::{debug, error};
use rusb::GlobalContext;
use serde::Serialize;

#[derive(Debug)]
pub struct UnsupportedFeatureError;

impl fmt::Display for UnsupportedFeatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "feature is not supported for this device")
    }
}

impl Error for UnsupportedFeatureError {}

const SESSION: u32 = 1;

pub struct Camera {
    bus: u8,
    address: u8,
    ptp: libptp::Camera<GlobalContext>,
    r#impl: Box<dyn CameraImpl<GlobalContext>>,
}

impl Camera {
    pub fn from_device(device: &rusb::Device<GlobalContext>) -> anyhow::Result<Self> {
        for supported_camera in devices::SUPPORTED {
            if let Ok(r#impl) = supported_camera.new_camera(device) {
                let bus = device.bus_number();
                let address = device.address();
                let mut ptp = libptp::Camera::new(device)?;

                debug!("Opening session");
                let () = r#impl.open_session(&mut ptp, SESSION)?;
                debug!("Session opened");

                return Ok(Self {
                    bus,
                    address,
                    ptp,
                    r#impl,
                });
            }
        }

        bail!("Device not supported");
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
        format!("{}.{}", self.bus, self.address)
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
            .get_prop_value(&mut self.ptp, DevicePropCode::FujiUsbMode);

        let result = Self::prop_value_as_scalar(&data?)?.into();
        Ok(result)
    }

    pub fn get_battery_info(&mut self) -> anyhow::Result<u32> {
        let data = self
            .r#impl
            .get_prop_value(&mut self.ptp, DevicePropCode::FujiBatteryInfo2);

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
            error!("Error closing session: {e}")
        }
        debug!("Session closed");
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum DevicePropCode {
    FujiUsbMode = 0xd16e,
    FujiBatteryInfo2 = 0xD36B,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum UsbMode {
    RawConversion,
    Unsupported,
}

impl From<u32> for UsbMode {
    fn from(val: u32) -> Self {
        match val {
            6 => Self::RawConversion,
            _ => Self::Unsupported,
        }
    }
}

impl fmt::Display for UsbMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::RawConversion => "USB RAW CONV./BACKUP RESTORE",
            Self::Unsupported => "Unsupported USB Mode",
        };
        write!(f, "{s}")
    }
}

pub trait CameraImpl<P: rusb::UsbContext> {
    fn supported_camera(&self) -> &'static SupportedCamera<P>;

    fn timeout(&self) -> Option<Duration> {
        None
    }

    fn open_session(&self, ptp: &mut libptp::Camera<P>, session_id: u32) -> anyhow::Result<()> {
        debug!("Sending OpenSession command");
        _ = ptp.command(
            StandardCommandCode::OpenSession,
            &[session_id],
            None,
            self.timeout(),
        )?;
        Ok(())
    }

    fn close_session(&self, ptp: &mut libptp::Camera<P>, _: u32) -> anyhow::Result<()> {
        debug!("Sending CloseSession command");
        let _ = ptp.command(StandardCommandCode::CloseSession, &[], None, self.timeout())?;
        Ok(())
    }

    fn get_info(&self, ptp: &mut libptp::Camera<P>) -> anyhow::Result<DeviceInfo> {
        debug!("Sending GetDeviceInfo command");
        let response = ptp.command(
            StandardCommandCode::GetDeviceInfo,
            &[],
            None,
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());
        let info = DeviceInfo::decode(&response)?;
        Ok(info)
    }

    fn get_prop_value(
        &self,
        ptp: &mut libptp::Camera<P>,
        prop: DevicePropCode,
    ) -> anyhow::Result<Vec<u8>> {
        debug!("Sending GetDevicePropValue command for property {prop:?}");
        let response = ptp.command(
            StandardCommandCode::GetDevicePropValue,
            &[prop as u32],
            None,
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());
        Ok(response)
    }

    fn export_backup(&self, ptp: &mut libptp::Camera<P>) -> anyhow::Result<Vec<u8>> {
        const HANDLE: u32 = 0x0;

        debug!("Sending GetObjectInfo command for backup");
        let response = ptp.command(
            StandardCommandCode::GetObjectInfo,
            &[HANDLE],
            None,
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());

        debug!("Sending GetObject command for backup");
        let response = ptp.command(
            StandardCommandCode::GetObject,
            &[HANDLE],
            None,
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());

        Ok(response)
    }

    fn import_backup(&self, ptp: &mut libptp::Camera<P>, buffer: &[u8]) -> anyhow::Result<()> {
        debug!("Preparing ObjectInfo header for backup");

        let mut obj_info = vec![0u8; 1012];

        let mut cursor = Cursor::new(&mut obj_info[..]);
        cursor.write_u32::<LittleEndian>(0x0)?;
        cursor.write_u16::<LittleEndian>(0x5000)?;
        cursor.write_u16::<LittleEndian>(0x0)?;
        cursor.write_u32::<LittleEndian>(u32::try_from(buffer.len())?)?;

        debug!("Sending SendObjectInfo command for backup");
        let response = ptp.command(
            libptp::StandardCommandCode::SendObjectInfo,
            &[0x0, 0x0],
            Some(&obj_info),
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());

        debug!("Sending SendObject command for backup");
        let response = ptp.command(
            libptp::StandardCommandCode::SendObject,
            &[0x0],
            Some(buffer),
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());

        Ok(())
    }
}
