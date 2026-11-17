use serde_json::{Value, json};

use super::{
    BackupFileError, SlotEdit,
    edit::apply,
    edit_fs, edit_slot, recipes,
    tables::{CHECKSUM, MAGIC, MODEL, SIZE, SLOT0, STRIDE},
};

/// A synthetic backup: valid header, default codes where zero would not decode.
fn blank() -> Vec<u8> {
    let mut d = vec![0u8; SIZE];
    d[..MAGIC.len()].copy_from_slice(MAGIC);
    d[MAGIC.len()..MAGIC.len() + MODEL.len()].copy_from_slice(MODEL);
    for slot in 0..7 {
        let base = SLOT0 + slot * STRIDE;
        d[base + 0x55] = 15; // classic chrome
        d[base + 0x73] = 2; // grain off
        d[base + 0x8D] = 1; // AUTO1
    }
    d[0x888] = 25; // FS1 acros g
    d[0x88B] = 17;
    d[0x88E] = 13;
    d
}

fn checksum(d: &[u8]) -> u16 {
    u16::from_le_bytes([d[CHECKSUM], d[CHECKSUM + 1]])
}

fn sets(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn fs(d: &[u8], n: usize) -> Value {
    recipes(d).expect("valid backup")["filmSimulationDial"][n - 1].clone()
}

#[test]
fn checksum_moves_by_the_byte_deltas_and_wraps() {
    let mut d = blank();
    d[CHECKSUM] = 0xFF;
    d[CHECKSUM + 1] = 0xFF;
    let out = apply(&d, &[(0x500, 3), (0x501, 0)].into_iter().collect());
    assert_eq!(checksum(&out), 2);
    let back = apply(&out, &std::iter::once((0x500, 0)).collect());
    assert_eq!(checksum(&back), 0xFFFF);
}

#[test]
fn rejects_other_files() {
    let mut d = blank();
    d[0x14] = b'Y';
    assert!(matches!(recipes(&d), Err(BackupFileError::NotXT30III)));
    assert!(matches!(
        recipes(&blank()[..100]),
        Err(BackupFileError::NotXT30III)
    ));
}

#[test]
fn slot_monochromatic_color_and_portrait_enhancer() {
    let edit = SlotEdit {
        monochromatic_color_temperature: Some(3),
        monochromatic_color_tint: Some(-2),
        portrait_enhancer: Some("medium".into()),
    };
    let mut d = blank();
    d[SLOT0 + 0x55] = 22; // acros, so WC/MG are reported
    let out = edit_slot(&d, 1, &edit).expect("valid edit");
    let c1 = &recipes(&out).expect("valid backup")["custom"][0];
    assert_eq!(c1["monochromaticColorTemperature"], json!(3));
    assert_eq!(c1["monochromaticColorTint"], json!(-2));
    assert_eq!(c1["portraitEnhancer"], json!("medium"));
    assert!(edit_slot(&d, 8, &edit).is_err());
}

#[test]
fn fs_recipe_round_trip() {
    let out = edit_fs(
        &blank(),
        2,
        None,
        &sets(&[
            ("recipe", "on"),
            ("filmSimulation", "acros_ye"),
            ("whiteBalance", "temperature"),
            ("whiteBalanceTemperature", "4200"),
            ("whiteBalanceShiftRed", "-3"),
            ("whiteBalanceShiftBlue", "4"),
            ("highlightTone", "1.5"),
            ("clarity", "-3"),
            ("monochromaticColorTemperature", "-5"),
            ("grainEffect", "weak_small"),
        ]),
    )
    .expect("valid edit");
    let fs2 = fs(&out, 2);
    assert_eq!(fs2["recipe"], json!("on"));
    assert_eq!(fs2["filmSimulation"], json!("acros_ye"));
    assert_eq!(fs2["whiteBalanceTemperature"], json!(4200));
    assert_eq!(fs2["whiteBalanceShiftRed"], json!(-3));
    assert_eq!(fs2["whiteBalanceShiftBlue"], json!(4));
    assert_eq!(fs2["highlightTone"], json!(1.5));
    assert_eq!(fs2["clarity"], json!(-3));
    assert_eq!(fs2["monochromaticColorTemperature"], json!(-5));
    assert_eq!(fs2["grainEffect"], json!("weak_small"));
    assert_eq!(fs(&out, 1), fs(&blank(), 1), "other positions untouched");
}

#[test]
fn fs_from_slot_copies_the_recipe() {
    let mut d = blank();
    let c3 = SLOT0 + 2 * STRIDE;
    d[c3 + 0x55] = 19; // eterna
    d[c3 + 0x16] = 8; // clarity +2
    d[c3 - 0x29] = 7; // fluorescent3
    d[c3 - 0x27 + 4 * 11] = 15; // its red shift: -6
    let out = edit_fs(&d, 1, Some(3), &[]).expect("valid edit");
    let fs1 = fs(&out, 1);
    assert_eq!(fs1["filmSimulation"], json!("eterna"));
    assert_eq!(fs1["clarity"], json!(2));
    assert_eq!(fs1["whiteBalance"], json!("fluorescent3"));
    assert_eq!(fs1["whiteBalanceShiftRed"], json!(-6));
    assert_eq!(fs1["recipe"], json!("on"));
}

#[test]
fn bad_fs_values_write_nothing() {
    let d = blank();
    for bad in [
        ("clarity", "9"),
        ("highlightTone", "1.25"),
        ("filmSimulation", "kodachrome"),
    ] {
        assert!(matches!(
            edit_fs(&d, 1, None, &sets(&[bad])),
            Err(BackupFileError::InvalidValue { .. })
        ));
    }
    assert!(matches!(
        edit_fs(&d, 1, None, &sets(&[("bogus", "1")])),
        Err(BackupFileError::UnknownKey(_))
    ));
    assert!(matches!(
        edit_fs(&d, 4, None, &[]),
        Err(BackupFileError::Fs(4))
    ));
}
