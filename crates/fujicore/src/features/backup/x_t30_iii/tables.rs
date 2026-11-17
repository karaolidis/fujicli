//! Layout of an X-T30 III backup file (firmware 1.10).
//!
//! Every offset and code here was found by writing a value from the camera (menu or PTP),
//! exporting a backup and diffing it, then confirmed by writing it back and reading the menu.

/// `FUJIFILMX-BACKUP0100` followed by the model name, NUL padded.
pub const MAGIC: &[u8] = b"FUJIFILMX-BACKUP0100";
pub const MODEL: &[u8] = b"X-T30 III\0";
pub const SIZE: usize = 0x1_137C;

/// u16 LE sum of the file bytes (minus a state area): it moves by the sum of any byte deltas.
pub const CHECKSUM: usize = 0x120;

/// Custom slot C1 starts here; C2..C7 follow every `STRIDE`. Some fields sit before the base.
pub const SLOT0: usize = 0xF800;
pub const STRIDE: usize = 0x400;
pub const SLOTS: usize = 7;

// Slot field offsets, relative to the slot base.
pub const S_NAME: isize = 0x1B8;
pub const S_NAME_LEN: usize = 0x40;
pub const S_PORTRAIT_ENHANCER: isize = 0x14;
pub const S_NOISE_REDUCTION: isize = 0x11;
pub const S_CLARITY: isize = 0x16;
pub const S_SIZE: isize = 0x17;
pub const S_ASPECT: isize = 0x19;
pub const S_QUALITY: isize = 0x1A;
pub const S_FILM: isize = 0x55; // + 0x58 / 0x5B: last Acros / Monochrome filter
pub const S_WC: isize = 0x62;
pub const S_MG: isize = 0x64;
pub const S_DR: isize = 0x66;
pub const S_DR_PRIORITY: isize = 0x68;
pub const S_COLOR: isize = 0x69;
pub const S_SHARPNESS: isize = 0x6B;
pub const S_HIGHLIGHT: isize = 0x6D;
pub const S_SHADOW: isize = 0x6F;
pub const S_CCE: isize = 0x71;
pub const S_CCB: isize = 0x72;
pub const S_GRAIN_STRENGTH: isize = 0x73;
pub const S_GRAIN_SIZE: isize = 0x74;
pub const S_ISO: isize = 0x8D;
pub const S_ISO_AUTO_MIN: isize = 0x91; // 3 bytes, AUTO1..3
pub const S_ISO_AUTO_MAX: isize = 0x94;
pub const S_ISO_AUTO_SHUTTER: isize = 0x97;
pub const S_MOUNT_FOCAL: isize = 0x9D;
pub const S_RAW_RECORDING: isize = 0xDC;
pub const S_FILE_FORMAT: isize = 0xDE;
pub const S_KELVIN: isize = -0x7C; // u16 LE
pub const S_COLOR_SPACE: isize = -0x5B;
pub const S_LONG_EXPOSURE_NR: isize = -0x5A;
pub const S_LMO: isize = -0x59;
pub const S_WB: isize = -0x29;
/// 14 entries of 4 bytes (red, _, blue, _), one per WB mode in `WB_SHIFT_ORDER`.
pub const S_WB_SHIFT: isize = -0x27;

// Film simulation dial FS1..FS3: global tables with one cell per dial position, FSn = cell n.
pub const FS_COUNT: usize = 3;
pub const FS_FILM: usize = 0x888; // 3 bytes per FS, like the slot's film bytes
pub const FS_RECIPE_ON: usize = 0xED0; // 1 byte per FS
pub const FS_RECIPE_SEEN: usize = 0xED6; // set by the camera the first time a recipe is switched on
pub const FS_KELVIN: usize = 0x4AC; // u16 LE per cell
pub const FS_WB_SHIFT: usize = 0x6F3; // 14 rows of 0x12 bytes: 9 red cells then 9 blue cells
pub const FS_WB_SHIFT_ROW: usize = 0x12;
pub const FS_WB: usize = 0x6EA;
pub const FS_NOISE_REDUCTION: usize = 0x7EF;
pub const FS_CLARITY: usize = 0x7FD;
pub const FS_DR: usize = 0x8B8;
pub const FS_DR_PRIORITY: usize = 0x8C1;
pub const FS_COLOR: usize = 0x8C5;
pub const FS_SHARPNESS: usize = 0x8CF;
pub const FS_HIGHLIGHT: usize = 0x8D8;
pub const FS_SHADOW: usize = 0x8E1;
pub const FS_CCE: usize = 0x8EA;
pub const FS_CCB: usize = 0x8EE;
pub const FS_GRAIN_STRENGTH: usize = 0x8F2;
pub const FS_GRAIN_SIZE: usize = 0x8F6;
pub const FS_WC: usize = 0x8A6;
pub const FS_MG: usize = 0x8AF;
pub const FS_PORTRAIT_ENHANCER: usize = 0x88A7;

