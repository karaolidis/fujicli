//! `MakerNotes` values -> simulation settings.
//!
//! Encodings follow `ExifTool`'s `FujiFilm` tables and were checked against camera renders
//! (one render per setting value on an X-T30 III).

use std::collections::HashMap;

use byteorder::{ByteOrder, LittleEndian};
use log::{debug, warn};
use serde_json::{Map, Value};

use super::{FujiExifMakerNoteTag as T, IFDType, ImageError};
use crate::generated::simulations::SimulationBase;

pub struct MakerNotes<'a>(pub HashMap<T, (IFDType, &'a [u8])>);

impl MakerNotes<'_> {
    /// All integer values of a tag, read with its declared type.
    fn ints(&self, tag: T) -> Option<Vec<i64>> {
        let (field_type, bytes) = self.0.get(&tag)?;
        let values = bytes.chunks_exact(field_type.size());
        Some(match field_type {
            IFDType::Byte | IFDType::Undefined => values.map(|b| i64::from(b[0])).collect(),
            IFDType::SByte => values
                .map(|b| i64::from(i8::from_le_bytes([b[0]])))
                .collect(),
            IFDType::Short => values
                .map(|b| i64::from(LittleEndian::read_u16(b)))
                .collect(),
            IFDType::SShort => values
                .map(|b| i64::from(LittleEndian::read_i16(b)))
                .collect(),
            IFDType::Long => values
                .map(|b| i64::from(LittleEndian::read_u32(b)))
                .collect(),
            IFDType::SLong => values
                .map(|b| i64::from(LittleEndian::read_i32(b)))
                .collect(),
            _ => return None,
        })
    }

    fn int(&self, tag: T) -> Option<i64> {
        self.ints(tag)?.first().copied()
    }

    /// One byte that `ExifTool` reads as signed although it is stored as unsigned.
    fn int8(&self, tag: T) -> Option<i64> {
        let (_, bytes) = self.0.get(&tag)?;
        bytes.first().map(|b| i64::from(i8::from_le_bytes([*b])))
    }
}

fn find<V: Copy>(table: &[(i64, V)], raw: i64) -> Option<V> {
    table.iter().find(|(k, _)| *k == raw).map(|(_, v)| *v)
}

const FILM_MODE: &[(i64, &str)] = &[
    (0x000, "provia"),
    (0x120, "astia"),
    (0x200, "velvia"),
    (0x500, "pro_neg_std"),
    (0x501, "pro_neg_hi"),
    (0x600, "classic_chrome"),
    (0x700, "eterna"),
    (0x800, "classic_negative"),
    (0x900, "eterna_bleach_bypass"),
    (0xa00, "nostalgic_negative"),
    (0xb00, "reala_ace"),
];
// Monochrome simulations have no FilmMode: Saturation carries them.
const MONOCHROME: &[(i64, &str)] = &[
    (0x300, "monochrome"),
    (0x301, "monochrome_r"),
    (0x302, "monochrome_ye"),
    (0x303, "monochrome_g"),
    (0x310, "sepia"),
    (0x500, "acros"),
    (0x501, "acros_r"),
    (0x502, "acros_ye"),
    (0x503, "acros_g"),
];
const COLOR: &[(i64, i64)] = &[
    (0x4e0, -4),
    (0x4c0, -3),
    (0x400, -2),
    (0x180, -1),
    (0x000, 0),
    (0x080, 1),
    (0x100, 2),
    (0x0c0, 3),
    (0x0e0, 4),
];
const SHARPNESS: &[(i64, i64)] = &[
    (0x00, -4),
    (0x01, -3),
    (0x02, -2),
    (0x82, -1),
    (0x03, 0),
    (0x84, 1),
    (0x04, 2),
    (0x05, 3),
    (0x06, 4),
];
const NOISE_REDUCTION: &[(i64, i64)] = &[
    (0x2e0, -4),
    (0x2c0, -3),
    (0x200, -2),
    (0x280, -1),
    (0x000, 0),
    (0x180, 1),
    (0x100, 2),
    (0x1c0, 3),
    (0x1e0, 4),
];
const WHITE_BALANCE: &[(i64, &str)] = &[
    (0x000, "auto"),
    (0x001, "white_priority"),
    (0x002, "ambience_priority"),
    (0x100, "daylight"),
    (0x200, "shade"),
    (0x300, "fluorescent1"),
    (0x301, "fluorescent2"),
    (0x302, "fluorescent3"),
    (0x400, "incandescent"),
    (0x600, "underwater"),
    (0xf00, "custom1"),
    (0xf01, "custom2"),
    (0xf02, "custom3"),
    (0xff0, "temperature"),
];
const LEVEL: &[(i64, &str)] = &[(0, "off"), (32, "weak"), (64, "strong")];
const GRAIN_SIZE: &[(i64, &str)] = &[(16, "small"), (32, "large")];
const DYNAMIC_RANGE: &[(i64, &str)] = &[
    (100, "hdr100"),
    (200, "hdr200"),
    (400, "hdr400"),
    (800, "hdr800"),
];
const PRIORITY_FIXED: &[(i64, &str)] = &[(1, "weak"), (2, "strong")];

