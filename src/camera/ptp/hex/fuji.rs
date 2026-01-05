use std::{
    fmt,
    ops::{Deref, DerefMut},
    str::FromStr,
};

use anyhow::{Context, bail};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ptp_macro::{PtpDeserialize, PtpSerialize};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum_macros::EnumIter;

use crate::camera::ptp::input::{Choices, CleanAlphanumeric};

use super::macros::{fuji_bool, fuji_enum, fuji_i16, fuji_i16_frac, fuji_try_conv_bits};

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr, Default)]
    FujiFileType, {
        #[default]
        Jpeg = 0x7, "JPEG", ["jpeg", "jpg"],
        Heif = 0x12, "HEIF", ["heif"],
        Tiff8 = 0x9, "TIFF 8-bit", ["tiff8", "tiff8bit"],
        Tiff16 = 0xb, "TIFF 16-bit", ["tiff16", "tiff16bit"],
    }
}

fuji_enum! {
    #[derive(Serialize_repr, Deserialize_repr)]
    FujiCustomSetting, {
        C1 = 0x1, "C1", ["c1", "1"],
        C2 = 0x2, "C2", ["c2", "2"],
        C3 = 0x3, "C3", ["c3", "3"],
        C4 = 0x4, "C4", ["c4", "4"],
        C5 = 0x5, "C5", ["c5", "5"],
        C6 = 0x6, "C6", ["c6", "6"],
        C7 = 0x7, "C7", ["c7", "7"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr, Default)]
    FujiImageSize, {
        #[default]
        R7728x5152 = 0x7, "7728x5152", ["7728x5152"],
        R7728x4344 = 0x8, "7728x4344", ["7728x4344"],
        R5152x5152 = 0x9, "5152x5152", ["5152x5152"],
        R6864x5152 = 0xe, "6864x5152", ["6864x5152"],
        R6432x5152 = 0x10, "6432x5152", ["6432x5152"],
        R5472x3648 = 0x4, "5472x3648", ["5472x3648"],
        R5472x3080 = 0x5, "5472x3080", ["5472x3080"],
        R3648x3648 = 0x6, "3648x3648", ["3648x3648"],
        R4864x3648 = 0x12, "4864x3648", ["4864x3648"],
        R4560x3648 = 0x14, "4560x3648", ["4560x3648"],
        R3888x2592 = 0x1, "3888x2592", ["3888x2592"],
        R3888x2184 = 0x2, "3888x2184", ["3888x2184"],
        R2592x2592 = 0x3, "2592x2592", ["2592x2592"],
        R3456x2592 = 0xa, "3456x2592", ["3456x2592"],
        R3264x2592 = 0xc, "3264x2592", ["3264x2592"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr, Default)]
    FujiImageQuality, {
        #[default]
        FineRaw = 0x4, "Fine + RAW", ["fineraw"],
        Fine = 0x2, "Fine", ["fine"],
        NormalRaw = 0x5, "Normal + RAW", ["normalraw"],
        Normal = 0x3, "Normal", ["normal"],
        Raw = 0x1, "RAW", ["raw"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    FujiDynamicRange, {
        Auto = 0xffff, "Auto", ["auto", "hdrauto", "drauto"],
        HDR100 = 0x64, "HDR100", ["100", "hdr100", "dr100"],
        HDR200 = 0xc8, "HDR200", ["200", "hdr200", "dr200"],
        HDR400 = 0x190, "HDR400", ["400", "hdr400", "dr400"],
        HDR800 = 0x320, "HDR800", ["800", "hdr800", "dr800"],
        HDR800Plus = 0x640, "HDR800+", ["800+", "800plus", "hdr800+", "hdr800plus", "dr800+", "dr800plus"] // Not currently used by fuji devices directly, added for UX.
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    FujiDynamicRangePriority, {
        Auto = 0x8000, "Auto", ["auto", "drpauto"],
        Plus = 0x3, "Plus", ["plus"], // Used in conjuction with HDR800 to represent HDR800+
        Strong = 0x2, "Strong", ["strong", "drpstrong"],
        Weak = 0x1, "Weak", ["weak", "drpweak"],
        Off = 0x0, "Off", ["off", "drpoff"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    FujiFilmSimulation, {
        Provia = 0x1, "Provia", ["provia"],
        Velvia = 0x2, "Velvia", ["velvia"],
        Astia = 0x3, "Astia", ["astia"],
        PRONegHi = 0x4, "PRO Neg. Hi", ["proneghi", "proneghigh"],
        PRONegStd = 0x5, "PRO Neg. Std", ["pronegstd", "pronegstandard"],
        Monochrome = 0x6, "Monochrome", ["mono", "monochrome"],
        MonochromeYe = 0x7, "Monochrome + Ye", ["monoy", "monoye", "monoyellow", "monochromey", "monochromeye", "monochromeyellow"],
        MonochromeR = 0x8, "Monochrome + R", ["monor", "monored", "monochromer", "monochromered"],
        MonochromeG = 0x9, "Monochrome + G", ["monog", "monogreen", "monochromeg", "monochromegreen"],
        Sepia = 0xa, "Sepia", ["sepia"],
        ClassicChrome = 0xb, "Classic Chrome", ["classicchrome"],
        AcrosSTD = 0xc, "Acros", ["acros"],
        AcrosYe = 0xd, "Acros + Ye", ["acrosy", "acrosye", "acrosyellow"],
        AcrosR = 0xe, "Acros + R", ["acrossr", "acrossred"],
        AcrosG = 0xf, "Acros + G", ["acrossg", "acrossgreen"],
        Eterna = 0x10, "Eterna", ["eterna"],
        ClassicNegative = 0x11, "Classic Negative", ["classicneg", "classicnegative"],
        NostalgicNegative = 0x13, "Nostalgic Negative", ["nostalgicneg", "nostalgicnegative"],
        EternaBleachBypass = 0x12, "Eterna Bleach Bypass", ["eternabb", "eternableach", "eternableachbypass"],
        RealaAce = 0x14, "Reala Ace", ["realaace"],
    }
}

impl FujiFilmSimulation {
    pub const fn is_black_and_white(self) -> bool {
        matches!(
            self,
            Self::Monochrome
                | Self::MonochromeYe
                | Self::MonochromeR
                | Self::MonochromeG
                | Self::AcrosSTD
                | Self::AcrosYe
                | Self::AcrosR
                | Self::AcrosG
        )
    }
}
fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    FujiGrainEffect, {
        StrongLarge = 0x5, "Strong Large", ["stronglarge", "largestrong"],
        WeakLarge = 0x4, "Weak Large", ["weaklarge", "largeweak"],
        StrongSmall = 0x3, "Strong Small", ["strongsmall", "smallstrong"],
        WeakSmall = 0x2, "Weak Small", ["weaksmall", "smallweak"],
        #[num_enum(alternatives = [0x6, 0x7])] // TODO: Figure out what's going on here. Even if we immediately get after setting to 0x1, we might get 0x6 or 0x7.
        Off = 0x1, "Off", ["off"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    FujiColorChromeEffect, {
        Strong = 0x3, "Strong", ["strong"],
        Weak = 0x2, "Weak", ["weak"],
        Off = 0x1, "Off", ["off"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    FujiColorChromeFXBlue, {
        Strong = 0x3, "Strong", ["strong"],
        Weak = 0x2, "Weak", ["weak"],
        Off = 0x1, "Off", ["off"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    FujiSmoothSkinEffect, {
        Strong = 0x3, "Strong", ["strong"],
        Weak = 0x2, "Weak", ["weak"],
        Off = 0x1, "Off", ["off"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr, Default)]
    FujiWhiteBalance, {
        AsShot = 0x0, "As Shot", ["asshot", "original"],
        WhitePriority = 0x8020, "White Priority", ["whitepriority", "white"],
        #[default]
        Auto = 0x2, "Auto", ["auto"],
        AmbiencePriority = 0x8021, "Ambience Priority", ["ambiencepriority", "ambience", "ambient"],
        Custom1 = 0x8008, "Custom 1", ["custom1", "c1"],
        Custom2 = 0x8009, "Custom 2", ["custom2", "c2"],
        Custom3 = 0x800A, "Custom 3", ["custom3", "c3"],
        Temperature = 0x8007, "Temperature", ["temperature", "k", "kelvin"],
        Daylight = 0x4, "Daylight", ["daylight", "sunny"],
        Shade = 0x8006, "Shade", ["shade", "cloudy"],
        Fluorescent1 = 0x8001, "Fluorescent 1", ["fluorescent1"],
        Fluorescent2 = 0x8002, "Fluorescent 2", ["fluorescent2"],
        Fluorescent3 = 0x8003, "Fluorescent 3", ["fluorescent3"],
        Incandescent = 0x6, "Incandescent", ["incandescent", "tungsten"],
        Underwater = 0x8, "Underwater", ["underwater"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    FujiColorSpace, {
        #[allow(clippy::upper_case_acronyms)]
        SRGB = 0x2, "sRGB", ["s", "srgb"],
        AdobeRGB = 0x1, "Adobe RGB", ["adobe", "adobergb"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    FujiUsbMode, {
        RawConversion = 0x6, "Raw Conversion", ["raw", "rawconversion"],
    }
}

fuji_i16!(FujiMonochromaticColorShift, -18, 18, 1, 10i16);
fuji_i16!(FujiWhiteBalanceShift, -9, 9, 1, 1i16);
fuji_i16!(FujiWhiteBalanceTemperature, 2500, 10000, 10, 1i16);
fuji_i16!(FujiColor, -4, 4, 1, 10i16);
fuji_i16!(FujiSharpness, -4, 4, 1, 10i16);
fuji_i16!(FujiClarity, -5, 5, 1, 10i16);

fuji_i16_frac!(FujiHighlightTone, -2.0, 4.0, 0.5, 10i16);
fuji_i16_frac!(FujiShadowTone, -2.0, 4.0, 0.5, 10i16);

fuji_bool!(FujiWhiteBalanceAsShot, True, False);
fuji_bool!(FujiLensModulationOptimizer, On, Off);
fuji_bool!(FujiTeleconverter, On, Off);

#[derive(
    Debug, Clone, PartialEq, Eq, PtpSerialize, PtpDeserialize, SerializeDisplay, DeserializeFromStr,
)]
pub struct FujiCustomSettingName(String);

impl FujiCustomSettingName {
    pub const MAX_LEN: usize = 25;
}

impl fmt::Display for FujiCustomSettingName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &**self)
    }
}

impl Deref for FujiCustomSettingName {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FujiCustomSettingName {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromStr for FujiCustomSettingName {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        if s.len() > Self::MAX_LEN {
            bail!("Value '{}' exceeds max length of {}", s, Self::MAX_LEN);
        }
        Ok(Self(s.to_string()))
    }
}

#[repr(i16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiExposureOffset {
    Plus3 = 3000,
    Plus2_7 = 2667,
    Plus2_3 = 2333,
    Plus2 = 2000,
    Plus1_7 = 1667,
    Plus1_3 = 1333,
    Plus1 = 1000,
    Plus0_7 = 667,
    Plus0_3 = 333,
    Zero = 0,
    Minus0_3 = -333,
    Minus0_7 = -667,
    Minus1 = -1000,
    Minus1_3 = -1333,
    Minus1_7 = -1667,
    Minus2 = -2000,
    Minus2_3 = -2333,
    Minus2_7 = -2667,
    Minus3 = -3000,
}

impl FujiExposureOffset {
    pub const fn to_float(self) -> f32 {
        match self {
            Self::Minus3 => -3.0,
            Self::Minus2_7 => -2.7,
            Self::Minus2_3 => -2.3,
            Self::Minus2 => -2.0,
            Self::Minus1_7 => -1.7,
            Self::Minus1_3 => -1.3,
            Self::Minus1 => -1.0,
            Self::Minus0_7 => -0.7,
            Self::Minus0_3 => -0.3,
            Self::Zero => 0.0,
            Self::Plus0_3 => 0.3,
            Self::Plus0_7 => 0.7,
            Self::Plus1 => 1.0,
            Self::Plus1_3 => 1.3,
            Self::Plus1_7 => 1.7,
            Self::Plus2 => 2.0,
            Self::Plus2_3 => 2.3,
            Self::Plus2_7 => 2.7,
            Self::Plus3 => 3.0,
        }
    }

    pub fn try_from_float(v: f32) -> anyhow::Result<Self> {
        let round = (v * 10.0).round() / 10.0;

        match round {
            3.0 => Ok(Self::Plus3),
            2.7 => Ok(Self::Plus2_7),
            2.3 => Ok(Self::Plus2_3),
            2.0 => Ok(Self::Plus2),
            1.7 => Ok(Self::Plus1_7),
            1.3 => Ok(Self::Plus1_3),
            1.0 => Ok(Self::Plus1),
            0.7 => Ok(Self::Plus0_7),
            0.3 => Ok(Self::Plus0_3),
            0.0 => Ok(Self::Zero),
            -0.3 => Ok(Self::Minus0_3),
            -0.7 => Ok(Self::Minus0_7),
            -1.0 => Ok(Self::Minus1),
            -1.3 => Ok(Self::Minus1_3),
            -1.7 => Ok(Self::Minus1_7),
            -2.0 => Ok(Self::Minus2),
            -2.3 => Ok(Self::Minus2_3),
            -2.7 => Ok(Self::Minus2_7),
            -3.0 => Ok(Self::Minus3),
            _ => bail!("Value {v} is out of range"),
        }
    }
}

impl fmt::Display for FujiExposureOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_float())
    }
}

impl FromStr for FujiExposureOffset {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s
            .clean()
            .parse::<f32>()
            .with_context(|| format!("Invalid numeric value '{s}'"))?;

        Self::try_from_float(input)
    }
}

impl Serialize for FujiExposureOffset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f32(self.to_float())
    }
}

impl<'de> Deserialize<'de> for FujiExposureOffset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f32::deserialize(deserializer)?;
        Self::try_from_float(value).map_err(serde::de::Error::custom)
    }
}

#[repr(u16)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, PtpSerialize, PtpDeserialize,
)]
pub enum FujiHighISONR {
    Plus4 = 0x5000,
    Plus3 = 0x6000,
    Plus2 = 0x0,
    Plus1 = 0x1000,
    Zero = 0x2000,
    Minus1 = 0x3000,
    Minus2 = 0x4000,
    Minus3 = 0x7000,
    Minus4 = 0x8000,
}

