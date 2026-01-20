pub mod devices;
pub mod features;
pub mod input;
pub mod ptp;
pub mod usb;

use anyhow::bail;
use devices::x_trans_v;
use features::{
    base::{CameraBase, info::CameraInfo},
    simulation::Simulation,
};
use log::{debug, error};
use ptp::{Ptp, fuji};
use rusb::{GlobalContext, constants::LIBUSB_CLASS_IMAGE};

use crate::{
    devices::{x_trans, x_trans_ii, x_trans_iii, x_trans_iv},
    features::render::ConversionProfile,
    usb::find_endpoint,
};

const ERROR_DEVICE_NOT_SUPPORTED: &str = "Device not supported";
const ERROR_CAMERA_DOES_NOT_SUPPORT_BACKUP_MANAGEMENT: &str =
    "This camera does not support backups yet";
const ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_PARSING: &str =
    "This camera does not support simulation parsing yet";
const ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_MANAGEMENT: &str =
    "This camera does not support simulation management yet";
const ERROR_CAMERA_DOES_NOT_SUPPORT_RENDER_MANAGEMENT: &str =
    "This camera does not support rendering images yet";

const SESSION: u32 = 1;

pub struct Camera {
    pub ptp: Ptp,
    r#impl: Box<dyn CameraBase<Context = GlobalContext>>,
}

impl Camera {
    pub fn from_device(
        device: &rusb::Device<GlobalContext>,
        emulated_vendor: Option<u16>,
        emulated_product: Option<u16>,
    ) -> anyhow::Result<Self> {
        let descriptor = device.device_descriptor()?;

        let vendor = emulated_vendor.unwrap_or_else(|| descriptor.vendor_id());
        let product = emulated_product.unwrap_or_else(|| descriptor.product_id());

        let Some(camera) = SUPPORTED
            .iter()
            .find(|c| c.vendor == vendor && c.product == product)
        else {
            bail!(ERROR_DEVICE_NOT_SUPPORTED)
        };
        debug!("Found supported camera {camera:x?}");

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
        debug!("Claimed interface");

        let bulk_in = find_endpoint(
            &interface_descriptor,
            rusb::Direction::In,
            rusb::TransferType::Bulk,
        )?;
        debug!("Found Bulk In endpoint");

        let bulk_out = find_endpoint(
            &interface_descriptor,
            rusb::Direction::Out,
            rusb::TransferType::Bulk,
        )?;
        debug!("Found Bulk Out endpoint");

        let transaction_id = 0;

        let r#impl = (camera.camera_factory)();
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

        ptp.open_session(SESSION)?;

        Ok(Self { ptp, r#impl })
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        if let Err(error) = self.ptp.close_session(SESSION) {
            error!("Error closing session: {error}");
        }
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

pub const SUPPORTED: &[SupportedCamera] = &[
    x_trans::x_e1::FUJIFILM_X_E1,
    x_trans::x_m1::FUJIFILM_X_M1,
    x_trans_ii::x70::FUJIFILM_X70,
    x_trans_ii::x_e2::FUJIFILM_X_E2,
    x_trans_ii::x_t1::FUJIFILM_X_T1,
    x_trans_ii::x_t10::FUJIFILM_X_T10,
    x_trans_iii::x100f::FUJIFILM_X100F,
    x_trans_iii::x_e3::FUJIFILM_X_E3,
    x_trans_iii::x_h1::FUJIFILM_X_H1,
    x_trans_iii::x_pro2::FUJIFILM_X_PRO2,
    x_trans_iii::x_t2::FUJIFILM_X_T2,
    x_trans_iii::x_t20::FUJIFILM_X_T20,
    x_trans_iv::x100v::FUJIFILM_X100V,
    x_trans_iv::x_e4::FUJIFILM_X_E4,
    x_trans_iv::x_pro3::FUJIFILM_X_PRO3,
    x_trans_iv::x_s10::FUJIFILM_X_S10,
    x_trans_iv::x_s20::FUJIFILM_X_S20,
    x_trans_iv::x_t3::FUJIFILM_X_T3,
    x_trans_iv::x_t4::FUJIFILM_X_T4,
    x_trans_v::x100vi::FUJIFILM_X100VI,
    x_trans_v::x_h2::FUJIFILM_X_H2,
    x_trans_v::x_h2s::FUJIFILM_X_H2S,
    x_trans_v::x_t5::FUJIFILM_X_T5,
];

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

    pub fn export_backup(&mut self) -> anyhow::Result<Vec<u8>> {
        if let Some(backups) = self.r#impl.as_backup_manager() {
            backups.export_backup(&mut self.ptp)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_BACKUP_MANAGEMENT);
        }
    }

    pub fn import_backup(&mut self, buffer: &[u8]) -> anyhow::Result<()> {
        if let Some(backups) = self.r#impl.as_backup_manager() {
            backups.import_backup(&mut self.ptp, buffer)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_BACKUP_MANAGEMENT);
        }
    }

    pub fn serialize_simulation(&self, simulation: &dyn Simulation) -> anyhow::Result<Vec<u8>> {
        if let Some(simulations) = self.r#impl.as_simulation_parser() {
            simulations.serialize_simulation(simulation)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_PARSING);
        }
    }

    pub fn deserialize_simulation(&self, simulation: &[u8]) -> anyhow::Result<Box<dyn Simulation>> {
        if let Some(simulations) = self.r#impl.as_simulation_parser() {
            simulations.deserialize_simulation(simulation)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_PARSING);
        }
    }

    pub fn custom_settings_slots(&self) -> anyhow::Result<Vec<fuji::CustomSetting>> {
        if let Some(sim) = self.r#impl.as_simulation_manager() {
            Ok(sim.custom_settings_slots())
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_MANAGEMENT);
        }
    }

    pub fn get_simulation(
        &mut self,
        slot: fuji::CustomSetting,
    ) -> anyhow::Result<Box<dyn Simulation>> {
        if let Some(sim) = self.r#impl.as_simulation_manager() {
            sim.get_simulation(&mut self.ptp, slot)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_MANAGEMENT);
        }
    }

    pub fn update_simulation(
        &mut self,
        slot: fuji::CustomSetting,
        modifier: &mut dyn FnMut(&mut dyn Simulation) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if let Some(sim) = self.r#impl.as_simulation_manager() {
            sim.update_simulation(&mut self.ptp, slot, modifier)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_MANAGEMENT);
        }
    }

    pub fn set_simulation(
        &mut self,
        slot: fuji::CustomSetting,
        simulation: &dyn Simulation,
    ) -> anyhow::Result<()> {
        if let Some(sim) = self.r#impl.as_simulation_manager() {
            sim.set_simulation(&mut self.ptp, slot, simulation)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_MANAGEMENT);
        }
    }

    pub fn render(
        &mut self,
        image: &[u8],
        conversion_profile_modifier: &mut dyn FnMut(
            &mut dyn ConversionProfile,
        ) -> anyhow::Result<()>,
        draft: bool,
    ) -> anyhow::Result<Vec<u8>> {
        if let Some(renders) = self.r#impl.as_render_manager() {
            renders.render(&mut self.ptp, image, conversion_profile_modifier, draft)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_RENDER_MANAGEMENT);
        }
    }
}
