use std::error::Error;

use libptp::DeviceInfo;
use rusb::GlobalContext;

use crate::hardware::{FujiUsbMode, SUPPORTED_CAMERAS};

pub struct Camera {
    camera_impl: Box<dyn crate::hardware::CameraImpl>,
    rusb_device: rusb::Device<GlobalContext>,
}

impl Camera {
    pub fn id(&self) -> String {
        let bus = self.rusb_device.bus_number();
        let address = self.rusb_device.address();
        format!("{}.{}", bus, address)
    }

    pub fn name(&self) -> String {
        self.camera_impl.name().to_string()
    }

    pub fn vendor_id(&self) -> u16 {
        let descriptor = self.rusb_device.device_descriptor().unwrap();
        descriptor.vendor_id()
    }

    pub fn product_id(&self) -> u16 {
        let descriptor = self.rusb_device.device_descriptor().unwrap();
        descriptor.product_id()
    }

    pub fn ptp(&self) -> Result<libptp::Camera<GlobalContext>, Box<dyn Error + Send + Sync>> {
        let handle = self.rusb_device.open()?;
        let device = handle.device();
        let ptp = libptp::Camera::new(&device)?;
        Ok(ptp)
    }

    pub fn get_info(&self) -> Result<DeviceInfo, Box<dyn Error + Send + Sync>> {
        let mut ptp = self.ptp()?;
        self.camera_impl.get_info(&mut ptp)
    }

    pub fn get_fuji_usb_mode(&self) -> Result<FujiUsbMode, Box<dyn Error + Send + Sync>> {
        let mut ptp = self.ptp()?;
        self.camera_impl.get_fuji_usb_mode(&mut ptp)
    }

    pub fn get_fuji_battery_info(&self) -> Result<u32, Box<dyn Error + Send + Sync>> {
        let mut ptp = self.ptp()?;
        self.camera_impl.get_fuji_battery_info(&mut ptp)
    }
}

pub fn get_connected_camers() -> Result<Vec<Camera>, Box<dyn Error + Send + Sync>> {
    let mut connected_cameras = Vec::new();

    for device in rusb::devices()?.iter() {
        let descriptor = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };

        for camera in SUPPORTED_CAMERAS.iter() {
            if camera.matches_descriptor(&descriptor) {
                let camera = Camera {
                    camera_impl: camera.into(),
                    rusb_device: device,
                };

                connected_cameras.push(camera);
                break;
            }
        }
    }

    Ok(connected_cameras)
}

pub fn get_connected_camera_by_id(id: &str) -> Result<Camera, Box<dyn Error + Send + Sync>> {
    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid device id format: {}", id).into());
    }

    let bus: u8 = parts[0].parse()?;
    let address: u8 = parts[1].parse()?;

    for device in rusb::devices()?.iter() {
        if device.bus_number() == bus && device.address() == address {
            let descriptor = device.device_descriptor()?;

            for camera in SUPPORTED_CAMERAS.iter() {
                if camera.matches_descriptor(&descriptor) {
                    return Ok(Camera {
                        camera_impl: camera.into(),
                        rusb_device: device,
                    });
                }
            }

            return Err(format!("Device found at {} but is not supported", id).into());
        }
    }

    Err(format!("No device found with id: {}", id).into())
}

pub fn get_camera(device_id: Option<&str>) -> Result<Camera, Box<dyn Error + Send + Sync>> {
    match device_id {
        Some(id) => get_connected_camera_by_id(id),
        None => get_connected_camers()?
            .into_iter()
            .next()
            .ok_or_else(|| "No supported devices connected.".into()),
    }
}
