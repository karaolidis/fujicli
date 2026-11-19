use crate::{
    features::{render::ConversionProfileFields, simulation::SimulationError},
    generated::{
        options::*,
        renders::{XT5RenderProfile, XT5RenderProfileDraft, x_t5},
    },
    ptp::option::ConversionProfileField,
};

const DR: usize = 5;
const DRP: usize = 6;
const WB_AS_SHOT: usize = 10;
const WB: usize = 11;
const WB_SHIFT_RED: usize = 12;
const WB_SHIFT_BLUE: usize = 13;
const HIGHLIGHT: usize = 15;
const SHADOW: usize = 16;

fn shot() -> XT5RenderProfile {
    XT5RenderProfile {
        file_type: FileType::Jpeg,
        image_size: ImageSize::L3_2,
        image_quality: ImageQuality::Fine,
        exposure_offset: ExposureOffset::Zero,
        dynamic_range: Some(DynamicRange::Hdr400),
        dynamic_range_priority: DynamicRangePriority::Off,
        film_simulation: FilmSimulation::Provia,
        grain_effect: GrainEffect::Off,
        color_chrome_effect: ColorChromeEffect::Off,
        white_balance: WhiteBalance::Auto,
        white_balance_shift_red: WhiteBalanceShiftRed::default(),
        white_balance_shift_blue: WhiteBalanceShiftBlue::default(),
        white_balance_temperature: None,
        highlight_tone: Some(HighlightTone::default()),
        shadow_tone: Some(ShadowTone::default()),
        color: Some(Color::default()),
        sharpness: Sharpness::default(),
        noise_reduction: NoiseReduction::Zero,
        lens_modulation_optimizer: LensModulationOptimizer::On,
        color_space: ColorSpace::Srgb,
        monochromatic_color_temperature: None,
        smooth_skin_effect: SmoothSkinEffect::Off,
        color_chrome_fx_blue: ColorChromeFxBlue::Off,
        monochromatic_color_tint: None,
        clarity: Clarity::default(),
        teleconverter: Teleconverter::Off,
    }
}

fn wire(edits: &[(usize, i32)]) -> Vec<i32> {
    let mut values = vec![0; XT5RenderProfile::LEN];
    shot().write_fields(&mut values).unwrap();
    for &(position, value) in edits {
        values[position] = value;
    }
    values
}

fn hdr800_plus() -> Vec<i32> {
    wire(&[(DR, 0x320), (DRP, 3), (HIGHLIGHT, -20), (SHADOW, -20)])
}

fn strong() -> Vec<i32> {
    wire(&[(DRP, 2), (HIGHLIGHT, 10), (SHADOW, 10)])
}

fn read(values: &[i32]) -> XT5RenderProfileDraft {
    XT5RenderProfileDraft::from(&XT5RenderProfile::read_fields(values).unwrap())
}

fn render(values: &[i32], partial: &XT5RenderProfileDraft) -> Result<Vec<i32>, SimulationError> {
    let original = read(values);
    let mut buf = original.clone();
    x_t5::try_update_from(&mut buf, partial, &original)?;
    let mut out = values.to_vec();
    XT5RenderProfile::try_from(buf)?
        .write_fields(&mut out)
        .unwrap();
    Ok(out)
}

fn raw<T: ConversionProfileField>(value: &T) -> i32 {
    value.try_into_conversion_profile_value().unwrap()
}

fn tone(value: f32) -> i32 {
    raw(&HighlightTone::try_from(value).unwrap())
}

#[test]
fn reads_hdr800_plus_as_one_dynamic_range_with_priority_off() {
    let draft = read(&hdr800_plus());
    assert_eq!(draft.dynamic_range, Some(DynamicRange::Hdr800Plus));
    assert_eq!(
        draft.dynamic_range_priority,
        Some(DynamicRangePriority::Off)
    );
    assert_eq!(draft.highlight_tone, None);
    assert_eq!(draft.shadow_tone, None);
}

#[test]
fn renders_hdr800_plus_as_shot() {
    let out = render(&hdr800_plus(), &XT5RenderProfileDraft::default()).unwrap();
    assert_eq!((out[DR], out[DRP]), (0x320, 3));
    assert_eq!((out[HIGHLIGHT], out[SHADOW]), (0, 0));
}

#[test]
fn reads_plain_hdr800_apart_from_hdr800_plus() {
    let values = wire(&[(DR, 0x320)]);
    assert_eq!(read(&values).dynamic_range, Some(DynamicRange::Hdr800));
    let out = render(&values, &XT5RenderProfileDraft::default()).unwrap();
    assert_eq!((out[DR], out[DRP]), (0x320, 0));
}

#[test]
fn rejects_the_hdr800_plus_priority_without_its_dynamic_range() {
    let err = XT5RenderProfile::read_fields(&wire(&[(DRP, 3)])).unwrap_err();
    assert!(err.to_string().contains("position 6 holds 3"), "{err}");
}

#[test]
fn does_not_read_a_dynamic_range_hidden_by_priority() {
    let draft = read(&wire(&[(DR, 12345), (DRP, 2)]));
    assert_eq!(draft.dynamic_range, None);
    assert!(XT5RenderProfile::read_fields(&wire(&[(DR, 12345)])).is_err());
}