pub const WB_MODES: &[(u8, &str)] = &[
    (0, "auto"),
    (1, "white_priority"),
    (2, "ambience_priority"),
    (3, "daylight"),
    (4, "shade"),
    (5, "fluorescent1"),
    (6, "fluorescent2"),
    (7, "fluorescent3"),
    (8, "incandescent"),
    (9, "underwater"),
    (10, "temperature"),
    (11, "custom1"),
    (12, "custom2"),
    (13, "custom3"),
];
pub const WB_SHIFT_ORDER: &[&str] = &[
    "auto",
    "white_priority",
    "ambience_priority",
    "custom1",
    "custom2",
    "custom3",
    "temperature",
    "daylight",
    "shade",
    "fluorescent1",
    "fluorescent2",
    "fluorescent3",
    "incandescent",
    "underwater",
];
pub const FILM: &[(u8, &str)] = &[
    (1, "provia"),
    (2, "astia"),
    (4, "velvia"),
    (6, "sepia"),
    (9, "monochrome"),
    (10, "monochrome_r"),
    (11, "monochrome_ye"),
    (12, "monochrome_g"),
    (13, "pro_neg_std"),
    (14, "pro_neg_hi"),
    (15, "classic_chrome"),
    (16, "classic_negative"),
    (17, "nostalgic_negative"),
    (18, "reala_ace"),
    (19, "eterna"),
    (20, "eterna_bleach_bypass"),
    (22, "acros"),
    (23, "acros_r"),
    (24, "acros_ye"),
    (25, "acros_g"),
];
/// Not linear: read off the sweep.
pub const COLOR: &[(u8, i64)] = &[
    (10, -4),
    (9, -3),
    (7, -2),
    (8, -1),
    (0, 0),
    (6, 1),
    (5, 2),
    (4, 3),
    (3, 4),
];
pub const SIZES: &[((u8, u8), &str)] = &[
    ((0, 1), "L 3:2"),
    ((0, 2), "L 16:9"),
    ((0, 3), "L 1:1"),
    ((1, 1), "M 3:2"),
    ((1, 2), "M 16:9"),
    ((1, 3), "M 1:1"),
    ((2, 1), "S 3:2"),
    ((2, 2), "S 16:9"),
    ((2, 3), "S 1:1"),
];
pub const QUALITY: &[(u8, &str)] = &[
    (0, "fine"),
    (1, "normal"),
    (2, "fine_raw"),
    (3, "normal_raw"),
    (4, "raw"),
];
pub const DR: &[(u8, &str)] = &[(0, "auto"), (1, "hdr100"), (2, "hdr200"), (3, "hdr400")];
pub const DR_PRIORITY: &[(u8, &str)] = &[(0, "auto"), (2, "strong"), (3, "weak"), (4, "off")];
pub const LEVEL: &[(u8, &str)] = &[(0, "off"), (1, "weak"), (2, "strong")];
pub const PORTRAIT_ENHANCER: &[(u8, &str)] =
    &[(0, "strong"), (1, "medium"), (2, "weak"), (3, "off")];
/// (strength, size) -> grain effect; strength 2 is off whatever the size.
pub const GRAIN: &[((u8, u8), &str)] = &[
    ((2, 0), "off"),
    ((2, 1), "off"),
    ((0, 1), "strong_large"),
    ((1, 1), "weak_large"),
    ((0, 0), "strong_small"),
    ((1, 0), "weak_small"),
];
pub const RAW_RECORDING: &[(u8, &str)] = &[(0, "uncompressed"), (1, "lossless"), (2, "compressed")];
pub const MOUNT_FOCAL_MM: &[(u8, i64)] = &[(0, 21), (1, 24), (2, 28), (3, 35), (4, 50), (5, 75)];
/// ISO in 1/3 stops from 80.
pub const ISO_THIRDS: &[i64] = &[
    80, 100, 125, 160, 200, 250, 320, 400, 500, 640, 800, 1000, 1250, 1600, 2000, 2500, 3200, 4000,
    5000, 6400, 8000, 10000, 12800,
];
/// Selected ISO codes outside the regular 200..12800 run (15..=33).
pub const ISO_SPECIAL: &[(u8, &str)] = &[
    (1, "auto1"),
    (2, "auto2"),
    (3, "auto3"),
    (13, "80"),
    (14, "100"),
    (34, "25600"),
    (35, "51200"),
    (38, "125"),
    (39, "160"),
];
pub const MIN_SHUTTER: &[(u8, &str)] = &[
    (0, "auto"),
    (1, "1/4000"),
    (2, "1/2000"),
    (3, "1/1000"),
    (4, "1/500"),
    (5, "1/400"),
    (6, "1/320"),
    (7, "1/250"),
    (8, "1/200"),
    (9, "1/160"),
    (10, "1/125"),
    (11, "1/100"),
    (12, "1/80"),
    (13, "1/60"),
    (14, "1/40"),
    (15, "1/30"),
    (16, "1/15"),
    (17, "1/8"),
    (18, "1/4"),
    (19, "1/2"),
];

pub fn name_of<K: PartialEq + Copy, V: Copy>(table: &[(K, V)], key: K) -> Option<V> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

pub fn code_of<K: Copy, V: PartialEq + Copy>(table: &[(K, V)], value: V) -> Option<K> {
    table.iter().find(|(_, v)| *v == value).map(|(k, _)| *k)
}

pub fn is_monochrome(film: &str) -> bool {
    film == "sepia" || film.starts_with("monochrome") || film.starts_with("acros")
}
