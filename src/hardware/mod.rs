use std::{
    fmt,
    ops::{Deref, DerefMut},
    time::Duration,
};

use anyhow::bail;
use libptp::{DeviceInfo, StandardCommandCode};
use log::{debug, error};
use rusb::{DeviceDescriptor, GlobalContext};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraId {
    pub name: &'static str,
    pub vendor: u16,
    pub product: u16,
}

type CameraFactory = fn(rusb::Device<GlobalContext>) -> Result<Box<dyn CameraImpl>, anyhow::Error>;

pub struct SupportedCamera {
    pub id: CameraId,
    pub factory: CameraFactory,
}

pub const SUPPORTED_CAMERAS: &[SupportedCamera] = &[SupportedCamera {
    id: FUJIFILM_XT5,
    factory: |d| FujifilmXT5::new_boxed(&d),
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

pub struct Ptp {
    ptp: libptp::Camera<GlobalContext>,
}

impl Deref for Ptp {
    type Target = libptp::Camera<GlobalContext>;

    fn deref(&self) -> &Self::Target {
        &self.ptp
    }
}

impl DerefMut for Ptp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ptp
    }
}

impl From<libptp::Camera<GlobalContext>> for Ptp {
    fn from(ptp: libptp::Camera<GlobalContext>) -> Self {
        Self { ptp }
    }
}

type SessionCloseFn =
    Box<dyn FnOnce(u32, &mut libptp::Camera<GlobalContext>) -> Result<(), anyhow::Error>>;

pub struct PtpSession {
    ptp: libptp::Camera<GlobalContext>,
    session_id: u32,
    close_fn: Option<SessionCloseFn>,
}

impl Deref for PtpSession {
    type Target = libptp::Camera<GlobalContext>;

    fn deref(&self) -> &Self::Target {
        &self.ptp
    }
}

impl DerefMut for PtpSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ptp
    }
}

impl Drop for PtpSession {
    fn drop(&mut self) {
        if let Some(close_fn) = self.close_fn.take() {
            if let Err(e) = close_fn(self.session_id, &mut self.ptp) {
                error!("Error closing session {}: {}", self.session_id, e);
            }
        }
    }
}

pub trait CameraImpl {
    fn id(&self) -> &'static CameraId;

    fn device(&self) -> &rusb::Device<rusb::GlobalContext>;

    fn usb_id(&self) -> String {
        let bus = self.device().bus_number();
        let address = self.device().address();
        format!("{bus}.{address}")
    }

    fn ptp(&self) -> Ptp;

    fn ptp_session(&self) -> Result<PtpSession, anyhow::Error>;

    fn get_info(&self, ptp: &mut Ptp) -> Result<DeviceInfo, anyhow::Error> {
        debug!("Sending GetDeviceInfo command");
        let response = ptp.command(StandardCommandCode::GetDeviceInfo, &[], None, Some(TIMEOUT))?;
        debug!("Received response with {} bytes", response.len());

        let info = DeviceInfo::decode(&response)?;
        Ok(info)
    }

    fn next_session_id(&self) -> u32;

    fn open_session(&self, ptp: Ptp) -> Result<PtpSession, anyhow::Error> {
        let session_id = self.next_session_id();
        let mut ptp = ptp.ptp;

        debug!("Opening session with id {session_id}");
        ptp.command(
            StandardCommandCode::OpenSession,
            &[session_id],
            None,
            Some(TIMEOUT),
        )?;
        debug!("Session {session_id} open");

        let close_fn: Option<SessionCloseFn> = Some(Box::new(move |_, ptp| {
            debug!("Closing session with id {session_id}");
            ptp.command(StandardCommandCode::CloseSession, &[], None, Some(TIMEOUT))?;
            debug!("Session {session_id} closed");
            Ok(())
        }));

        Ok(PtpSession {
            ptp,
            session_id,
            close_fn,
        })
    }

    fn get_prop_value_raw(
        &self,
        ptp: &mut PtpSession,
        prop: DevicePropCode,
    ) -> Result<Vec<u8>, anyhow::Error> {
        debug!("Getting property {prop:?}");

        let response = ptp.command(
            StandardCommandCode::GetDevicePropValue,
            &[prop as u32],
            None,
            Some(TIMEOUT),
        )?;

        debug!("Received response with {} bytes", response.len());

        Ok(response)
    }

