use std::sync::atomic::{AtomicU32, Ordering};

use super::{CameraId, CameraImpl};

pub const FUJIFILM_XT5: CameraId = CameraId {
    vendor: 0x04cb,
    product: 0x02fc,
};

#[derive(Debug)]
pub struct FujifilmXT5 {
    session_counter: AtomicU32,
}

impl FujifilmXT5 {
    pub fn new() -> Self {
        Self {
            session_counter: AtomicU32::new(1),
        }
    }
}

impl CameraImpl for FujifilmXT5 {
    fn name(&self) -> &'static str {
        "FUJIFILM X-T5"
    }

    fn next_session_id(&self) -> u32 {
        self.session_counter.fetch_add(1, Ordering::SeqCst)
    }
}
