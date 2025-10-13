use std::error::Error;

use crate::hardware::{CameraImpl, SUPPORTED_CAMERAS};

pub fn get_connected_camers()
-> Result<Vec<Box<dyn crate::hardware::CameraImpl>>, Box<dyn Error + Send + Sync>> {
    let mut connected_cameras = Vec::new();

    for device in rusb::devices()?.iter() {
        let descriptor = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };

        for camera in SUPPORTED_CAMERAS.iter() {
            if camera.matches_descriptor(&descriptor) {
                let camera = (camera.factory)(device)?;
                connected_cameras.push(camera);
                break;
            }
        }
    }

    Ok(connected_cameras)
}

pub fn get_connected_camera_by_id(
    id: &str,
) -> Result<Box<dyn CameraImpl>, Box<dyn Error + Send + Sync>> {
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
                    let camera = (camera.factory)(device)?;
                    return Ok(camera);
                }
            }

            return Err(format!("Device found at {} but is not supported", id).into());
        }
    }

    Err(format!("No device found with id: {}", id).into())
}

pub fn get_camera(
    device_id: Option<&str>,
) -> Result<Box<dyn CameraImpl>, Box<dyn Error + Send + Sync>> {
    match device_id {
        Some(id) => get_connected_camera_by_id(id),
        None => get_connected_camers()?
            .into_iter()
            .next()
            .ok_or_else(|| "No supported devices connected.".into()),
    }
}
