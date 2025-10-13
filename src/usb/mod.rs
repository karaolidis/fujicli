use std::error::Error;

use rusb::GlobalContext;

use crate::hardware::SUPPORTED_MODELS;

#[derive(Clone)]
pub struct Device {
    pub model: &'static dyn crate::hardware::Camera,
    pub rusb_device: rusb::Device<GlobalContext>,
}

impl Device {
    pub fn camera(&self) -> Result<libptp::Camera<GlobalContext>, Box<dyn Error + Send + Sync>> {
        let handle = self.rusb_device.open()?;
        let device = handle.device();
        let camera = libptp::Camera::new(&device)?;
        Ok(camera)
    }

    pub fn id(&self) -> String {
        let bus = self.rusb_device.bus_number();
        let address = self.rusb_device.address();
        format!("{}.{}", bus, address)
    }

    pub fn name(&self) -> String {
        self.model.name().to_string()
    }

    pub fn vendor_id(&self) -> u16 {
        let descriptor = self.rusb_device.device_descriptor().unwrap();
        descriptor.vendor_id()
    }

    pub fn product_id(&self) -> u16 {
        let descriptor = self.rusb_device.device_descriptor().unwrap();
        descriptor.product_id()
    }
}

pub fn get_connected_devices() -> Result<Vec<Device>, Box<dyn Error + Send + Sync>> {
    let mut connected_devices = Vec::new();

    for device in rusb::devices()?.iter() {
        let descriptor = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };

        for model in SUPPORTED_MODELS.iter() {
            if descriptor.vendor_id() == model.vendor_id()
                && descriptor.product_id() == model.product_id()
            {
                let connected_device = Device {
                    model: *model,
                    rusb_device: device,
                };

                connected_devices.push(connected_device);
                break;
            }
        }
    }

    Ok(connected_devices)
}

pub fn get_connected_device_by_id(id: &str) -> Result<Device, Box<dyn Error + Send + Sync>> {
    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid device id format: {}", id).into());
    }

    let bus: u8 = parts[0].parse()?;
    let address: u8 = parts[1].parse()?;

    for device in rusb::devices()?.iter() {
        if device.bus_number() == bus && device.address() == address {
            let descriptor = device.device_descriptor()?;

            for model in SUPPORTED_MODELS.iter() {
                if descriptor.vendor_id() == model.vendor_id()
                    && descriptor.product_id() == model.product_id()
                {
                    return Ok(Device {
                        model: *model,
                        rusb_device: device,
                    });
                }
            }

            return Err(format!("Device found at {} but is not supported", id).into());
        }
    }

    Err(format!("No device found with id: {}", id).into())
}

pub fn get_device(device_id: Option<&str>) -> Result<Device, Box<dyn Error + Send + Sync>> {
    match device_id {
        Some(id) => get_connected_device_by_id(id),
        None => get_connected_devices()?
            .into_iter()
            .next()
            .ok_or_else(|| "No supported devices connected.".into()),
    }
}
