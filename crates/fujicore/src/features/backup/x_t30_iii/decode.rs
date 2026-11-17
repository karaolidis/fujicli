//! Backup bytes -> recipes (same keys as `simulation export`, plus what PTP can't reach).

use serde_json::{Map, Value, json};

use super::tables::{
    COLOR, DR, DR_PRIORITY, FILM, FS_CCB, FS_CCE, FS_CLARITY, FS_COLOR, FS_DR, FS_DR_PRIORITY,
    FS_FILM, FS_GRAIN_SIZE, FS_GRAIN_STRENGTH, FS_HIGHLIGHT, FS_KELVIN, FS_MG, FS_NOISE_REDUCTION,
    FS_PORTRAIT_ENHANCER, FS_RECIPE_ON, FS_SHADOW, FS_SHARPNESS, FS_WB, FS_WB_SHIFT,
    FS_WB_SHIFT_ROW, FS_WC, GRAIN, ISO_SPECIAL, ISO_THIRDS, LEVEL, MIN_SHUTTER, MOUNT_FOCAL_MM,
    PORTRAIT_ENHANCER, QUALITY, RAW_RECORDING, S_ASPECT, S_CCB, S_CCE, S_CLARITY, S_COLOR,
    S_COLOR_SPACE, S_DR, S_DR_PRIORITY, S_FILE_FORMAT, S_FILM, S_GRAIN_SIZE, S_GRAIN_STRENGTH,
    S_HIGHLIGHT, S_ISO, S_ISO_AUTO_MAX, S_ISO_AUTO_MIN, S_ISO_AUTO_SHUTTER, S_KELVIN, S_LMO,
    S_LONG_EXPOSURE_NR, S_MG, S_MOUNT_FOCAL, S_NAME, S_NAME_LEN, S_NOISE_REDUCTION,
    S_PORTRAIT_ENHANCER, S_QUALITY, S_RAW_RECORDING, S_SHADOW, S_SHARPNESS, S_SIZE, S_WB,
    S_WB_SHIFT, S_WC, SIZES, SLOT0, STRIDE, WB_MODES, WB_SHIFT_ORDER, is_monochrome, name_of,
};

/// A byte that matches no known code is reported as `"?<code>"` instead of being guessed.
fn named<K: PartialEq + Copy + std::fmt::Display>(table: &[(K, &str)], key: K) -> Value {
    name_of(table, key).map_or_else(|| json!(format!("?{key}")), Value::from)
}

fn named_pair(table: &[((u8, u8), &str)], key: (u8, u8)) -> Value {
    name_of(table, key).map_or_else(|| json!(format!("?{},{}", key.0, key.1)), Value::from)
}

fn numbered(table: &[(u8, i64)], key: u8) -> Value {
    name_of(table, key).map_or_else(|| json!(format!("?{key}")), Value::from)
}

/// Tones are stored as `2 * (tone + 2)`, so half steps are integers.
fn tone(raw: u8) -> Value {
    json!(f64::from(raw) / 2.0 - 2.0)
}

fn shift(raw: u8) -> Value {
    json!(9 - i64::from(raw))
}

fn wc_mg(raw: u8) -> Value {
    json!(18 - i64::from(raw))
}

pub const fn slot_base(slot: usize) -> usize {
    SLOT0 + (slot - 1) * STRIDE
}

pub const fn at(d: &[u8], base: usize, off: isize) -> u8 {
    d[base.wrapping_add_signed(off)]
}

const fn u16_at(d: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes([d[pos], d[pos + 1]])
}

fn wb_shift_index(wb: &str) -> Option<usize> {
    WB_SHIFT_ORDER.iter().position(|m| *m == wb)
}

fn iso_selected(code: u8) -> Value {
    if let Some(s) = name_of(ISO_SPECIAL, code) {
        return s.parse::<i64>().map_or_else(|_| json!(s), Value::from);
    }
    match usize::from(code).checked_sub(11) {
        Some(i) if (15..=33).contains(&code) => json!(ISO_THIRDS[i]),
        _ => json!(format!("?{code}")),
    }
}

