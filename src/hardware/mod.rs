use std::error::Error;

use libptp::{DeviceInfo, StandardCommandCode};
use log::debug;
use rusb::GlobalContext;

mod common;
mod xt5;

pub trait Camera {
    fn vendor_id(&self) -> u16;
    fn product_id(&self) -> u16;
    fn name(&self) -> &'static str;

    fn get_device_info(
        &self,
        camera: &mut libptp::Camera<GlobalContext>,
    ) -> Result<DeviceInfo, Box<dyn Error + Send + Sync>> {
        debug!("Using default GetDeviceInfo command for {}", self.name());

        let response = camera.command(
            StandardCommandCode::GetDeviceInfo,
            &[],
            None,
            Some(common::TIMEOUT),
        )?;

        debug!("Received response with {} bytes", response.len());

        let device_info = DeviceInfo::decode(&response)?;

        Ok(device_info)
    }
}

pub const SUPPORTED_MODELS: &[&dyn Camera] = &[&xt5::FujifilmXT5];
