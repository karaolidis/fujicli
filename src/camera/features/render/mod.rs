pub mod conversion;

use crate::camera::{features::render::conversion::ConversionProfile, ptp::Ptp};

use super::base::CameraBase;

pub trait CameraRenders: CameraBase {
    fn render(
        &self,
        ptp: &mut Ptp,
        image: &[u8],
        conversion_profile_modifier: &mut dyn FnMut(
            &mut dyn ConversionProfile,
        ) -> anyhow::Result<()>,
        draft: bool,
    ) -> anyhow::Result<Vec<u8>>;
}
