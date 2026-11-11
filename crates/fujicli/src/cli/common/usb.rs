use anyhow::{Context, anyhow};
use fujicore::{
    Camera, UsbId,
    features::base::info::CameraInfoListItem,
    ptp::transport::{self, Device},
};
use log::trace;

pub fn get_device_by_serial(serial: &str) -> anyhow::Result<Device> {
    transport::devices()
        .context("enumerating USB devices")?
        .into_iter()
        .find(|d| d.serial() == Some(serial))
        .ok_or_else(|| anyhow!("no camera found with serial number {serial}"))
}

fn supported_devices() -> anyhow::Result<impl Iterator<Item = Device>> {
    Ok(transport::devices()
        .context("enumerating USB devices")?
        .into_iter()
        .filter(|d| {
            let supported = Camera::probe(d);
            if !supported {
                trace!("USB device {} is not a supported camera", d.usb_id());
            }
            supported
        }))
}

pub fn list_cameras() -> anyhow::Result<Vec<CameraInfoListItem>> {
    Ok(transport::devices()
        .context("enumerating USB devices")?
        .iter()
        .filter_map(CameraInfoListItem::from_device)
        .collect())
}

pub fn get_camera(serial: Option<&str>, emulate: Option<UsbId>) -> anyhow::Result<Camera> {
    let device = match serial {
        Some(serial) => get_device_by_serial(serial)?,
        None => supported_devices()?
            .next()
            .ok_or_else(|| anyhow!("no supported camera found"))?,
    };

    emulate.as_ref().map_or_else(
        || Camera::open(&device).context("opening camera"),
        |usb_id| {
            Camera::open_as(&device, *usb_id)
                .with_context(|| format!("opening camera as emulated {usb_id}"))
        },
    )
}
