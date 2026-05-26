pub mod error;

pub use error::ImageError;

use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt};
use exiftool::ExifTool;
use log::debug;

use crate::{CoreResult, features::simulation::Simulation};

pub const MAKER_NOTES_TAG: &str = "MakerNotes";

const ENTRY_SIZE: u64 = 12;

pub fn extract_simulation(image: &Path) -> CoreResult<Box<dyn Simulation>> {
    let exiftool = ExifTool::new().map_err(ImageError::from)?;
    let maker_notes = exiftool
        .read_tag_binary(image, MAKER_NOTES_TAG)
        .map_err(ImageError::from)?;
    debug!("{maker_notes:x?}");

    let mut cursor = Cursor::new(&maker_notes);

    let mut header = [0u8; 8];
    cursor.read_exact(&mut header).map_err(ImageError::from)?;

    if header != *b"FUJIFILM" {
        return Err(ImageError::NotFujifilm.into());
    }
    debug!("Correct header");

    let offset = cursor
        .read_u16::<LittleEndian>()
        .map_err(ImageError::from)?;
    debug!("Offset: {offset:x?}");

    cursor.set_position(offset.into());

    let entries_len = cursor
        .read_u16::<LittleEndian>()
        .map_err(ImageError::from)?;
    debug!("Entries: {entries_len:?}");

    let mut fields: HashMap<FujiExifMakerNoteTag, &[u8]> =
        HashMap::with_capacity(entries_len as usize);

    for _ in 0..entries_len {
        let tag = FujiExifMakerNoteTag::try_from(
            cursor
                .read_u16::<LittleEndian>()
                .map_err(ImageError::from)?,
        );
        let field_type = IFDType::try_from(
            cursor
                .read_u16::<LittleEndian>()
                .map_err(ImageError::from)?,
        );
        let count = cursor
            .read_u32::<LittleEndian>()
            .map_err(ImageError::from)?;
        let raw = cursor
            .read_u32::<LittleEndian>()
            .map_err(ImageError::from)?;

        let Ok(tag) = tag else { continue };
        let Ok(field_type) = field_type else { continue };

        let value_len = field_type.size() * count as usize;
        let entry_start = cursor.position() - ENTRY_SIZE;

        let start: usize = if value_len <= 4 {
            (entry_start + 8)
                .try_into()
                .map_err(|_| ImageError::MalformedField {
                    offset: entry_start,
                })?
        } else {
            raw.try_into().map_err(|_| ImageError::MalformedField {
                offset: entry_start,
            })?
        };

        let end = start
            .checked_add(value_len)
            .ok_or(ImageError::MalformedField {
                offset: entry_start,
            })?;
        let value: &[u8] = maker_notes
            .get(start..end)
            .ok_or(ImageError::MalformedField {
                offset: entry_start,
            })?;

        fields.insert(tag, value);
    }

    debug!("{fields:x?}");

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
    Contrast2 = 0x1006,       // What
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
    FilmMode = 0x1401,                // Film Simulation
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
    #[must_use]
    pub const fn size(&self) -> usize {
        match self {
            Self::Byte | Self::Ascii | Self::SByte | Self::Undefined => 1,
            Self::Short | Self::SShort => 2,
            Self::Long | Self::SLong | Self::Float => 4,
            Self::Rational | Self::SRational | Self::Double => 8,
        }
    }
}
