use anyhow::Context;
use fujicore::{Camera, UsbId, generated::cameras::SUPPORTED};
use rusb::GlobalContext;

#[derive(Debug, Clone)]
pub struct DeviceCandidate {
    pub name: &'static str,
    pub usb_id: UsbId,
    pub bus: u8,
    pub address: u8,
    pub device: rusb::Device<GlobalContext>,
}

pub fn enumerate() -> anyhow::Result<Vec<DeviceCandidate>> {
    let mut candidates = Vec::new();
    for device in rusb::devices().context("enumerating USB devices")?.iter() {
        if !Camera::probe(&device).context("probing USB device for Fujifilm support")? {
            continue;
        }

        let descriptor = device
            .device_descriptor()
            .context("reading USB device descriptor")?;
        let usb_id = UsbId {
            vendor: descriptor.vendor_id(),
            product: descriptor.product_id(),
        };
        let name = SUPPORTED
            .iter()
            .find(|c| c.usb_id == usb_id)
            .expect("Camera::probe checked SUPPORTED above")
            .name;

        candidates.push(DeviceCandidate {
            name,
            usb_id,
            bus: device.bus_number(),
            address: device.address(),
            device,
        });
    }
    Ok(candidates)
}
