use anyhow::{anyhow, bail};

use crate::hardware::{CameraImpl, SUPPORTED_CAMERAS};

pub fn get_connected_camers() -> Result<Vec<Box<dyn crate::hardware::CameraImpl>>, anyhow::Error> {
    let mut connected_cameras = Vec::new();

    for device in rusb::devices()?.iter() {
        let Ok(descriptor) = device.device_descriptor() else {
            continue;
        };

        for camera in SUPPORTED_CAMERAS {
            if camera.matches_descriptor(&descriptor) {
                let camera = (camera.factory)(device)?;
                connected_cameras.push(camera);
                break;
            }
        }
    }

    Ok(connected_cameras)
}

pub fn get_connected_camera_by_id(id: &str) -> Result<Box<dyn CameraImpl>, anyhow::Error> {
    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() != 2 {
        bail!("Invalid device id format: {id}");
    }

    let bus: u8 = parts[0].parse()?;
    let address: u8 = parts[1].parse()?;

    for device in rusb::devices()?.iter() {
        if device.bus_number() == bus && device.address() == address {
            let descriptor = device.device_descriptor()?;

            for camera in SUPPORTED_CAMERAS {
                if camera.matches_descriptor(&descriptor) {
                    let camera = (camera.factory)(device)?;
                    return Ok(camera);
                }
            }

            bail!("Device found at {id} but is not supported");
        }
    }

    bail!("No device found with id: {id}");
}

pub fn get_camera(device_id: Option<&str>) -> Result<Box<dyn CameraImpl>, anyhow::Error> {
    match device_id {
        Some(id) => get_connected_camera_by_id(id),
        None => get_connected_camers()?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No supported devices connected.")),
    }
}
