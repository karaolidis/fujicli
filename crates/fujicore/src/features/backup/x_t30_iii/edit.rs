//! Recipe values -> backup bytes, with the checksum kept valid.

use std::collections::BTreeMap;

use serde_json::Value;

use super::{
    BackupFileError,
    decode::{at, decode_fs, decode_slot, fs_shift_row, slot_base},
    tables::{
        CHECKSUM, COLOR, DR, DR_PRIORITY, FILM, FS_CCB, FS_CCE, FS_CLARITY, FS_COLOR, FS_DR,
        FS_DR_PRIORITY, FS_FILM, FS_GRAIN_SIZE, FS_GRAIN_STRENGTH, FS_HIGHLIGHT, FS_KELVIN, FS_MG,
        FS_NOISE_REDUCTION, FS_PORTRAIT_ENHANCER, FS_RECIPE_ON, FS_RECIPE_SEEN, FS_SHADOW,
        FS_SHARPNESS, FS_WB, FS_WB_SHIFT, FS_WB_SHIFT_ROW, FS_WC, GRAIN, LEVEL, PORTRAIT_ENHANCER,
        S_CCB, S_CCE, S_CLARITY, S_COLOR, S_DR, S_DR_PRIORITY, S_FILM, S_GRAIN_SIZE,
        S_GRAIN_STRENGTH, S_HIGHLIGHT, S_KELVIN, S_MG, S_NOISE_REDUCTION, S_PORTRAIT_ENHANCER,
        S_SHADOW, S_SHARPNESS, S_WB, S_WB_SHIFT, S_WC, WB_MODES, WB_SHIFT_ORDER, code_of, name_of,
    },
};

type Changes = BTreeMap<usize, u8>;

/// Writes `changes` and moves the checksum by the sum of the byte deltas (it is a u16 byte sum).
pub fn apply(data: &[u8], changes: &Changes) -> Vec<u8> {
    let mut out = data.to_vec();
    let mut delta: i64 = 0;
    for (&pos, &value) in changes {
        debug_assert!(!(CHECKSUM..CHECKSUM + 2).contains(&pos));
        delta += i64::from(value) - i64::from(out[pos]);
        out[pos] = value;
    }
    let sum = i64::from(u16::from_le_bytes([out[CHECKSUM], out[CHECKSUM + 1]]));
    let sum = u16::try_from((sum + delta).rem_euclid(0x1_0000)).unwrap_or_default();
    out[CHECKSUM..CHECKSUM + 2].copy_from_slice(&sum.to_le_bytes());
    out
}

fn invalid(key: &str, value: &str) -> BackupFileError {
    BackupFileError::InvalidValue {
        key: key.to_owned(),
        value: value.to_owned(),
    }
}

fn int_in(key: &str, value: &str, lo: i64, hi: i64) -> Result<i64, BackupFileError> {
    value
        .parse::<i64>()
        .ok()
        .filter(|v| (lo..=hi).contains(v))
        .ok_or_else(|| invalid(key, value))
}

fn byte(key: &str, value: &str, v: i64) -> Result<u8, BackupFileError> {
    u8::try_from(v).map_err(|_| invalid(key, value))
}

/// Tones: -2..=4 in half steps, stored as `2 * (tone + 2)`.
fn tone(key: &str, value: &str) -> Result<u8, BackupFileError> {
    let v: f64 = value.parse().map_err(|_| invalid(key, value))?;
    let raw = (v + 2.0) * 2.0;
    if !(0.0..=12.0).contains(&raw) || raw.fract() != 0.0 {
        return Err(invalid(key, value));
    }
    #[allow(clippy::cast_possible_truncation)] // checked above: a whole number in 0..=12
    byte(key, value, raw as i64)
}

fn named(table: &[(u8, &str)], key: &str, value: &str) -> Result<u8, BackupFileError> {
    code_of(table, value).ok_or_else(|| invalid(key, value))
}

