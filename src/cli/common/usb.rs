use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use anyhow::{anyhow, bail};
use fujicli::{Camera, find_device, list_devices, ptp::TransportKind};

#[derive(Debug, Clone, Copy)]
pub struct Identity {
    pub vendor: u16,
    pub product: u16,
}

impl FromStr for Identity {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (vendor, product) = s.split_once(':').ok_or_else(|| {
            anyhow!("Invalid model format: {s}, expected <VENDOR_ID>:<PRODUCT_ID>")
        })?;

        Ok(Self {
            vendor: u16::from_str_radix(vendor, 16)
                .map_err(|_| anyhow!("Invalid vendor ID: {vendor}"))?,
            product: u16::from_str_radix(product, 16)
                .map_err(|_| anyhow!("Invalid product ID: {product}"))?,
        })
    }
}

impl Display for Identity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor, self.product)
    }
}

pub fn get_all_cameras(transport: TransportKind) -> anyhow::Result<Vec<Camera>> {
    let mut cameras = Vec::new();

    for device in list_devices(transport)? {
        cameras.push(Camera::open(&device)?);
    }

    Ok(cameras)
}

pub fn get_camera(
    transport: TransportKind,
    device: Option<&str>,
    emulate: Option<Identity>,
) -> anyhow::Result<Camera> {
    let device = match device {
        Some(id) => find_device(transport, id)?,
        None => list_devices(transport)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No supported camera found"))?,
    };

    emulate.as_ref().map_or_else(
        || Camera::open(&device),
        |identity| Camera::open_as(&device, identity.vendor, identity.product),
    )
}

/// Resolve `-d` without requiring the device to be a known model.
pub fn get_unknown_camera(
    transport: TransportKind,
    device: Option<&str>,
    purpose: &str,
) -> anyhow::Result<Camera> {
    let Some(id) = device else {
        bail!("Device must be specified for {purpose}");
    };

    let device = find_device(transport, id)?;
    Camera::open_unknown(&device)
}
