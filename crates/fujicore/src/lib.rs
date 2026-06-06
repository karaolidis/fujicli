pub mod error;
pub mod features;
pub mod generated;
pub mod input;
pub mod ptp;

pub use error::{Capability, CoreError, CoreResult};

use features::{
    base::{CameraBase, info::CameraInfo},
    simulation::{Simulation, SimulationDescriptors},
};
use log::{debug, error};
use ptp::Ptp;
use rusb::{GlobalContext, constants::LIBUSB_CLASS_IMAGE};
use serde::{Deserialize, Serialize};

use crate::{
    features::base::UNKNOWN_CAMERA,
    generated::{
        cameras::SUPPORTED, options::CustomSetting, renders::RenderBase,
        simulations::SimulationBase,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

impl std::fmt::Display for UsbId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor, self.product)
    }
}

const SESSION: u32 = 1;

pub struct Camera {
    pub ptp: Ptp,
    r#impl: Box<dyn CameraBase<Context = GlobalContext>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Supported,
    Emulated { vendor: u16, product: u16 },
    Unknown,
}

impl Camera {
    pub fn probe(device: &rusb::Device<GlobalContext>) -> CoreResult<bool> {
        let descriptor = device.device_descriptor()?;
        let usb_id = UsbId {
            vendor: descriptor.vendor_id(),
            product: descriptor.product_id(),
        };
        Ok(SUPPORTED.iter().any(|c| c.usb_id == usb_id))
    }

    pub fn open_with(mode: CameraMode, device: &rusb::Device<GlobalContext>) -> CoreResult<Self> {
        let descriptor = device.device_descriptor()?;

        let usb_id = match mode {
            CameraMode::Supported | CameraMode::Unknown => UsbId {
                vendor: descriptor.vendor_id(),
                product: descriptor.product_id(),
            },
            CameraMode::Emulated { vendor, product } => UsbId { vendor, product },
        };

        let factory = match mode {
            CameraMode::Supported | CameraMode::Emulated { .. } => SUPPORTED
                .iter()
                .find(|c| c.usb_id == usb_id)
                .map(|c| {
                    debug!("Found supported camera: {}", c.name);
                    c.camera_factory
                })
                .ok_or(CoreError::DeviceUnsupported(usb_id))?,
            CameraMode::Unknown => UNKNOWN_CAMERA.camera_factory,
        };

        let bus = device.bus_number();
        let address = device.address();

        let config_descriptor = device.active_config_descriptor()?;
        let interface_descriptor = config_descriptor
            .interfaces()
            .flat_map(|i| i.descriptors())
            .find(|x| x.class_code() == LIBUSB_CLASS_IMAGE)
            .ok_or(CoreError::NoImagingInterface)?;

        let interface = interface_descriptor.interface_number();
        debug!("Found interface {interface}");

        let handle = device.open()?;
        handle.claim_interface(interface)?;
        debug!("Claimed interface");

        let find_endpoint = |direction: rusb::Direction,
                             transfer_type: rusb::TransferType|
         -> Result<u8, rusb::Error> {
            interface_descriptor
                .endpoint_descriptors()
                .find(|ep| ep.direction() == direction && ep.transfer_type() == transfer_type)
                .map(|x| x.address())
                .ok_or(rusb::Error::NotFound)
        };

        let bulk_in = find_endpoint(rusb::Direction::In, rusb::TransferType::Bulk)?;
        debug!("Found Bulk In endpoint");

        let bulk_out = find_endpoint(rusb::Direction::Out, rusb::TransferType::Bulk)?;
        debug!("Found Bulk Out endpoint");

        let transaction_id = 0;
        let r#impl = (factory)();
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

    pub fn open(device: &rusb::Device<GlobalContext>) -> CoreResult<Self> {
        Self::open_with(CameraMode::Supported, device)
    }

    pub fn open_as(
        device: &rusb::Device<GlobalContext>,
        vendor: u16,
        product: u16,
    ) -> CoreResult<Self> {
        Self::open_with(CameraMode::Emulated { vendor, product }, device)
    }

    pub fn open_unknown(device: &rusb::Device<GlobalContext>) -> CoreResult<Self> {
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

type CameraFactory = fn() -> Box<dyn CameraBase<Context = GlobalContext>>;

#[derive(Debug, Clone, Copy)]
pub struct SupportedCamera {
    pub name: &'static str,
    pub usb_id: UsbId,
    pub camera_factory: CameraFactory,
    pub simulation: Option<&'static SimulationDescriptors>,
}

impl Camera {
    pub fn name(&self) -> &'static str {
        self.r#impl.camera_definition().name
    }

    pub fn usb_id(&self) -> UsbId {
        self.r#impl.camera_definition().usb_id
    }

    pub fn bus_address(&self) -> String {
        format!("{}.{}", self.ptp.bus, self.ptp.address)
    }

    pub fn capabilities(&self) -> &'static [Capability] {
        self.r#impl.capabilities()
    }

    pub fn supports(&self, capability: Capability) -> bool {
        self.capabilities().contains(&capability)
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

    pub fn simulation_descriptors(&self) -> Option<&'static SimulationDescriptors> {
        self.r#impl.camera_definition().simulation
    }

    pub fn validate_simulation(&self, base: SimulationBase) -> CoreResult<SimulationBase> {
        let descriptors = self
            .simulation_descriptors()
            .ok_or(CoreError::Unsupported(Capability::SimulationManagement))?;
        (descriptors.validate)(base).map_err(Into::into)
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

    pub fn render(
        &mut self,
        image: &[u8],
        partial: &RenderBase,
        draft: bool,
    ) -> CoreResult<Vec<u8>> {
        let renders = self
            .r#impl
            .as_render_manager()
            .ok_or(CoreError::Unsupported(Capability::RenderManagement))?;
        renders.render(&mut self.ptp, image, partial, draft)
    }
}
