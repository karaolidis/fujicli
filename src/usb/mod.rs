use anyhow::{anyhow, bail};

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
        if let Ok(camera) = Camera::try_from(&device) {
            connected_cameras.push(camera);
        }
    }

    Ok(connected_cameras)
}

pub fn get_connected_camera_by_id(id: &str) -> anyhow::Result<Camera> {
    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() != 2 {
        bail!("Invalid device id format: {id}");
    }

    let bus: u8 = parts[0].parse()?;
    let address: u8 = parts[1].parse()?;

    for device in rusb::devices()?.iter() {
        if device.bus_number() == bus && device.address() == address {
            return Camera::try_from(&device);
        }
    }

    bail!("No device found with id: {id}");
}

pub fn get_camera(device_id: Option<&str>) -> anyhow::Result<Camera> {
    match device_id {
        Some(id) => get_connected_camera_by_id(id),
        None => get_connected_cameras()?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No supported devices connected")),
    }
}