impl FujiHighISONR {
    pub const fn to_int(self) -> i16 {
        match self {
            Self::Plus4 => 4,
            Self::Plus3 => 3,
            Self::Plus2 => 2,
            Self::Plus1 => 1,
            Self::Zero => 0,
            Self::Minus1 => -1,
            Self::Minus2 => -2,
            Self::Minus3 => -3,
            Self::Minus4 => -4,
        }
    }

    pub fn try_from_int(value: i16) -> anyhow::Result<Self> {
        match value {
            4 => Ok(Self::Plus4),
            3 => Ok(Self::Plus3),
            2 => Ok(Self::Plus2),
            1 => Ok(Self::Plus1),
            0 => Ok(Self::Zero),
            -1 => Ok(Self::Minus1),
            -2 => Ok(Self::Minus2),
            -3 => Ok(Self::Minus3),
            -4 => Ok(Self::Minus4),
            _ => bail!("Value {value} is out of range"),
        }
    }
}

impl fmt::Display for FujiHighISONR {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_int() {
            0 => write!(f, "0"),
            n => write!(f, "{n:+}"),
        }
    }
}

impl FromStr for FujiHighISONR {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let value = s
            .clean()
            .parse::<i16>()
            .with_context(|| format!("Invalid numeric value '{s}'"))?;

