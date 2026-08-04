pub mod features;
pub mod generated;
pub mod input;
pub mod ptp;

use anyhow::{anyhow, bail};
use features::{
    base::{CameraBase, info::CameraInfo},
    simulation::Simulation,
};
use log::{debug, error};
use ptp::{
    Ptp,
    transport::{self, Device, TransportKind},
};

use crate::{
    features::base::UNKNOWN_CAMERA,
    generated::{
        cameras::SUPPORTED, options::CustomSetting, renders::RenderBase,
        simulations::SimulationBase,
    },
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
    r#impl: Box<dyn CameraBase>,
}

pub enum CameraMode {
    Supported,
    Emulated { vendor: u16, product: u16 },
    Unknown,
}

/// Enumerate every supported camera visible to `transport`.
///
/// With [`TransportKind::Auto`] each transport is tried in preference order and
/// the first one that reports a supported camera wins, so Windows users on the
/// stock MTP driver and Zadig/WinUSB users are both served.
pub fn list_devices(transport: TransportKind) -> anyhow::Result<Vec<Device>> {
    for kind in transport.candidates() {
        let devices: Vec<Device> = transport::enumerate(kind)
            .unwrap_or_default()
            .into_iter()
            .filter(Camera::probe)
            .collect();

        if !devices.is_empty() {
            debug!("Using {kind} transport");
            return Ok(devices);
        }
    }

    Ok(Vec::new())
}

/// Resolve a `-d` selector against `transport`.
///
/// The selector is opaque and transport-defined: `<BUS>.<ADDRESS>` for libusb,
/// a WPD device ID (or an unambiguous substring of one) for WPD.
pub fn find_device(transport: TransportKind, id: &str) -> anyhow::Result<Device> {
    let needle = id.to_ascii_lowercase();
    let mut partial: Vec<Device> = Vec::new();

    for kind in transport.candidates() {
        for device in transport::enumerate(kind).unwrap_or_default() {
            if device.id().eq_ignore_ascii_case(id) {
                return Ok(device);
            }

            if device.id().to_ascii_lowercase().contains(&needle) {
                partial.push(device);
            }
        }
    }

    if partial.len() > 1 {
        bail!("Device selector {id} is ambiguous, use the full device ID");
    }

    partial
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No device found matching {id}"))
}

impl Camera {
    #[must_use]
    pub fn probe(device: &Device) -> bool {
        SUPPORTED
            .iter()
            .any(|c| c.vendor == device.vendor() && c.product == device.product())
    }

    pub fn open_with(mode: CameraMode, device: &Device) -> anyhow::Result<Self> {
        let (vendor, product) = match mode {
            CameraMode::Supported | CameraMode::Unknown => (device.vendor(), device.product()),
            CameraMode::Emulated { vendor, product } => (vendor, product),
        };

        let factory = match mode {
            CameraMode::Supported | CameraMode::Emulated { .. } => SUPPORTED
                .iter()
                .find(|c| c.vendor == vendor && c.product == product)
                .map(|c| {
                    debug!("Found supported camera: {}", c.name);
                    c.camera_factory
                })
                .ok_or_else(|| anyhow!(ERROR_DEVICE_NOT_SUPPORTED))?,
            CameraMode::Unknown => UNKNOWN_CAMERA.camera_factory,
        };

        debug!("Opening {device:?}");
        let mut transport = device.open()?;

        let r#impl = (factory)();
        transport.set_chunk_size(r#impl.chunk_size());

        let mut ptp = Ptp::new(transport);
        ptp.open_session(SESSION)?;

        Ok(Self { ptp, r#impl })
    }

    pub fn open(device: &Device) -> anyhow::Result<Self> {
        Self::open_with(CameraMode::Supported, device)
    }

    pub fn open_as(device: &Device, vendor: u16, product: u16) -> anyhow::Result<Self> {
        Self::open_with(CameraMode::Emulated { vendor, product }, device)
    }

    pub fn open_unknown(device: &Device) -> anyhow::Result<Self> {
        Self::open_with(CameraMode::Unknown, device)
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        if let Err(error) = self.ptp.close_session(SESSION) {
            error!("Error closing session: {error}");
        }
    }
}

type CameraFactory = fn() -> Box<dyn CameraBase>;

#[derive(Debug, Clone, Copy)]
pub struct SupportedCamera {
    pub name: &'static str,
    pub vendor: u16,
    pub product: u16,
    pub camera_factory: CameraFactory,
}

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
        self.ptp.id()
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

    pub fn custom_settings_slots(&self) -> anyhow::Result<Vec<CustomSetting>> {
        if let Some(sim) = self.r#impl.as_simulation_manager() {
            Ok(sim.custom_settings_slots())
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_MANAGEMENT);
        }
    }

    pub fn get_simulation(&mut self, slot: CustomSetting) -> anyhow::Result<Box<dyn Simulation>> {
        if let Some(sim) = self.r#impl.as_simulation_manager() {
            sim.get_simulation(&mut self.ptp, slot)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_MANAGEMENT);
        }
    }

    pub fn update_simulation(
        &mut self,
        slot: CustomSetting,
        partial: SimulationBase,
    ) -> anyhow::Result<()> {
        if let Some(sim) = self.r#impl.as_simulation_manager() {
            sim.update_simulation(&mut self.ptp, slot, partial)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_SIMULATION_MANAGEMENT);
        }
    }

    pub fn set_simulation(
        &mut self,
        slot: CustomSetting,
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
        partial: RenderBase,
        draft: bool,
    ) -> anyhow::Result<Vec<u8>> {
        if let Some(renders) = self.r#impl.as_render_manager() {
            renders.render(&mut self.ptp, image, partial, draft)
        } else {
            bail!(ERROR_CAMERA_DOES_NOT_SUPPORT_RENDER_MANAGEMENT);
        }
    }
}
