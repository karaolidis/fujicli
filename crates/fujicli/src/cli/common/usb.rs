use std::{
    fmt::{Display, Formatter},
    num::ParseIntError,
    str::FromStr,
};

use anyhow::Context;
use fujicore::{Camera, UsbId};
use log::trace;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseLocationError {
    #[error("invalid device format '{0}', expected <BUS>.<ADDRESS>")]
    Format(String),

    #[error("invalid bus number '{value}': {source}")]
    Bus {
        value: String,
        #[source]
        source: ParseIntError,
    },

    #[error("invalid address '{value}': {source}")]
    Address {
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
            .ok_or_else(|| ParseLocationError::Format(s.to_owned()))?;

        let bus = bus.parse().map_err(|source| ParseLocationError::Bus {
            value: bus.to_owned(),
            source,
        })?;

        let address = address
            .parse()
            .map_err(|source| ParseLocationError::Address {
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

pub fn get_camera(device: Option<Location>, emulate: Option<UsbId>) -> anyhow::Result<Camera> {
    if let Some(location) = device {
        let device = get_usb_device_by_location(location)?;

        emulate.as_ref().map_or_else(
            || Camera::open(&device).context("opening camera"),
            |usb_id| {
                Camera::open_as(&device, *usb_id)
                    .with_context(|| format!("opening camera as emulated {usb_id}"))
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
                |usb_id| {
                    Camera::open_as(&device, *usb_id)
                        .with_context(|| format!("opening camera as emulated {usb_id}"))
                },
            );
        }

        anyhow::bail!("no supported camera found");
    }
}
