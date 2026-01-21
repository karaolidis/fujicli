use std::{fmt::Display, fmt::Formatter, str::FromStr};

use anyhow::{anyhow, bail};
use fujicli::Camera;
use log::trace;

#[derive(Debug, Clone, Copy)]
pub struct Location {
    pub bus: u8,
    pub address: u8,
}

impl FromStr for Location {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (bus, address) = s
            .split_once('.')
            .ok_or_else(|| anyhow!("Invalid device format: {s}, expected <BUS>.<ADDRESS>"))?;

        Ok(Self {
            bus: bus
                .parse()
                .map_err(|_| anyhow!("Invalid bus number: {bus}"))?,
            address: address
                .parse()
                .map_err(|_| anyhow!("Invalid address: {address}"))?,
        })
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.bus, self.address)
    }
}

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

pub fn get_usb_device_by_location(
    location: Location,
) -> anyhow::Result<rusb::Device<rusb::GlobalContext>> {
    for device in rusb::devices()?.iter() {
        let bus = device.bus_number();
        let address = device.address();

        if bus != location.bus || address != location.address {
            trace!("USB device {device:x?} does not match specified location");
            continue;
        }

        return Ok(device);
    }

    bail!("No USB device found at location {location}");
}

pub fn get_all_cameras() -> anyhow::Result<Vec<Camera>> {
    let mut cameras = Vec::new();

    for device in rusb::devices()?.iter() {
        trace!("Found USB device {device:x?}");
        if !Camera::probe(&device)? {
            trace!("USB device {device:x?} is not a supported camera");
            continue;
        }

        let camera = Camera::open(&device)?;
        cameras.push(camera);
    }

    Ok(cameras)
}

pub fn get_camera(device: Option<Location>, emulate: Option<Identity>) -> anyhow::Result<Camera> {
    if let Some(location) = device {
        let device = get_usb_device_by_location(location)?;

        emulate.as_ref().map_or_else(
            || Camera::open(&device),
            |identity| Camera::open_as(&device, identity.vendor, identity.product),
        )
    } else {
        for device in rusb::devices()?.iter() {
            trace!("Found USB device {device:x?}");
            if !Camera::probe(&device)? {
                trace!("USB device {device:x?} is not a supported camera");
                continue;
            }

            return emulate.as_ref().map_or_else(
                || Camera::open(&device),
                |identity| Camera::open_as(&device, identity.vendor, identity.product),
            );
        }

        bail!("No supported camera found");
    }
}