/// `sets` uses the keys and value spellings printed by the decoder.
fn fs_changes(
    data: &[u8],
    n: usize,
    sets: &[(String, String)],
) -> Result<Changes, BackupFileError> {
    let mut ch = Changes::new();
    let mut shifts = Vec::new();
    for (key, value) in sets {
        let (k, v) = (key.as_str(), value.as_str());
        let cell = |table: usize| table + n;
        match k {
            "filmSimulation" => {
                let code = named(FILM, k, v)?;
                let film = FS_FILM + 3 * (n - 1);
                ch.insert(film, code);
                // the next two bytes remember the last Acros / Monochrome filter, as in the slots
                if v.starts_with("acros") {
                    ch.insert(film + 1, code);
                } else if v.starts_with("monochrome") {
                    ch.insert(film + 2, code);
                }
            }
            "recipe" => {
                let on = match v {
                    "on" => 1,
                    "off" => 0,
                    _ => return Err(invalid(k, v)),
                };
                ch.insert(FS_RECIPE_ON + n - 1, on);
                if on == 1 {
                    ch.insert(FS_RECIPE_SEEN + n - 1, 1);
                }
            }
            "grainEffect" => {
                let (strength, size) = code_of(GRAIN, v).ok_or_else(|| invalid(k, v))?;
                ch.insert(cell(FS_GRAIN_STRENGTH), strength);
                ch.insert(cell(FS_GRAIN_SIZE), size);
            }
            "colorChromeEffect" => _ = ch.insert(cell(FS_CCE), named(LEVEL, k, v)?),
            "colorChromeFxBlue" => _ = ch.insert(cell(FS_CCB), named(LEVEL, k, v)?),
            "whiteBalance" => _ = ch.insert(cell(FS_WB), named(WB_MODES, k, v)?),
            "dynamicRange" => _ = ch.insert(cell(FS_DR), named(DR, k, v)?),
            "dynamicRangePriority" => {
                ch.insert(cell(FS_DR_PRIORITY), named(DR_PRIORITY, k, v)?);
            }
            "portraitEnhancer" => {
                ch.insert(cell(FS_PORTRAIT_ENHANCER), named(PORTRAIT_ENHANCER, k, v)?);
            }
            "highlightTone" => _ = ch.insert(cell(FS_HIGHLIGHT), tone(k, v)?),
            "shadowTone" => _ = ch.insert(cell(FS_SHADOW), tone(k, v)?),
            "color" => {
                let c = int_in(k, v, -4, 4)?;
                ch.insert(
                    cell(FS_COLOR),
                    code_of(COLOR, c).ok_or_else(|| invalid(k, v))?,
                );
            }
            "sharpness" => _ = ch.insert(cell(FS_SHARPNESS), byte(k, v, 4 - int_in(k, v, -4, 4)?)?),
            "noiseReduction" => {
                ch.insert(
                    cell(FS_NOISE_REDUCTION),
                    byte(k, v, int_in(k, v, -4, 4)? + 4)?,
                );
            }
            "clarity" => _ = ch.insert(cell(FS_CLARITY), byte(k, v, int_in(k, v, -5, 5)? + 6)?),
            "monochromaticColorTemperature" => {
                ch.insert(cell(FS_WC), byte(k, v, 18 - int_in(k, v, -18, 18)?)?);
            }
            "monochromaticColorTint" => {
                ch.insert(cell(FS_MG), byte(k, v, 18 - int_in(k, v, -18, 18)?)?);
            }
            "whiteBalanceTemperature" => {
                let kelvin = int_in(k, v, 2500, 10000).ok().filter(|t| t % 10 == 0);
                let kelvin = kelvin
                    .and_then(|t| u16::try_from(t).ok())
                    .ok_or_else(|| invalid(k, v))?;
                let [lo, hi] = kelvin.to_le_bytes();
                ch.insert(FS_KELVIN + 2 * n, lo);
                ch.insert(FS_KELVIN + 2 * n + 1, hi);
            }
            "whiteBalanceShiftRed" | "whiteBalanceShiftBlue" => {
                shifts.push((
                    k == "whiteBalanceShiftBlue",
                    byte(k, v, 9 - int_in(k, v, -9, 9)?)?,
                ));
            }
            _ => return Err(BackupFileError::UnknownKey(key.clone())),
        }
    }
    // shifts are stored per WB mode: use the mode this edit leaves in place
    let wb_code = ch.get(&(FS_WB + n)).copied().unwrap_or(data[FS_WB + n]);
    let row = name_of(WB_MODES, wb_code)
        .and_then(fs_shift_row)
        .ok_or(BackupFileError::Corrupt)?;
    for (blue, raw) in shifts {
        ch.insert(row + if blue { 9 } else { 0 } + n, raw);
    }
    Ok(ch)
}