fn iso_auto(d: &[u8], base: usize) -> Value {
    let preset = |i: isize| {
        let min = 22usize.checked_sub(usize::from(at(d, base, S_ISO_AUTO_MIN + i)));
        // full stops from 12800 = 0: 6400 = 1, 3200 = 2, ...
        let max = 12800i64.checked_shr(u32::from(at(d, base, S_ISO_AUTO_MAX + i)));
        json!({
            "min": min.and_then(|i| ISO_THIRDS.get(i)).map_or(Value::Null, |v| json!(v)),
            "max": max,
            "minShutter": named(MIN_SHUTTER, at(d, base, S_ISO_AUTO_SHUTTER + i)),
        })
    };
    json!([preset(0), preset(1), preset(2)])
}

pub fn decode_slot(d: &[u8], slot: usize) -> Map<String, Value> {
    let base = slot_base(slot);
    let b = |off| at(d, base, off);
    let name_start = base.wrapping_add_signed(S_NAME);
    let name = &d[name_start..name_start + S_NAME_LEN];
    let name = name.split(|c| *c == 0).next().unwrap_or_default();

    let film = name_of(FILM, b(S_FILM)).unwrap_or("?");
    let wb = name_of(WB_MODES, b(S_WB)).unwrap_or("?");
    let mut r = Map::new();
    r.insert(
        "customSettingName".into(),
        json!(String::from_utf8_lossy(name)),
    );
    r.insert(
        "imageSize".into(),
        named_pair(SIZES, (b(S_SIZE), b(S_ASPECT))),
    );
    r.insert("imageQuality".into(), named(QUALITY, b(S_QUALITY)));
    r.insert("filmSimulation".into(), named(FILM, b(S_FILM)));
    r.insert(
        "grainEffect".into(),
        named_pair(GRAIN, (b(S_GRAIN_STRENGTH), b(S_GRAIN_SIZE))),
    );
    r.insert("colorChromeEffect".into(), named(LEVEL, b(S_CCE)));
    r.insert("colorChromeFxBlue".into(), named(LEVEL, b(S_CCB)));
    r.insert("whiteBalance".into(), named(WB_MODES, b(S_WB)));
    if let Some(i) = wb_shift_index(wb) {
        let entry = S_WB_SHIFT + 4 * isize::try_from(i).unwrap_or_default();
        r.insert("whiteBalanceShiftRed".into(), shift(b(entry)));
        r.insert("whiteBalanceShiftBlue".into(), shift(b(entry + 2)));
    }
    if wb == "temperature" {
        r.insert(
            "whiteBalanceTemperature".into(),
            json!(u16_at(d, base.wrapping_add_signed(S_KELVIN))),
        );
    }
    r.insert("highlightTone".into(), tone(b(S_HIGHLIGHT)));
    r.insert("shadowTone".into(), tone(b(S_SHADOW)));
    if is_monochrome(film) {
        r.insert("monochromaticColorTemperature".into(), wc_mg(b(S_WC)));
        r.insert("monochromaticColorTint".into(), wc_mg(b(S_MG)));
    } else {
        r.insert("color".into(), numbered(COLOR, b(S_COLOR)));
    }
    r.insert("sharpness".into(), json!(4 - i64::from(b(S_SHARPNESS))));
    r.insert(
        "noiseReduction".into(),
        json!(i64::from(b(S_NOISE_REDUCTION)) - 4),
    );
    r.insert("clarity".into(), json!(i64::from(b(S_CLARITY)) - 6));
    r.insert(
        "lensModulationOptimizer".into(),
        json!(if b(S_LMO) == 0 { "on" } else { "off" }),
    );
    r.insert(
        "colorSpace".into(),
        json!(if b(S_COLOR_SPACE) == 0 {
            "srgb"
        } else {
            "adobe_rgb"
        }),
    );
    r.insert("dynamicRange".into(), named(DR, b(S_DR)));
    r.insert(
        "dynamicRangePriority".into(),
        named(DR_PRIORITY, b(S_DR_PRIORITY)),
    );
    r.insert(
        "portraitEnhancer".into(),
        named(PORTRAIT_ENHANCER, b(S_PORTRAIT_ENHANCER)),
    );
    r.insert(
        "longExposureNr".into(),
        json!(if b(S_LONG_EXPOSURE_NR) == 0 {
            "on"
        } else {
            "off"
        }),
    );
    r.insert(
        "fileFormat".into(),
        json!(if b(S_FILE_FORMAT) == 0 { "jpg" } else { "heif" }),
    );
    r.insert(
        "rawRecording".into(),
        named(RAW_RECORDING, b(S_RAW_RECORDING)),
    );
    r.insert(
        "mountAdaptorFocal".into(),
        numbered(MOUNT_FOCAL_MM, b(S_MOUNT_FOCAL)),
    );
    r.insert("iso".into(), iso_selected(b(S_ISO)));
    r.insert("isoAuto".into(), iso_auto(d, base));
    r
}