        Self::try_from_int(value)
    }
}

impl Serialize for FujiHighISONR {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i16(self.to_int())
    }
}

impl<'de> Deserialize<'de> for FujiHighISONR {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = i16::deserialize(deserializer)?;
        Self::try_from_int(value).map_err(serde::de::Error::custom)
    }
}

fuji_try_conv_bits!(FujiImageSize, u32, u16);
fuji_try_conv_bits!(FujiImageQuality, u32, u16);
fuji_try_conv_bits!(FujiDynamicRange, u32, u16);
fuji_try_conv_bits!(FujiDynamicRangePriority, u32, u16);
fuji_try_conv_bits!(FujiFilmSimulation, u32, u16);
fuji_try_conv_bits!(FujiFileType, u32, u16);
fuji_try_conv_bits!(FujiGrainEffect, u32, u16);
fuji_try_conv_bits!(FujiColorChromeEffect, u32, u16);
fuji_try_conv_bits!(FujiColorChromeFXBlue, u32, u16);
fuji_try_conv_bits!(FujiSmoothSkinEffect, u32, u16);
fuji_try_conv_bits!(FujiWhiteBalance, u32, u16);
fuji_try_conv_bits!(FujiHighISONR, u32, u16);
fuji_try_conv_bits!(FujiColorSpace, u32, u16);
fuji_try_conv_bits!(FujiExposureOffset, i32, i16);
fuji_try_conv_bits!(FujiMonochromaticColorShift, i32, i16);
fuji_try_conv_bits!(FujiWhiteBalanceShift, i32, i16);
fuji_try_conv_bits!(FujiWhiteBalanceTemperature, i32, i16);
fuji_try_conv_bits!(FujiColor, i32, i16);
fuji_try_conv_bits!(FujiSharpness, i32, i16);
fuji_try_conv_bits!(FujiClarity, i32, i16);

fuji_try_conv_bits!(FujiHighlightTone, i32, i16);
fuji_try_conv_bits!(FujiShadowTone, i32, i16);

fuji_try_conv_bits!(FujiWhiteBalanceAsShot, u32, u16);
fuji_try_conv_bits!(FujiLensModulationOptimizer, u32, u16);
fuji_try_conv_bits!(FujiTeleconverter, u32, u16);
