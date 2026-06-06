use std::{fmt, num::ParseIntError, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{SupportedCamera, generated::cameras::SUPPORTED};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UsbId {
    pub vendor: u16,
    pub product: u16,
}

impl UsbId {
    #[must_use]
    pub fn supported_camera(self) -> Option<&'static SupportedCamera> {
        SUPPORTED.iter().find(|c| c.usb_id == self)
    }
}

impl fmt::Display for UsbId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor, self.product)
    }
}

#[derive(Debug, Error)]
pub enum ParseUsbIdError {
    #[error("invalid USB ID format '{0}', expected <VENDOR_ID>:<PRODUCT_ID>")]
    Format(String),

    #[error("invalid vendor id '{value}': {source}")]
    Vendor {
        value: String,
        #[source]
        source: ParseIntError,
    },

    #[error("invalid product id '{value}': {source}")]
    Product {
        value: String,
        #[source]
        source: ParseIntError,
    },
}

impl FromStr for UsbId {
    type Err = ParseUsbIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (vendor, product) = s
            .split_once(':')
            .ok_or_else(|| ParseUsbIdError::Format(s.to_owned()))?;

        let vendor = u16::from_str_radix(vendor, 16).map_err(|source| ParseUsbIdError::Vendor {
            value: vendor.to_owned(),
            source,
        })?;

        let product =
            u16::from_str_radix(product, 16).map_err(|source| ParseUsbIdError::Product {
                value: product.to_owned(),
                source,
            })?;

        Ok(Self { vendor, product })
    }
}