#[derive(Default)]
struct Settings(Map<String, Value>);

impl Settings {
    /// Sets `key` from a looked-up value, or logs why it stays unset.
    fn set<V: Into<Value>>(&mut self, key: &str, tag: T, raw: Option<i64>, value: Option<V>) {
        match (raw, value) {
            (_, Some(v)) => {
                self.0.insert(key.to_owned(), v.into());
            }
            (Some(raw), None) => warn!("unrecognised {tag:?} value {raw:#x}, leaving {key} unset"),
            (None, None) => {}
        }
    }
}

/// Tones are stored as -16 per step (half steps allowed).
fn tone(raw: i64) -> Option<f64> {
    i32::try_from(-raw).ok().map(|v| f64::from(v) / 16.0)
}

pub fn simulation_from_fields(notes: &MakerNotes) -> Result<SimulationBase, ImageError> {
    let mut s = Settings::default();

    let saturation = notes.int(T::Saturation);
    if let Some(film_mode) = notes.int(T::FilmMode) {
        s.set(
            "filmSimulation",
            T::FilmMode,
            Some(film_mode),
            find(FILM_MODE, film_mode),
        );
        s.set(
            "color",
            T::Saturation,
            saturation,
            saturation.and_then(|v| find(COLOR, v)),
        );
    } else {
        s.set(
            "filmSimulation",
            T::Saturation,
            saturation,
            saturation.and_then(|v| find(MONOCHROME, v)),
        );
        s.set(
            "monochromaticColorTemperature",
            T::MonochromaticColorTemperature,
            None,
            notes.int8(T::MonochromaticColorTemperature),
        );
        s.set(
            "monochromaticColorTint",
            T::MonochromaticColorTint,
            None,
            notes.int8(T::MonochromaticColorTint),
        );
    }

    let raw = notes.int(T::Sharpness);
    s.set(
        "sharpness",
        T::Sharpness,
        raw,
        raw.and_then(|v| find(SHARPNESS, v)),
    );
    let raw = notes.int(T::NoiseReduction2);
    s.set(
        "noiseReduction",
        T::NoiseReduction2,
        raw,
        raw.and_then(|v| find(NOISE_REDUCTION, v)),
    );
    s.set(
        "clarity",
        T::Clarity,
        None,
        notes.int(T::Clarity).map(|v| v / 1000),
    );
    s.set(
        "highlightTone",
        T::Highlight,
        None,
        notes.int(T::Highlight).and_then(tone),
    );
    s.set(
        "shadowTone",
        T::Shadow,
        None,
        notes.int(T::Shadow).and_then(tone),
    );

    let raw = notes.int(T::WhiteBalance);
    let white_balance = raw.and_then(|v| find(WHITE_BALANCE, v));
    s.set("whiteBalance", T::WhiteBalance, raw, white_balance);
    if white_balance == Some("temperature") {
        s.set(
            "whiteBalanceTemperature",
            T::ColorTemperature,
            None,
            notes.int(T::ColorTemperature),
        );
    }
    if let Some([red, blue]) = notes.ints(T::WhiteBalanceFineTune).as_deref() {
        s.set(
            "whiteBalanceShiftRed",
            T::WhiteBalanceFineTune,
            None,
            Some(red / 20),
        );
        s.set(
            "whiteBalanceShiftBlue",
            T::WhiteBalanceFineTune,
            None,
            Some(blue / 20),
        );
    }

    for (key, tag) in [
        ("colorChromeEffect", T::ColorChromeEffect),
        ("colorChromeFxBlue", T::ColorChromeFXBlue),
        ("smoothSkinEffect", T::SmoothSkinEffect),
    ] {
        let raw = notes.int(tag);
        s.set(key, tag, raw, raw.and_then(|v| find(LEVEL, v)));
    }

    let roughness = notes.int(T::GrainEffectRoughness);
    let grain = match (roughness, notes.int(T::GrainEffectSize)) {
        (Some(0), _) => Some("off".to_owned()),
        (Some(r), Some(size)) => find(LEVEL, r)
            .zip(find(GRAIN_SIZE, size))
            .map(|(r, size)| format!("{r}_{size}")),
        _ => None,
    };
    s.set("grainEffect", T::GrainEffectRoughness, roughness, grain);

    let lmo = notes.int(T::LensModulationOptimizer);
    s.set(
        "lensModulationOptimizer",
        T::LensModulationOptimizer,
        None,
        lmo.map(|v| if v == 0 { "off" } else { "on" }),
    );

    let raw = notes.int(T::DevelopmentDynamicRange);
    let dynamic_range = if notes.int(T::DynamicRangeSetting) == Some(0) {
        Some("auto")
    } else {
        raw.and_then(|v| find(DYNAMIC_RANGE, v))
    };
    s.set(
        "dynamicRange",
        T::DevelopmentDynamicRange,
        raw,
        dynamic_range,
    );
    // The priority tags are only written while D-Range Priority is on.
    let raw = notes.int(T::DRangePriority);
    let priority = match raw {
        Some(0) => Some("auto"),
        Some(1) => notes
            .int(T::DRangePriorityFixed)
            .and_then(|v| find(PRIORITY_FIXED, v)),
        _ => None,
    };
    s.set("dynamicRangePriority", T::DRangePriority, raw, priority);

    debug!("{:?}", s.0);
    Ok(serde_json::from_value(Value::Object(s.0))?)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn notes(entries: &[(T, IFDType, Vec<u8>)]) -> HashMap<T, (IFDType, Vec<u8>)> {
        entries
            .iter()
            .map(|(t, ty, b)| (*t, (*ty, b.clone())))
            .collect()
    }

    fn extract(entries: &[(T, IFDType, Vec<u8>)]) -> Value {
        let owned = notes(entries);
        let borrowed = MakerNotes(
            owned
                .iter()
                .map(|(t, (ty, b))| (*t, (*ty, b.as_slice())))
                .collect(),
        );
        let base = simulation_from_fields(&borrowed).expect("valid maker notes");
        serde_json::to_value(base).expect("serializable")
    }

    fn short(v: u16) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    fn long(v: i32) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    #[test]
    fn color_recipe() {
        // Classic Chrome, color +3, sharpness -1, NR -4, H -2, S +0.5, clarity 2,
        // daylight R+2 B-6, grain weak small, CCE strong, CCB weak, DR400
        let v = extract(&[
            (T::FilmMode, IFDType::Short, short(0x600)),
            (T::Saturation, IFDType::Short, short(0x0c0)),
            (T::Sharpness, IFDType::Short, short(0x82)),
            (T::NoiseReduction2, IFDType::Short, short(0x2e0)),
            (T::Highlight, IFDType::SLong, long(32)),
            (T::Shadow, IFDType::SLong, long(-8)),
            (T::Clarity, IFDType::SLong, long(2000)),
            (T::WhiteBalance, IFDType::Short, short(0x100)),
            (
                T::WhiteBalanceFineTune,
                IFDType::SLong,
                [long(40), long(-120)].concat(),
            ),
            (T::GrainEffectRoughness, IFDType::Long, long(32)),
            (T::GrainEffectSize, IFDType::Long, long(16)),
            (T::ColorChromeEffect, IFDType::Long, long(64)),
            (T::ColorChromeFXBlue, IFDType::Long, long(32)),
            (T::DynamicRangeSetting, IFDType::Short, short(1)),
            (T::DevelopmentDynamicRange, IFDType::Short, short(400)),
        ]);
        assert_eq!(
            v,
            json!({
                "filmSimulation": "classic_chrome", "color": 3, "sharpness": -1, "noiseReduction": -4,
                "highlightTone": -2.0, "shadowTone": 0.5, "clarity": 2, "whiteBalance": "daylight",
                "whiteBalanceShiftRed": 2, "whiteBalanceShiftBlue": -6, "grainEffect": "weak_small",
                "colorChromeEffect": "strong", "colorChromeFxBlue": "weak", "dynamicRange": "hdr400",
            })
        );
    }

    #[test]
    fn monochrome_with_kelvin() {
        // Acros Ye, WC +3 / MG -2 (signed bytes), Kelvin 5300, D-Range Priority fixed strong
        let v = extract(&[
            (T::Saturation, IFDType::Short, short(0x502)),
            (T::MonochromaticColorTemperature, IFDType::Byte, vec![3]),
            (T::MonochromaticColorTint, IFDType::Byte, vec![0xfe]),
            (T::WhiteBalance, IFDType::Short, short(0xff0)),
            (T::ColorTemperature, IFDType::Short, short(5300)),
            (T::DRangePriority, IFDType::Short, short(1)),
            (T::DRangePriorityFixed, IFDType::Short, short(2)),
        ]);
        assert_eq!(
            v,
            json!({
                "filmSimulation": "acros_ye", "monochromaticColorTemperature": 3, "monochromaticColorTint": -2,
                "whiteBalance": "temperature", "whiteBalanceTemperature": 5300, "dynamicRangePriority": "strong",
            })
        );
    }

    #[test]
    fn unknown_values_are_left_unset() {
        let v = extract(&[
            (T::FilmMode, IFDType::Short, short(0x123)),
            (T::Sharpness, IFDType::Short, short(0x99)),
        ]);
        assert_eq!(v, json!({}));
    }
}
