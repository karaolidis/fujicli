use crate::generated::{
    options::*,
    simulations::{XT5Simulation, XT5SimulationDraft, x_t5},
};

const RULE_A: &str = "Dynamic Range can only be set when Dynamic Range Priority is disabled.";

fn slot() -> XT5SimulationDraft {
    XT5SimulationDraft {
        custom_setting_name: Some("C1".parse().unwrap()),
        image_size: Some(ImageSize::L3_2),
        image_quality: Some(ImageQuality::Fine),
        film_simulation: Some(FilmSimulation::Provia),
        grain_effect: Some(GrainEffect::Off),
        color_chrome_effect: Some(ColorChromeEffect::Off),
        color_chrome_fx_blue: Some(ColorChromeFxBlue::Off),
        white_balance: Some(WhiteBalance::Auto),
        white_balance_shift_red: Some(WhiteBalanceShiftRed::default()),
        white_balance_shift_blue: Some(WhiteBalanceShiftBlue::default()),
        highlight_tone: Some(HighlightTone::default()),
        shadow_tone: Some(ShadowTone::default()),
        color: Some(Color::default()),
        sharpness: Some(Sharpness::default()),
        noise_reduction: Some(NoiseReduction::Zero),
        clarity: Some(Clarity::default()),
        lens_modulation_optimizer: Some(LensModulationOptimizer::On),
        color_space: Some(ColorSpace::Srgb),
        dynamic_range: Some(DynamicRange::Hdr400),
        dynamic_range_priority: Some(DynamicRangePriority::Off),
        smooth_skin_effect: Some(SmoothSkinEffect::Off),
        ..Default::default()
    }
}

#[test]
fn a_file_that_breaks_a_rule_is_refused() {
    let mut file = serde_json::to_value(slot()).unwrap();
    file["dynamicRangePriority"] = "strong".into();
    let err = serde_json::from_value::<XT5Simulation>(file).unwrap_err();
    assert!(err.to_string().contains(RULE_A), "{err}");
}

#[test]
fn a_valid_file_is_taken_as_is() {
    let file = serde_json::to_value(slot()).unwrap();
    let simulation = serde_json::from_value::<XT5Simulation>(file).unwrap();
    assert_eq!(simulation.dynamic_range, Some(DynamicRange::Hdr400));
}

#[test]
fn reading_a_slot_does_not_repair_it() {
    let mut pulled = slot();
    pulled.dynamic_range = Some(DynamicRange::Hdr800Plus);
    let simulation = XT5Simulation::try_from(pulled).unwrap();
    assert_eq!(simulation.dynamic_range, Some(DynamicRange::Hdr800Plus));
}

#[test]
fn an_update_repairs_what_the_request_leaves_unset() {
    let mut draft = slot();
    let partial = XT5SimulationDraft {
        dynamic_range_priority: Some(DynamicRangePriority::Strong),
        ..Default::default()
    };
    x_t5::try_update_from(&mut draft, &partial).unwrap();
    assert_eq!(draft.dynamic_range, None);
    assert_eq!(draft.highlight_tone, None);
}