pub fn fs_shift_row(wb: &str) -> Option<usize> {
    wb_shift_index(wb).map(|i| FS_WB_SHIFT + i * FS_WB_SHIFT_ROW)
}

pub fn decode_fs(d: &[u8], n: usize) -> Map<String, Value> {
    let film = name_of(FILM, d[FS_FILM + 3 * (n - 1)]).unwrap_or("?");
    let wb = name_of(WB_MODES, d[FS_WB + n]).unwrap_or("?");
    let mut r = Map::new();
    r.insert("dial".into(), json!(format!("FS{n}")));
    r.insert(
        "filmSimulation".into(),
        named(FILM, d[FS_FILM + 3 * (n - 1)]),
    );
    // with the recipe off, only the film simulation applies
    r.insert(
        "recipe".into(),
        json!(if d[FS_RECIPE_ON + n - 1] == 0 {
            "off"
        } else {
            "on"
        }),
    );
    r.insert(
        "grainEffect".into(),
        named_pair(GRAIN, (d[FS_GRAIN_STRENGTH + n], d[FS_GRAIN_SIZE + n])),
    );
    r.insert("colorChromeEffect".into(), named(LEVEL, d[FS_CCE + n]));
    r.insert("colorChromeFxBlue".into(), named(LEVEL, d[FS_CCB + n]));
    r.insert("whiteBalance".into(), named(WB_MODES, d[FS_WB + n]));
    if let Some(row) = fs_shift_row(wb) {
        r.insert("whiteBalanceShiftRed".into(), shift(d[row + n]));
        r.insert("whiteBalanceShiftBlue".into(), shift(d[row + 9 + n]));
    }
    if wb == "temperature" {
        r.insert(
            "whiteBalanceTemperature".into(),
            json!(u16_at(d, FS_KELVIN + 2 * n)),
        );
    }
    r.insert("highlightTone".into(), tone(d[FS_HIGHLIGHT + n]));
    r.insert("shadowTone".into(), tone(d[FS_SHADOW + n]));
    if is_monochrome(film) {
        r.insert("monochromaticColorTemperature".into(), wc_mg(d[FS_WC + n]));
        r.insert("monochromaticColorTint".into(), wc_mg(d[FS_MG + n]));
    } else {
        r.insert("color".into(), numbered(COLOR, d[FS_COLOR + n]));
    }
    r.insert(
        "sharpness".into(),
        json!(4 - i64::from(d[FS_SHARPNESS + n])),
    );
    r.insert(
        "noiseReduction".into(),
        json!(i64::from(d[FS_NOISE_REDUCTION + n]) - 4),
    );
    r.insert("clarity".into(), json!(i64::from(d[FS_CLARITY + n]) - 6));
    r.insert("dynamicRange".into(), named(DR, d[FS_DR + n]));
    r.insert(
        "dynamicRangePriority".into(),
        named(DR_PRIORITY, d[FS_DR_PRIORITY + n]),
    );
    r.insert(
        "portraitEnhancer".into(),
        named(PORTRAIT_ENHANCER, d[FS_PORTRAIT_ENHANCER + n]),
    );
    r
}