/// The camera copies the active slot when an FS recipe is switched on; this copies slot `slot`.
fn from_slot_changes(data: &[u8], n: usize, slot: usize) -> Changes {
    let base = slot_base(slot);
    let pairs = [
        (S_PORTRAIT_ENHANCER, FS_PORTRAIT_ENHANCER),
        (S_NOISE_REDUCTION, FS_NOISE_REDUCTION),
        (S_CLARITY, FS_CLARITY),
        (S_DR, FS_DR),
        (S_DR_PRIORITY, FS_DR_PRIORITY),
        (S_COLOR, FS_COLOR),
        (S_SHARPNESS, FS_SHARPNESS),
        (S_HIGHLIGHT, FS_HIGHLIGHT),
        (S_SHADOW, FS_SHADOW),
        (S_CCE, FS_CCE),
        (S_CCB, FS_CCB),
        (S_GRAIN_STRENGTH, FS_GRAIN_STRENGTH),
        (S_GRAIN_SIZE, FS_GRAIN_SIZE),
        (S_WC, FS_WC),
        (S_MG, FS_MG),
        (S_WB, FS_WB),
    ];
    let mut ch: Changes = pairs
        .iter()
        .map(|&(s, f)| (f + n, at(data, base, s)))
        .collect();
    for i in 0..WB_SHIFT_ORDER.len() {
        let entry = S_WB_SHIFT + 4 * isize::try_from(i).unwrap_or_default();
        let row = FS_WB_SHIFT + i * FS_WB_SHIFT_ROW;
        ch.insert(row + n, at(data, base, entry));
        ch.insert(row + 9 + n, at(data, base, entry + 2));
    }
    ch.insert(FS_KELVIN + 2 * n, at(data, base, S_KELVIN));
    ch.insert(FS_KELVIN + 2 * n + 1, at(data, base, S_KELVIN + 1));
    for i in 0..3 {
        ch.insert(
            FS_FILM + 3 * (n - 1) + i,
            at(
                data,
                base,
                S_FILM + 3 * isize::try_from(i).unwrap_or_default(),
            ),
        );
    }
    ch.insert(FS_RECIPE_ON + n - 1, 1);
    ch.insert(FS_RECIPE_SEEN + n - 1, 1);
    ch
}

fn same(got: Option<&Value>, want: &str) -> bool {
    match got {
        Some(Value::String(s)) => s == want,
        Some(v) => v
            .as_f64()
            .is_some_and(|g| want.parse::<f64>().is_ok_and(|w| (g - w).abs() < 1e-9)),
        None => false,
    }
}

/// Every requested value must read back as asked, or nothing is returned.
fn check_read_back(
    got: &serde_json::Map<String, Value>,
    wanted: &[(String, String)],
) -> Result<(), BackupFileError> {
    for (key, value) in wanted {
        if !same(got.get(key), value) {
            return Err(BackupFileError::ReadBack {
                key: key.clone(),
                wanted: value.clone(),
                got: got
                    .get(key)
                    .map_or_else(|| "nothing".to_owned(), ToString::to_string),
            });
        }
    }
    Ok(())
}

pub fn edit_fs(
    data: &[u8],
    n: usize,
    from_slot: Option<usize>,
    sets: &[(String, String)],
) -> Result<Vec<u8>, BackupFileError> {
    let mut wanted = Vec::new();
    let mut out = data.to_vec();
    if let Some(slot) = from_slot {
        out = apply(&out, &from_slot_changes(&out, n, slot));
        let fs = decode_fs(&out, n);
        for (k, v) in decode_slot(&out, slot) {
            if fs.contains_key(&k) {
                wanted.push((k, v.as_str().map_or_else(|| v.to_string(), str::to_owned)));
            }
        }
    }
    out = apply(&out, &fs_changes(&out, n, sets)?);
    wanted.retain(|(k, _)| !sets.iter().any(|(s, _)| s == k));
    wanted.extend_from_slice(sets);
    check_read_back(&decode_fs(&out, n), &wanted)?;
    Ok(out)
}

#[derive(Debug, Default, Clone)]
pub struct SlotEdit {
    pub monochromatic_color_temperature: Option<i64>,
    pub monochromatic_color_tint: Option<i64>,
    pub portrait_enhancer: Option<String>,
}

/// What PTP can't write to a custom slot: Monochromatic Color and Portrait Enhancer.
pub fn edit_slot(data: &[u8], slot: usize, edit: &SlotEdit) -> Result<Vec<u8>, BackupFileError> {
    let base = slot_base(slot);
    let pos = |off: isize| base.wrapping_add_signed(off);
    let mut ch = Changes::new();
    for (key, value, off) in [
        (
            "monochromaticColorTemperature",
            edit.monochromatic_color_temperature,
            S_WC,
        ),
        (
            "monochromaticColorTint",
            edit.monochromatic_color_tint,
            S_MG,
        ),
    ] {
        if let Some(v) = value {
            let text = v.to_string();
            ch.insert(
                pos(off),
                byte(key, &text, 18 - int_in(key, &text, -18, 18)?)?,
            );
        }
    }
    if let Some(pe) = &edit.portrait_enhancer {
        ch.insert(
            pos(S_PORTRAIT_ENHANCER),
            named(PORTRAIT_ENHANCER, "portraitEnhancer", pe)?,
        );
    }
    Ok(apply(data, &ch))
}
