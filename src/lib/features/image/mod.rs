use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::Path,
};

use anyhow::bail;
use byteorder::{LittleEndian, ReadBytesExt};
use exiftool::ExifTool;
use log::debug;

use crate::features::simulation::Simulation;

pub const MAKER_NOTES_TAG: &'static str = "MakerNotes";

pub fn extract_simulation(image: &Path) -> anyhow::Result<Box<dyn Simulation>> {
    let mut exiftool = ExifTool::new()?;

    let maker_notes = exiftool.read_tag_binary(image, MAKER_NOTES_TAG)?;
    debug!("{:x?}", maker_notes);

    let mut cursor = Cursor::new(&maker_notes);

    let mut header = [0u8; 8];
    cursor.read_exact(&mut header)?;

    if header != *b"FUJIFILM" {
        bail!("Not Fujifilm MakerNotes");
    } else {
        debug!("Correct header");
    }

    let offset = cursor.read_u16::<LittleEndian>()?;
    debug!("Offset: {:x?}", offset);

    cursor.set_position(offset.into());

    let entries_len = cursor.read_u16::<LittleEndian>()?;
    debug!("Entries: {:?}", entries_len);

    let mut fields: HashMap<FujiExifMakerNoteTag, &[u8]> =
        HashMap::with_capacity(entries_len as usize);

    for _ in 0..entries_len {
        let tag = FujiExifMakerNoteTag::try_from(cursor.read_u16::<LittleEndian>()?);
        let field_type = IFDType::try_from(cursor.read_u16::<LittleEndian>()?);
        let count = cursor.read_u32::<LittleEndian>()?;
        let raw = cursor.read_u32::<LittleEndian>()?;

        let tag = match tag {
            Ok(tag) => tag,
            Err(_) => continue,
        };
        let field_type = match field_type {
            Ok(field_type) => field_type,
            Err(_) => continue,
        };

        let value_len = field_type.size() * count as usize;

        let value: &[u8] = if value_len <= 4 {
            let start = (cursor.position() - 4).try_into()?;
            &maker_notes[start..start + value_len]
        } else {
            let start = raw.try_into()?;
            &maker_notes[start..start + value_len]
        };

        fields.insert(tag, value);
    }

    debug!("{:x?}", fields);

    todo!()
}

#[repr(u16)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, num_enum::IntoPrimitive, num_enum::TryFromPrimitive,
)]
pub enum FujiExifMakerNoteTag {
    // ImageSize, Get from metadata
    ImageQuality = 0x1000,
    Sharpness = 0x1001,
    WhiteBalance = 0x1002,
    Saturation = 0x1003, // Both Color and monochrome/sepia film simulations, motherfucker
    Contrast1 = 0x1004,  // What
    ColorTemperature = 0x1005,
    Contrast2 = 0x1006, // What
    NoiseReduction1 = 0x100b, // for older cameras maybe?
    NoiseReduction2 = 0x100e,
    Clarity = 0x100f,
    Shadow = 0x1040,
    Highlight = 0x1041,
    LensModulationOptimizer = 0x1045,
    GrainEffectRoughness = 0x1047,
    ColorChromeEffect = 0x1048,
    MonochromaticColorTemperature = 0x1049,
    MonochromaticColorTint = 0x104b,
    GrainEffectSize = 0x104c,
    ColorChromeFXBlue = 0x104e,
    FilmMode = 0x1401, // Film Simulation
    DevelopmentDynamicRange = 0x1403, // This is the one we want for XT-5
    SmoothSkinEffect = 0x104a,
    WhiteBalanceShiftRed = 0x144a,
    WhiteBalanceShiftGreen = 0x144b,
    WhiteBalanceShiftBlue = 0x144c,
    DynamicRangePriority = 0x1444, // same bollocks with HDR800+ as PTP
    ColorSpace = 0xa001,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive,
)]
#[repr(u16)]
pub enum IFDType {
    Byte = 1,       // 8-bit unsigned integer
    Ascii = 2,      // 8-bit ASCII character
    Short = 3,      // 16-bit unsigned integer
    Long = 4,       // 32-bit unsigned integer
    Rational = 5,   // 2x Long: numerator / denominator
    SByte = 6,      // 8-bit signed integer
    Undefined = 7,  // 8-bit byte, arbitrary data
    SShort = 8,     // 16-bit signed integer
    SLong = 9,      // 32-bit signed integer
    SRational = 10, // 2x SLong: signed numerator / denominator
    Float = 11,     // 4-byte IEEE float
    Double = 12,    // 8-byte IEEE double
}

impl IFDType {
    pub fn size(&self) -> usize {
        match self {
            IFDType::Byte | IFDType::Ascii | IFDType::SByte | IFDType::Undefined => 1,
            IFDType::Short | IFDType::SShort => 2,
            IFDType::Long | IFDType::SLong | IFDType::Float => 4,
            IFDType::Rational | IFDType::SRational | IFDType::Double => 8,
        }
    }
}
