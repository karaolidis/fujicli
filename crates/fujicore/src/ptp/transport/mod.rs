#[cfg(not(windows))]
mod libusb;
#[cfg(windows)]
mod wpd;

#[cfg(not(windows))]
pub use libusb::{Device, Transport, devices};
#[cfg(windows)]
pub use wpd::{Device, Transport, devices};

#[cfg(not(windows))]
pub type TransportError = rusb::Error;
#[cfg(windows)]
pub type TransportError = windows::core::Error;
