use anyhow::Context;
use fujicore::{Camera, generated::cameras::SUPPORTED};
use rusb::GlobalContext;

#[derive(Debug, Clone)]
pub struct DeviceCandidate {
    pub name: &'static str,
    pub vendor: u16,
    pub product: u16,
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
        let vendor = descriptor.vendor_id();
        let product = descriptor.product_id();
        let name = SUPPORTED
            .iter()
            .find(|c| c.vendor == vendor && c.product == product)
            .expect("Camera::probe checked SUPPORTED above")
            .name;

        candidates.push(DeviceCandidate {
            name,
            vendor,
            product,
            bus: device.bus_number(),
            address: device.address(),
            device,
        });
    }
    Ok(candidates)
}
