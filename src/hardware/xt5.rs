use std::{
    error::Error,
    sync::atomic::{AtomicU32, Ordering},
};

use rusb::GlobalContext;

use super::{CameraId, CameraImpl};

pub const FUJIFILM_XT5: CameraId = CameraId {
    name: "FUJIFILM XT-5",
    vendor: 0x04cb,
    product: 0x02fc,
};

pub struct FujifilmXT5 {
    device: rusb::Device<GlobalContext>,
    session_counter: AtomicU32,
}

impl FujifilmXT5 {
    pub fn new_boxed(
        rusb_device: rusb::Device<GlobalContext>,
    ) -> Result<Box<dyn CameraImpl>, Box<dyn Error + Send + Sync>> {
        let session_counter = AtomicU32::new(1);

        let handle = rusb_device.open()?;
        let device = handle.device();

        Ok(Box::new(Self {
            session_counter,
            device,
        }))
    }
}

impl CameraImpl for FujifilmXT5 {
    fn id(&self) -> &'static CameraId {
        &FUJIFILM_XT5
    }

    fn usb_id(&self) -> String {
        let bus = self.device.bus_number();
        let address = self.device.address();
        format!("{}.{}", bus, address)
    }

    fn ptp(&self) -> libptp::Camera<rusb::GlobalContext> {
        libptp::Camera::new(&self.device).unwrap()
    }

    fn next_session_id(&self) -> u32 {
        self.session_counter.fetch_add(1, Ordering::SeqCst)
    }
}
