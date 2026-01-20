use crate::{
    features::{
        base::CameraBase, render::ConversionProfile, simulation::parser::CameraSimulationParser,
    },
    ptp::Ptp,
};

pub trait CameraRenderManager: CameraBase + CameraSimulationParser {
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
