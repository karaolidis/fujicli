use std::{error::Error, fmt, time::Duration};

use libptp::{DeviceInfo, StandardCommandCode};
use log::debug;
use rusb::{DeviceDescriptor, GlobalContext};
use serde::Serialize;

mod xt5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraId {
    pub name: &'static str,
    pub vendor: u16,
    pub product: u16,
}

pub struct SupportedCamera {
    pub id: CameraId,
    pub factory: fn(
        rusb::Device<GlobalContext>,
    ) -> Result<Box<dyn CameraImpl>, Box<dyn Error + Send + Sync>>,
}

pub const SUPPORTED_CAMERAS: &[SupportedCamera] = &[SupportedCamera {
    id: xt5::FUJIFILM_XT5,
    factory: |d| xt5::FujifilmXT5::new_boxed(d),
}];

impl SupportedCamera {
    pub fn matches_descriptor(&self, descriptor: &DeviceDescriptor) -> bool {
        descriptor.vendor_id() == self.id.vendor && descriptor.product_id() == self.id.product
    }
}

pub const TIMEOUT: Duration = Duration::from_millis(500);

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum DevicePropCode {
    FujiUsbMode = 0xd16e,
    FujiBatteryInfo2 = 0xD36B,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum FujiUsbMode {
    RawConversion, // mode == 6
    Unsupported,
}

impl From<u32> for FujiUsbMode {
    fn from(val: u32) -> Self {
        match val {
            6 => FujiUsbMode::RawConversion,
            _ => FujiUsbMode::Unsupported,
        }
    }
}

impl fmt::Display for FujiUsbMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FujiUsbMode::RawConversion => "USB RAW CONV./BACKUP RESTORE",
            FujiUsbMode::Unsupported => "Unsupported USB Mode",
        };
        write!(f, "{}", s)
    }
}

pub trait CameraImpl {
    fn id(&self) -> &'static CameraId;

    fn usb_id(&self) -> String;

    fn ptp(&self) -> libptp::Camera<GlobalContext>;

    fn get_info(&self) -> Result<DeviceInfo, Box<dyn Error + Send + Sync>> {
        debug!("Sending GetDeviceInfo command");
        let response =
            self.ptp()
                .command(StandardCommandCode::GetDeviceInfo, &[], None, Some(TIMEOUT))?;
        debug!("Received response with {} bytes", response.len());

        let info = DeviceInfo::decode(&response)?;
        Ok(info)
    }

    fn next_session_id(&self) -> u32;

    fn open_session(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let session_id = self.next_session_id();

        debug!("Opening new session with id {}", session_id);
        self.ptp().command(
            StandardCommandCode::OpenSession,
            &[session_id],
            None,
            Some(TIMEOUT),
        )?;

        Ok(())
    }

    fn close_session(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        debug!("Closing session");
        self.ptp()
            .command(StandardCommandCode::CloseSession, &[], None, Some(TIMEOUT))?;

        Ok(())
    }

    fn get_prop_value_raw(
        &self,
        prop: DevicePropCode,
    ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        self.open_session()?;

        debug!("Getting property {:?}", prop);

        let response = self.ptp().command(
            StandardCommandCode::GetDevicePropValue,
            &[prop as u32],
            None,
            Some(TIMEOUT),
        );

        self.close_session()?;

        let response = response?;
        debug!("Received response with {} bytes", response.len());

        Ok(response)
    }

    fn get_prop_value_scalar(
        &self,
        prop: DevicePropCode,
    ) -> Result<u32, Box<dyn Error + Send + Sync>> {
        let data = self.get_prop_value_raw(prop)?;

        match data.len() {
            1 => Ok(data[0] as u32),
            2 => Ok(u16::from_le_bytes([data[0], data[1]]) as u32),
            4 => Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]])),
            n => Err(format!("Cannot parse property {:?} as scalar: {} bytes", prop, n).into()),
        }
    }

    fn get_fuji_usb_mode(&self) -> Result<FujiUsbMode, Box<dyn Error + Send + Sync>> {
        let result = self.get_prop_value_scalar(DevicePropCode::FujiUsbMode)?;
        Ok(result.into())
    }

    fn get_fuji_battery_info(&self) -> Result<u32, Box<dyn Error + Send + Sync>> {
        let data = self.get_prop_value_raw(DevicePropCode::FujiBatteryInfo2)?;
        debug!("Raw battery data: {:?}", data);

        if data.len() < 3 {
            return Err("Battery info payload too short".into());
        }

        let utf16: Vec<u16> = data[1..]
            .chunks(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .take_while(|&c| c != 0)
            .collect();

        debug!("Decoded UTF-16 units: {:?}", utf16);

        let utf8_string = String::from_utf16(&utf16)?;
        debug!("Decoded UTF-16 string: {}", utf8_string);

        let percentage: u32 = utf8_string
            .split(',')
            .next()
            .ok_or("Failed to parse battery percentage")?
            .parse()?;

        Ok(percentage)
    }
}
