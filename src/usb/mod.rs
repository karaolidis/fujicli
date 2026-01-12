use anyhow::{anyhow, bail};
use log::trace;

use crate::camera::Camera;

pub fn find_endpoint(
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

pub fn get_connected_cameras() -> anyhow::Result<Vec<Camera>> {
    let mut connected_cameras = Vec::new();

    for device in rusb::devices()?.iter() {
        trace!("Found USB device {device:x?}");
        if let Ok(camera) = Camera::from_device(&device, None, None) {
            connected_cameras.push(camera);
        }
    }

    Ok(connected_cameras)
}

pub fn get_connected_camera_by_id(device: &str, emulate: Option<&str>) -> anyhow::Result<Camera> {
    let (bus, address): (u8, u8) = {
        let parts: Vec<&str> = device.split('.').collect();
        if parts.len() != 2 {
            bail!("Invalid device id format: {device}");
        }
        (parts[0].parse()?, parts[1].parse()?)
    };

    let (emulated_vendor, emulated_product): (Option<u16>, Option<u16>) = match emulate {
        Some(emulated) => {
            let parts: Vec<&str> = emulated.split(':').collect();
            if parts.len() != 2 {
                bail!("Invalid model format: {emulated}");
            }
            (
                Some(u16::from_str_radix(parts[0], 16)?),
                Some(u16::from_str_radix(parts[1], 16)?),
            )
        }
        None => (None, None),
    };

    for device in rusb::devices()?.iter() {
        if device.bus_number() == bus && device.address() == address {
            return Camera::from_device(&device, emulated_vendor, emulated_product);
        }
    }

    bail!("No device found with id: {device}");
}

pub fn get_camera(device: Option<&str>, emulate: Option<&str>) -> anyhow::Result<Camera> {
    match device {
        Some(device) => get_connected_camera_by_id(device, emulate),
        None => get_connected_cameras()?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No supported devices connected")),
    }
}
