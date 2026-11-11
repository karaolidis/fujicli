use std::{
    fmt::{Display, Formatter},
    num::ParseIntError,
    str::FromStr,
};

use anyhow::Context;
use fujicore::{
    Camera, UsbId,
    features::base::info::CameraInfoListItem,
    ptp::transport::{self, Device},
};
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

pub fn get_device_by_location(location: Location) -> anyhow::Result<Device> {
    transport::devices()
        .context("enumerating USB devices")?
        .into_iter()
        .find(|d| d.bus() == location.bus && d.address() == location.address)
        .ok_or_else(|| anyhow::anyhow!("no USB device found at location {location}"))
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

pub fn get_camera(device: Option<Location>, emulate: Option<UsbId>) -> anyhow::Result<Camera> {
    let device = match device {
        Some(location) => get_device_by_location(location)?,
        None => supported_devices()?
            .next()
            .ok_or_else(|| anyhow::anyhow!("no supported camera found"))?,
    };

    emulate.as_ref().map_or_else(
        || Camera::open(&device).context("opening camera"),
        |usb_id| {
            Camera::open_as(&device, *usb_id)
                .with_context(|| format!("opening camera as emulated {usb_id}"))
        },
    )
}