    fn get_prop_value_scalar(
        &self,
        ptp: &mut PtpSession,
        prop: DevicePropCode,
    ) -> Result<u32, anyhow::Error> {
        let data = self.get_prop_value_raw(ptp, prop)?;

        match data.len() {
            1 => Ok(u32::from(data[0])),
            2 => Ok(u32::from(u16::from_le_bytes([data[0], data[1]]))),
            4 => Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]])),
            n => bail!("Cannot parse property {prop:?} as scalar: {n} bytes"),
        }
    }

    fn get_usb_mode(&self, ptp: &mut PtpSession) -> Result<UsbMode, anyhow::Error> {
        let result = self.get_prop_value_scalar(ptp, DevicePropCode::FujiUsbMode)?;
        Ok(result.into())
    }

    fn get_battery_info(&self, ptp: &mut PtpSession) -> Result<u32, anyhow::Error> {
        let data = self.get_prop_value_raw(ptp, DevicePropCode::FujiBatteryInfo2)?;
        debug!("Raw battery data: {data:?}");

        if data.len() < 3 {
            bail!("Battery info payload too short");
        }

        let utf16: Vec<u16> = data[1..]
            .chunks(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .take_while(|&c| c != 0)
            .collect();

        debug!("Decoded UTF-16 units: {utf16:?}");

        let utf8_string = String::from_utf16(&utf16)?;
        debug!("Decoded UTF-16 string: {utf8_string}");

        let percentage: u32 = utf8_string
            .split(',')
            .next()
            .ok_or_else(|| anyhow::anyhow!("Failed to parse battery percentage"))?
            .parse()?;

        Ok(percentage)
    }

    fn export_backup(&self, ptp: &mut PtpSession) -> Result<Vec<u8>, anyhow::Error> {
        const HANDLE: u32 = 0x0;

        debug!("Getting object info for backup");

        let info = ptp.command(
            StandardCommandCode::GetObjectInfo,
            &[HANDLE],
            None,
            Some(TIMEOUT),
        )?;

        debug!("Got object info, {} bytes", info.len());

        debug!("Downloading backup object");

        let object = ptp.command(
            StandardCommandCode::GetObject,
            &[HANDLE],
            None,
            Some(TIMEOUT),
        )?;

        debug!("Downloaded backup object ({} bytes)", object.len());

        Ok(object)
    }

    fn import_backup(&self, ptp: &mut PtpSession, buffer: &[u8]) -> Result<(), anyhow::Error> {
        todo!("This is currently broken");

        debug!("Preparing ObjectInfo header for backup");

        let mut obj_info = vec![0u8; 1088];
        let mut offset = 0;

        let padding0: u32 = 0x0;
        let object_format: u16 = 0x5000;
        let padding1: u16 = 0x0;

        obj_info[offset..offset + size_of::<u32>()].copy_from_slice(&padding0.to_le_bytes());
        offset += size_of::<u32>();
        obj_info[offset..offset + size_of::<u16>()].copy_from_slice(&object_format.to_le_bytes());
        offset += size_of::<u16>();
        obj_info[offset..offset + size_of::<u16>()].copy_from_slice(&padding1.to_le_bytes());
        offset += size_of::<u16>();
        obj_info[offset..offset + size_of::<u32>()]
            .copy_from_slice(&u32::try_from(buffer.len())?.to_le_bytes());

        let param0: u32 = 0x0;
        let param1: u32 = 0x0;

        debug!("Sending ObjectInfo for backup");

        ptp.command(
            libptp::StandardCommandCode::SendObjectInfo,
            &[param0, param1],
            Some(&obj_info),
            Some(TIMEOUT),
        )?;

        debug!("Sending backup payload ({} bytes)", buffer.len());

        ptp.command(
            libptp::StandardCommandCode::SendObject,
            &[],
            Some(buffer),
            Some(TIMEOUT),
        )?;

        Ok(())
    }
}

macro_rules! default_camera_impl {
    (
        $const_name:ident,
        $struct_name:ident,
        $vendor:expr,
        $product:expr,
        $display_name:expr
    ) => {
        pub const $const_name: CameraId = CameraId {
            name: $display_name,
            vendor: $vendor,
            product: $product,
        };

        pub struct $struct_name {
            device: rusb::Device<rusb::GlobalContext>,
            session_counter: std::sync::atomic::AtomicU32,
        }

        impl $struct_name {
            pub fn new_boxed(
                rusb_device: &rusb::Device<rusb::GlobalContext>,
            ) -> Result<Box<dyn CameraImpl>, anyhow::Error> {
                let session_counter = std::sync::atomic::AtomicU32::new(1);

                let handle = rusb_device.open()?;
                let device = handle.device();

                Ok(Box::new(Self {
                    device,
                    session_counter,
                }))
            }
        }

        impl CameraImpl for $struct_name {
            fn id(&self) -> &'static CameraId {
                &$const_name
            }

            fn device(&self) -> &rusb::Device<rusb::GlobalContext> {
                &self.device
            }

            fn ptp(&self) -> Ptp {
                libptp::Camera::new(&self.device).unwrap().into()
            }

            fn ptp_session(&self) -> Result<PtpSession, anyhow::Error> {
                let ptp = self.ptp();
                self.open_session(ptp)
            }

            fn next_session_id(&self) -> u32 {
                self.session_counter
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            }
        }
    };
}

default_camera_impl!(FUJIFILM_XT5, FujifilmXT5, 0x04cb, 0x02fc, "FUJIFILM XT-5");
