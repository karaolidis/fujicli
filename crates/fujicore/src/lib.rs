pub mod error;
pub mod features;
pub mod generated;
pub mod input;
pub mod ptp;
pub mod usb;

pub use error::{Capability, CoreError, CoreResult};
pub use usb::{ParseUsbIdError, UsbId};

use features::{
    base::{CameraBase, info::CameraInfo},
    simulation::Simulation,
};
use log::debug;
use ptp::{Ptp, transport::Device};

use crate::{
    features::base::UNKNOWN_CAMERA,
    generated::{
        cameras::SUPPORTED, options::CustomSetting, renders::RenderBase,
        simulations::SimulationBase,
    },
};

pub struct Camera {
    pub ptp: Ptp,
    r#impl: Box<dyn CameraBase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Supported,
    Emulated(UsbId),
    Unknown,
}

impl Camera {
    #[must_use]
    pub fn probe(device: &Device) -> bool {
        SupportedCamera::find(device.usb_id()).is_some()
    }

    pub fn open_with(mode: CameraMode, device: &Device) -> CoreResult<Self> {
        let usb_id = match mode {
            CameraMode::Supported | CameraMode::Unknown => device.usb_id(),
            CameraMode::Emulated(usb_id) => usb_id,
        };

        let factory = match mode {
            CameraMode::Supported | CameraMode::Emulated(_) => SupportedCamera::find(usb_id)
                .map(|c| {
                    debug!("Found supported camera: {}", c.name);
                    c.camera_factory
                })
                .ok_or(CoreError::DeviceUnsupported(usb_id))?,
            CameraMode::Unknown => UNKNOWN_CAMERA.camera_factory,
        };

        let r#impl = (factory)();
        let transport = device.open(r#impl.chunk_size())?;

        Ok(Self {
            ptp: Ptp::new(transport),
            r#impl,
        })
    }

    pub fn open(device: &Device) -> CoreResult<Self> {
        Self::open_with(CameraMode::Supported, device)
    }

    pub fn open_as(device: &Device, usb_id: UsbId) -> CoreResult<Self> {
        Self::open_with(CameraMode::Emulated(usb_id), device)
    }

    pub fn open_unknown(device: &Device) -> CoreResult<Self> {
        Self::open_with(CameraMode::Unknown, device)
    }
}

type CameraFactory = fn() -> Box<dyn CameraBase>;

#[derive(Debug, Clone, Copy)]
pub struct SupportedCamera {
    pub name: &'static str,
    pub usb_id: UsbId,
    pub camera_factory: CameraFactory,
}

impl SupportedCamera {
    #[must_use]
    pub fn find(usb_id: UsbId) -> Option<&'static Self> {
        SUPPORTED.iter().find(|c| c.usb_id == usb_id)
    }
}

impl Camera {
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.r#impl.camera_definition().name
    }

    #[must_use]
    pub fn usb_id(&self) -> UsbId {
        self.r#impl.camera_definition().usb_id
    }

    pub fn get_info(&mut self) -> CoreResult<Box<dyn CameraInfo>> {
        self.r#impl.get_info(&mut self.ptp)
    }

    pub fn export_backup(&mut self) -> CoreResult<Vec<u8>> {
        let backups = self
            .r#impl
            .as_backup_manager()
            .ok_or(CoreError::Unsupported(Capability::BackupManagement))?;
        backups.export_backup(&mut self.ptp)
    }

    pub fn import_backup(&mut self, buffer: &[u8]) -> CoreResult<()> {
        let backups = self
            .r#impl
            .as_backup_manager()
            .ok_or(CoreError::Unsupported(Capability::BackupManagement))?;
        backups.import_backup(&mut self.ptp, buffer)
    }

    pub fn serialize_simulation(&self, simulation: &dyn Simulation) -> CoreResult<Vec<u8>> {
        let parser = self
            .r#impl
            .as_simulation_parser()
            .ok_or(CoreError::Unsupported(Capability::SimulationParsing))?;
        parser.serialize_simulation(simulation)
    }

    pub fn deserialize_simulation(&self, simulation: &[u8]) -> CoreResult<Box<dyn Simulation>> {
        let parser = self
            .r#impl
            .as_simulation_parser()
            .ok_or(CoreError::Unsupported(Capability::SimulationParsing))?;
        parser.deserialize_simulation(simulation)
    }

    pub fn custom_settings_slots(&self) -> CoreResult<Vec<CustomSetting>> {
        let sim = self
            .r#impl
            .as_simulation_manager()
            .ok_or(CoreError::Unsupported(Capability::SimulationManagement))?;
        Ok(sim.custom_settings_slots())
    }

    pub fn get_simulation(&mut self, slot: CustomSetting) -> CoreResult<Box<dyn Simulation>> {
        let sim = self
            .r#impl
            .as_simulation_manager()
            .ok_or(CoreError::Unsupported(Capability::SimulationManagement))?;
        sim.get_simulation(&mut self.ptp, slot)
    }

    pub fn update_simulation(
        &mut self,
        slot: CustomSetting,
        partial: SimulationBase,
    ) -> CoreResult<()> {
        let sim = self
            .r#impl
            .as_simulation_manager()
            .ok_or(CoreError::Unsupported(Capability::SimulationManagement))?;
        sim.update_simulation(&mut self.ptp, slot, partial)
    }

    pub fn set_simulation(
        &mut self,
        slot: CustomSetting,
        simulation: &dyn Simulation,
    ) -> CoreResult<()> {
        let sim = self
            .r#impl
            .as_simulation_manager()
            .ok_or(CoreError::Unsupported(Capability::SimulationManagement))?;
        sim.set_simulation(&mut self.ptp, slot, simulation)
    }

    pub fn send_image(&mut self, image: &[u8]) -> CoreResult<()> {
        let renders = self
            .r#impl
            .as_render_manager()
            .ok_or(CoreError::Unsupported(Capability::RenderManagement))?;
        renders.send_image(&mut self.ptp, image)
    }

    pub fn read_profile(&mut self) -> CoreResult<RenderBase> {
        let renders = self
            .r#impl
            .as_render_manager()
            .ok_or(CoreError::Unsupported(Capability::RenderManagement))?;
        renders.read_profile(&mut self.ptp)
    }

    pub fn apply_profile(&mut self, partial: &RenderBase) -> CoreResult<()> {
        let renders = self
            .r#impl
            .as_render_manager()
            .ok_or(CoreError::Unsupported(Capability::RenderManagement))?;
        renders.apply_profile(&mut self.ptp, partial)
    }

    pub fn render(&mut self, draft: bool) -> CoreResult<Vec<u8>> {
        let renders = self
            .r#impl
            .as_render_manager()
            .ok_or(CoreError::Unsupported(Capability::RenderManagement))?;
        renders.render(&mut self.ptp, draft)
    }
}
