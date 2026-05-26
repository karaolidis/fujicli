use std::{
    fmt::{Display, Formatter},
    num::ParseIntError,
    str::FromStr,
};

use anyhow::Context;
use fujicore::Camera;
use log::trace;
use thiserror::Error;

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Error)]
pub enum ParseLocationError {
    #[error("invalid device format '{0}', expected <BUS>.<ADDRESS>")]
    BadFormat(String),

    #[error("invalid bus number '{value}': {source}")]
    BadBus {
        value: String,
        #[source]
        source: ParseIntError,
    },

    #[error("invalid address '{value}': {source}")]
    BadAddress {
        value: String,
        #[source]
        source: ParseIntError,
    },
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Error)]
pub enum ParseIdentityError {
    #[error("invalid model format '{0}', expected <VENDOR_ID>:<PRODUCT_ID>")]
    BadFormat(String),

    #[error("invalid vendor id '{value}': {source}")]
    BadVendor {
        value: String,
        #[source]
        source: ParseIntError,
    },

    #[error("invalid product id '{value}': {source}")]
    BadProduct {
        value: String,
        #[source]
        source: ParseIntError,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Location {
    pub bus: u8,
    pub address: u8,
}

impl FromStr for Location {
    type Err = ParseLocationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (bus, address) = s
            .split_once('.')
            .ok_or_else(|| ParseLocationError::BadFormat(s.to_owned()))?;

        let bus = bus.parse().map_err(|source| ParseLocationError::BadBus {
            value: bus.to_owned(),
            source,
        })?;

        let address = address
            .parse()
            .map_err(|source| ParseLocationError::BadAddress {
                value: address.to_owned(),
                source,
            })?;

        Ok(Self { bus, address })
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
    type Err = ParseIdentityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (vendor, product) = s
            .split_once(':')
            .ok_or_else(|| ParseIdentityError::BadFormat(s.to_owned()))?;

        let vendor =
            u16::from_str_radix(vendor, 16).map_err(|source| ParseIdentityError::BadVendor {
                value: vendor.to_owned(),
                source,
            })?;

        let product =
            u16::from_str_radix(product, 16).map_err(|source| ParseIdentityError::BadProduct {
                value: product.to_owned(),
                source,
            })?;

        Ok(Self { vendor, product })
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
    for device in rusb::devices().context("enumerating USB devices")?.iter() {
        let bus = device.bus_number();
        let address = device.address();

        if bus != location.bus || address != location.address {
            trace!("USB device {device:x?} does not match specified location");
            continue;
        }

        return Ok(device);
    }

    anyhow::bail!("no USB device found at location {location}");
}

pub fn get_all_cameras() -> anyhow::Result<Vec<Camera>> {
    let mut cameras = Vec::new();

    for device in rusb::devices().context("enumerating USB devices")?.iter() {
        trace!("Found USB device {device:x?}");
        if !Camera::probe(&device).context("probing USB device for Fujifilm support")? {
            trace!("USB device {device:x?} is not a supported camera");
            continue;
        }

        let camera = Camera::open(&device).context("opening supported camera")?;
        cameras.push(camera);
    }

    Ok(cameras)
}

pub fn get_camera(device: Option<Location>, emulate: Option<Identity>) -> anyhow::Result<Camera> {
    if let Some(location) = device {
        let device = get_usb_device_by_location(location)?;

        emulate.as_ref().map_or_else(
            || Camera::open(&device).context("opening camera"),
            |identity| {
                Camera::open_as(&device, identity.vendor, identity.product)
                    .with_context(|| format!("opening camera as emulated {identity}"))
            },
        )
    } else {
        for device in rusb::devices().context("enumerating USB devices")?.iter() {
            trace!("Found USB device {device:x?}");
            if !Camera::probe(&device).context("probing USB device for Fujifilm support")? {
                trace!("USB device {device:x?} is not a supported camera");
                continue;
            }

            return emulate.as_ref().map_or_else(
                || Camera::open(&device).context("opening camera"),
                |identity| {
                    Camera::open_as(&device, identity.vendor, identity.product)
                        .with_context(|| format!("opening camera as emulated {identity}"))
                },
            );
        }

        anyhow::bail!("no supported camera found");
    }
}