#[test]
fn lowers_hdr800_plus_to_hdr800() {
    let partial = XT5RenderProfileDraft {
        dynamic_range: Some(DynamicRange::Hdr800),
        ..Default::default()
    };
    let out = render(&hdr800_plus(), &partial).unwrap();
    assert_eq!((out[DR], out[DRP]), (0x320, 0));
}

#[test]
fn auto_keeps_the_shot_value() {
    let partial = XT5RenderProfileDraft {
        dynamic_range: Some(DynamicRange::Auto),
        dynamic_range_priority: Some(DynamicRangePriority::Auto),
        ..Default::default()
    };
    let out = render(&wire(&[]), &partial).unwrap();
    assert_eq!((out[DR], out[DRP]), (0x190, 0));
    let out = render(&strong(), &partial).unwrap();
    assert_eq!((out[DR], out[DRP]), (0, 2));
}

#[test]
fn raw_qualities_render_their_jpeg_part() {
    let partial = XT5RenderProfileDraft {
        image_quality: Some(ImageQuality::FineRaw),
        ..Default::default()
    };
    let out = render(&wire(&[]), &partial).unwrap();
    assert_eq!(out[3], raw(&ImageQuality::Fine));

    let normal_raw = wire(&[(3, raw(&ImageQuality::NormalRaw))]);
    let partial = XT5RenderProfileDraft {
        image_quality: Some(ImageQuality::Raw),
        ..Default::default()
    };
    let out = render(&normal_raw, &partial).unwrap();
    assert_eq!(out[3], raw(&ImageQuality::Normal));
}

#[test]
fn repairs_an_inherited_auto() {
    let values = wire(&[(DR, 0xffff), (DRP, 0x8000)]);
    let out = render(&values, &XT5RenderProfileDraft::default()).unwrap();
    assert_eq!((out[DR], out[DRP]), (0, 0));
}

#[test]
fn keeps_white_balance_as_shot_with_its_shifts() {
    let values = wire(&[
        (WB_AS_SHOT, 1),
        (WB, 0),
        (WB_SHIFT_RED, 3),
        (WB_SHIFT_BLUE, -2),
    ]);
    assert_eq!(read(&values).white_balance, Some(WhiteBalance::AsShot));
    let out = render(&values, &XT5RenderProfileDraft::default()).unwrap();
    assert_eq!(&out[WB_AS_SHOT..=WB_SHIFT_BLUE], &[1, 0, 3, -2]);

    let partial = XT5RenderProfileDraft {
        white_balance: Some(WhiteBalance::Daylight),
        ..Default::default()
    };
    let out = render(&values, &partial).unwrap();
    assert_eq!(
        (out[WB_AS_SHOT], out[WB]),
        (2, raw(&WhiteBalance::Daylight))
    );
}

#[test]
fn turning_priority_off_leaves_neutral_tones() {
    let partial = XT5RenderProfileDraft {
        dynamic_range_priority: Some(DynamicRangePriority::Off),
        ..Default::default()
    };
    let out = render(&strong(), &partial).unwrap();
    assert_eq!(
        (out[DR], out[DRP], out[HIGHLIGHT], out[SHADOW]),
        (0, 0, 0, 0)
    );
}

#[test]
fn a_tone_turns_priority_off() {
    let partial = XT5RenderProfileDraft {
        highlight_tone: Some(HighlightTone::try_from(1.0).unwrap()),
        ..Default::default()
    };
    let out = render(&strong(), &partial).unwrap();
    assert_eq!((out[DRP], out[HIGHLIGHT]), (0, tone(1.0)));
}

#[test]
fn a_tone_drops_hdr800_plus() {
    let partial = XT5RenderProfileDraft {
        highlight_tone: Some(HighlightTone::try_from(1.0).unwrap()),
        ..Default::default()
    };
    let out = render(&hdr800_plus(), &partial).unwrap();
    assert_eq!((out[DR], out[DRP], out[HIGHLIGHT]), (0, 0, tone(1.0)));
}

#[test]
fn a_recipe_with_auto_and_tones_drops_hdr800_plus() {
    let partial = XT5RenderProfileDraft {
        dynamic_range: Some(DynamicRange::Auto),
        dynamic_range_priority: Some(DynamicRangePriority::Off),
        highlight_tone: Some(HighlightTone::try_from(1.0).unwrap()),
        shadow_tone: Some(ShadowTone::try_from(1.0).unwrap()),
        ..Default::default()
    };
    let out = render(&hdr800_plus(), &partial).unwrap();
    assert_eq!(
        (out[DR], out[DRP], out[HIGHLIGHT], out[SHADOW]),
        (0, 0, tone(1.0), tone(1.0))
    );
}

#[test]
fn refuses_tones_with_hdr800_plus_asked_for() {
    let partial = XT5RenderProfileDraft {
        dynamic_range: Some(DynamicRange::Hdr800Plus),
        highlight_tone: Some(HighlightTone::try_from(1.0).unwrap()),
        ..Default::default()
    };
    let shot = wire(&[(DR, 0x320)]);
    assert!(render(&shot, &partial).is_err());
}

#[test]
fn a_priority_drops_hdr800_plus() {
    let partial = XT5RenderProfileDraft {
        dynamic_range_priority: Some(DynamicRangePriority::Weak),
        ..Default::default()
    };
    let out = render(&hdr800_plus(), &partial).unwrap();
    assert_eq!((out[DR], out[DRP]), (0, 1));
}
