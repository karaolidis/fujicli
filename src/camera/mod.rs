pub mod devices;
pub mod error;
pub mod features;
pub mod ptp;

use anyhow::bail;
use devices::x_trans_v;
use features::{
    base::{CameraBase, info::CameraInfo},
    simulation::simulation::Simulation,
};
use log::{debug, error};
use ptp::{Ptp, hex::FujiCustomSetting};
use rusb::{GlobalContext, constants::LIBUSB_CLASS_IMAGE};

use crate::usb::find_endpoint;

const SESSION: u32 = 1;

pub struct Camera {
    ptp: Ptp,
    r#impl: Box<dyn CameraBase<Context = GlobalContext>>,
}

impl TryFrom<&rusb::Device<GlobalContext>> for Camera {
    type Error = anyhow::Error;

    fn try_from(device: &rusb::Device<GlobalContext>) -> anyhow::Result<Self> {
        let descriptor = device.device_descriptor()?;

        let vendor = descriptor.vendor_id();
        let product = descriptor.product_id();

        for supported_camera in SUPPORTED {
            if vendor != supported_camera.vendor || product != supported_camera.product {
                continue;
            }

            let bus = device.bus_number();
            let address = device.address();

            let config_descriptor = device.active_config_descriptor()?;

            let interface_descriptor = config_descriptor
                .interfaces()
                .flat_map(|i| i.descriptors())
                .find(|x| x.class_code() == LIBUSB_CLASS_IMAGE)
                .ok_or(rusb::Error::NotFound)?;

            let interface = interface_descriptor.interface_number();
            debug!("Found interface {interface}");

            let handle = device.open()?;
            handle.claim_interface(interface)?;

            let bulk_in = find_endpoint(
                &interface_descriptor,
                rusb::Direction::In,
                rusb::TransferType::Bulk,
            )?;

            let bulk_out = find_endpoint(
                &interface_descriptor,
                rusb::Direction::Out,
                rusb::TransferType::Bulk,
            )?;

            let transaction_id = 0;

            let r#impl = (supported_camera.camera_factory)();
            let chunk_size = r#impl.chunk_size();

            let mut ptp = Ptp {
                bus,
                address,
                interface,
                bulk_in,
                bulk_out,
                handle,
                transaction_id,
                chunk_size,
            };

            debug!("Opening session");
            let () = ptp.open_session(SESSION)?;
            debug!("Session opened");

            return Ok(Self { ptp, r#impl });
        }

        bail!("Device not supported");
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        debug!("Closing session");
        if let Err(e) = self.ptp.close_session(SESSION) {
            error!("Error closing session: {e}");
        }
        debug!("Session closed");
    }
}

type CameraFactory = fn() -> Box<dyn CameraBase<Context = GlobalContext>>;

#[derive(Debug, Clone, Copy)]
pub struct SupportedCamera {
    pub name: &'static str,
    pub vendor: u16,
    pub product: u16,
    pub camera_factory: CameraFactory,
}

pub const SUPPORTED: &[SupportedCamera] = &[x_trans_v::x_t5::FUJIFILM_XT5];

impl Camera {
    pub fn name(&self) -> &'static str {
        self.r#impl.camera_definition().name
    }

    pub fn vendor_id(&self) -> u16 {
        self.r#impl.camera_definition().vendor
    }

    pub fn product_id(&self) -> u16 {
        self.r#impl.camera_definition().product
    }

    pub fn connected_usb_id(&self) -> String {
        format!("{}.{}", self.ptp.bus, self.ptp.address)
    }

    pub fn get_info(&mut self) -> anyhow::Result<Box<dyn CameraInfo>> {
        self.r#impl.get_info(&mut self.ptp)
    }

    pub fn custom_settings_slots(&self) -> anyhow::Result<Vec<FujiCustomSetting>> {
        if let Some(sim) = self.r#impl.as_simulations() {
            Ok(sim.custom_settings_slots())
        } else {
            bail!("Camera does not support simulations");
        }
    }

    pub fn get_simulation(
        &mut self,
        slot: FujiCustomSetting,
    ) -> anyhow::Result<Box<dyn Simulation>> {
        if let Some(sim) = self.r#impl.as_simulations() {
            sim.get_simulation(&mut self.ptp, slot)
        } else {
            bail!("Camera does not support simulations");
        }
    }

    pub fn update_simulation(
        &mut self,
        slot: FujiCustomSetting,
        modifier: &mut dyn FnMut(&mut dyn Simulation) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if let Some(sim) = self.r#impl.as_simulations() {
            sim.update_simulation(&mut self.ptp, slot, modifier)
        } else {
            bail!("Camera does not support simulations");
        }
    }

    pub fn export_simulation(&mut self, slot: FujiCustomSetting) -> anyhow::Result<Vec<u8>> {
        if let Some(simulations) = self.r#impl.as_simulations() {
            simulations.export_simulation(&mut self.ptp, slot)
        } else {
            bail!("Camera does not support simulations");
        }
    }

    pub fn import_simulation(
        &mut self,
        slot: FujiCustomSetting,
        buffer: &[u8],
    ) -> anyhow::Result<()> {
        if let Some(simulations) = self.r#impl.as_simulations() {
            simulations.import_simulation(&mut self.ptp, slot, buffer)
        } else {
            bail!("Camera does not support simulations");
        }
    }

    pub fn export_backup(&mut self) -> anyhow::Result<Vec<u8>> {
        if let Some(backups) = self.r#impl.as_backups() {
            backups.export_backup(&mut self.ptp)
        } else {
            bail!("Camera does not support backups");
        }
    }

    pub fn import_backup(&mut self, buffer: &[u8]) -> anyhow::Result<()> {
        if let Some(backups) = self.r#impl.as_backups() {
            backups.import_backup(&mut self.ptp, buffer)
        } else {
            bail!("Camera does not support backups");
        }
    }
}
