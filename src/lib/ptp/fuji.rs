use std::{
    fmt,
    io::{self, Write},
    ops::{Deref, DerefMut},
    str::FromStr,
};

use anyhow::{Context, bail};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ptp_cursor::PtpSerialize;
use ptp_macro::{PtpDeserialize, PtpSerialize};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum_macros::EnumIter;

use crate::{
    input::{Choices, CleanAlphanumeric},
    ptp::{ObjectFormat, ObjectInfo},
};

macro_rules! fuji_i16 {
    ($name:ident, $min:expr, $max:expr, $step:expr, $scale:literal) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, ptp_macro::PtpSerialize, ptp_macro::PtpDeserialize,
        )]
        pub struct $name(i16);

        impl $name {
            pub const MIN: i16 = $min;
            pub const MAX: i16 = $max;
            pub const STEP: i16 = $step;

            pub const SCALE: i16 = $scale;

            pub const RAW_MIN: i16 = $min * $scale;
            pub const RAW_MAX: i16 = $max * $scale;
            pub const RAW_STEP: i16 = $step * $scale;

            pub fn try_from_int(value: i16) -> anyhow::Result<Self> {
                if !(Self::MIN..=Self::MAX).contains(&value) {
                    anyhow::bail!("Value {} is out of range", value);
                }

                #[allow(clippy::modulo_one)]
                if (value - Self::MIN) % Self::STEP != 0 {
                    anyhow::bail!("Value {} is not aligned to step {}", value, Self::STEP);
                }

                let raw = value * Self::SCALE;

                Ok(Self(raw))
            }

            pub const fn to_int(self) -> i16 {
                self.0 / Self::SCALE
            }
        }

        impl std::ops::Deref for $name {
            type Target = i16;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl std::convert::TryFrom<i16> for $name {
            type Error = anyhow::Error;

            fn try_from(value: i16) -> anyhow::Result<Self> {
                if !(Self::RAW_MIN..=Self::RAW_MAX).contains(&value) {
                    anyhow::bail!("Value {} is out of range", value);
                }

                #[allow(clippy::modulo_one)]
                if (value - Self::RAW_MIN) % Self::RAW_STEP != 0 {
                    anyhow::bail!("Value {} is not aligned to step {}", value, Self::RAW_STEP);
                }

                Ok(Self(value))
            }
        }

        impl std::convert::From<$name> for i16 {
            fn from(value: $name) -> i16 {
                *value.deref()
            }
        }

        impl std::str::FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> anyhow::Result<Self> {
                use crate::input::CleanAlphanumeric;
                use anyhow::Context;

                let input = s
                    .clean()
                    .parse::<i16>()
                    .with_context(|| format!("Invalid numeric value '{s}'"))?;

                Self::try_from_int(input)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.to_int())
            }
        }
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_i16(self.to_int())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let val = i16::deserialize(deserializer)?;
                Self::try_from_int(val).map_err(serde::de::Error::custom)
            }
        }
    };
}

macro_rules! fuji_i16_frac {
    ($name:ident, $min:expr, $max:expr, $step:expr, $scale:literal) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, ptp_macro::PtpSerialize, ptp_macro::PtpDeserialize,
        )]
        pub struct $name(i16);

        impl $name {
            pub const MIN: f32 = $min;
            pub const MAX: f32 = $max;
            pub const STEP: f32 = $step;

            pub const SCALE: f32 = $scale as f32;

            #[allow(clippy::cast_possible_truncation)]
            pub const RAW_MIN: i16 = ($min * $scale as f32) as i16;
            #[allow(clippy::cast_possible_truncation)]
            pub const RAW_MAX: i16 = ($max * $scale as f32) as i16;
            #[allow(clippy::cast_possible_truncation)]
            pub const RAW_STEP: i16 = ($step * $scale as f32) as i16;

            pub fn try_from_float(value: f32) -> anyhow::Result<Self> {
                if !(Self::MIN..=Self::MAX).contains(&value) {
                    anyhow::bail!("Value {} is out of range", value);
                }

                #[allow(clippy::modulo_one)]
                if (value - Self::MIN) % Self::STEP != 0.0 {
                    anyhow::bail!("Value {} is not aligned to step {}", value, Self::STEP);
                }

                #[allow(clippy::cast_possible_truncation)]
                let raw = (value * Self::SCALE).round() as i16;

                Ok(Self(raw))
            }

            pub fn to_float(self) -> f32 {
                f32::from(self.0) / Self::SCALE
            }
        }

        impl std::ops::Deref for $name {
            type Target = i16;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl std::convert::TryFrom<i16> for $name {
            type Error = anyhow::Error;

            fn try_from(value: i16) -> anyhow::Result<Self> {
                if !(Self::RAW_MIN..=Self::RAW_MAX).contains(&value) {
                    anyhow::bail!("Value {} is out of range", value);
                }

                #[allow(clippy::modulo_one)]
                if (value - Self::RAW_MIN) % Self::RAW_STEP != 0 {
                    anyhow::bail!("Value {} is not aligned to step {}", value, Self::RAW_STEP);
                }

                Ok(Self(value))
            }
        }

        impl std::convert::From<$name> for i16 {
            fn from(value: $name) -> i16 {
                *value.deref()
            }
        }

        impl std::str::FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> anyhow::Result<Self> {
                use crate::input::CleanAlphanumeric;
                use anyhow::Context;

                let input = s
                    .clean()
                    .parse::<f32>()
                    .with_context(|| format!("Invalid numeric value '{s}'"))?;

                Self::try_from_float(input)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.to_float())
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_f32(self.to_float())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = f32::deserialize(deserializer)?;
                Self::try_from_float(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

macro_rules! fuji_bool {
    ($name:ident, $true_variant:ident, $false_variant:ident) => {
        #[repr(u16)]
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            num_enum::IntoPrimitive,
            num_enum::TryFromPrimitive,
            ptp_macro::PtpSerialize,
            ptp_macro::PtpDeserialize,
            strum_macros::EnumIter,
        )]
        pub enum $name {
            $true_variant = 0x1,
            $false_variant = 0x2,
        }

        impl $name {
            pub const fn to_bool(self) -> bool {
                matches!(self, Self::$true_variant)
            }

            pub const fn from_bool(b: bool) -> Self {
                if b {
                    Self::$true_variant
                } else {
                    Self::$false_variant
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let s = match self.to_bool() {
                    true => "On",
                    false => "Off",
                };
                write!(f, "{}", s)
            }
        }

        impl std::str::FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> anyhow::Result<Self> {
                match s.clean().as_str() {
                    "true" | "on" => return Ok(Self::$true_variant),
                    "false" | "off" => return Ok(Self::$false_variant),
                    _ => {}
                }

                if let Some(best) = Self::closest(s) {
                    anyhow::bail!(
                        "Unknown {} '{}'. Did you mean '{best}'?",
                        stringify!($name),
                        s
                    );
                }

                anyhow::bail!("Unknown {} '{}'", stringify!($name), s);
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_bool(self.to_bool())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let b = bool::deserialize(deserializer)?;
                Ok(Self::from_bool(b))
            }
        }
    };
}

macro_rules! fuji_enum {
    (
        $(#[$enum_meta:meta])*
        $name:ident, {
            $(
                $(#[$variant_meta:meta])*
                $variant_name:ident = $variant_value:expr, $display_string:literal, [$($match_string:literal),* $(,)?]
            ),* $(,)?
        }
    ) => {
        #[repr(u16)]
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            num_enum::IntoPrimitive,
            num_enum::TryFromPrimitive,
            ptp_macro::PtpSerialize,
            ptp_macro::PtpDeserialize,
            strum_macros::EnumIter,
        )]
        $(#[$enum_meta])*
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant_name = $variant_value,
            )*
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$variant_name => write!(f, $display_string),)*
                }
            }
        }

        impl std::str::FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> anyhow::Result<Self> {
                match s.clean().as_str() {
                    $($($match_string)|* => return Ok(Self::$variant_name),)*
                    _ => {}
                }

                if let Some(best) = Self::closest(s) {
                    anyhow::bail!("Unknown {} '{s}'. Did you mean '{best}'?", stringify!($name));
                }

                anyhow::bail!("Unknown {} '{s}'", stringify!($name));
            }
        }
    };
}

macro_rules! fuji_try_conv_bits {
    ($name:ident, $from:ty, $to:ty) => {
        impl std::convert::TryFrom<$from> for $name {
            type Error = anyhow::Error;

            fn try_from(value: $from) -> anyhow::Result<Self> {
                let primitive = <$to>::try_from(value)?;
                #[allow(clippy::needless_question_mark)]
                Ok($name::try_from(primitive)?)
            }
        }

        impl std::convert::From<$name> for $from {
            fn from(value: $name) -> $from {
                let primitive: $to = value.into();
                <$from>::from(primitive)
            }
        }
    };
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr, Default)]
    FileType, {
        #[default]
        Jpeg = 0x7, "JPEG", ["jpeg", "jpg"],
        Heif = 0x12, "HEIF", ["heif"],
        Tiff8 = 0x9, "TIFF 8-bit", ["tiff8", "tiff8bit"],
        Tiff16 = 0xb, "TIFF 16-bit", ["tiff16", "tiff16bit"],
    }
}

fuji_enum! {
    #[derive(Serialize_repr, Deserialize_repr)]
    CustomSetting, {
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
    ImageSize, {
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
    ImageQuality, {
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
    DynamicRange, {
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
    DynamicRangePriority, {
        Auto = 0x8000, "Auto", ["auto", "drpauto"],
        Plus = 0x3, "Plus", ["plus"], // Used in conjuction with HDR800 to represent HDR800+
        Strong = 0x2, "Strong", ["strong", "drpstrong"],
        Weak = 0x1, "Weak", ["weak", "drpweak"],
        Off = 0x0, "Off", ["off", "drpoff"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    FilmSimulation, {
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
        AcrosR = 0xe, "Acros + R", ["acrosr", "acrosred"],
        AcrosG = 0xf, "Acros + G", ["acrosg", "acrosgreen"],
        Eterna = 0x10, "Eterna", ["eterna"],
        ClassicNegative = 0x11, "Classic Negative", ["classicneg", "classicnegative"],
        NostalgicNegative = 0x13, "Nostalgic Negative", ["nostalgicneg", "nostalgicnegative"],
        EternaBleachBypass = 0x12, "Eterna Bleach Bypass", ["eternabb", "eternableach", "eternableachbypass"],
        RealaAce = 0x14, "Reala Ace", ["realaace", "reala"],
    }
}

impl FilmSimulation {
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
    GrainEffect, {
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
    ColorChromeEffect, {
        Strong = 0x3, "Strong", ["strong"],
        Weak = 0x2, "Weak", ["weak"],
        Off = 0x1, "Off", ["off"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    ColorChromeFXBlue, {
        Strong = 0x3, "Strong", ["strong"],
        Weak = 0x2, "Weak", ["weak"],
        Off = 0x1, "Off", ["off"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    SmoothSkinEffect, {
        Strong = 0x3, "Strong", ["strong"],
        Weak = 0x2, "Weak", ["weak"],
        Off = 0x1, "Off", ["off"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr, Default)]
    WhiteBalance, {
        AsShot = 0x0, "As Shot", ["asshot", "original"],
        WhitePriority = 0x8020, "White Priority", ["whitepriority", "white"],
        #[default]
        Auto = 0x2, "Auto", ["auto"],
        AmbiencePriority = 0x8021, "Ambience Priority", ["ambiencepriority", "ambience", "ambient"],
        Custom1 = 0x8008, "Custom 1", ["custom1", "c1"],
        Custom2 = 0x8009, "Custom 2", ["custom2", "c2"],
        Custom3 = 0x800A, "Custom 3", ["custom3", "c3"],
        Temperature = 0x8007, "Temperature", ["temperature", "temp", "k", "kelvin"],
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
    ColorSpace, {
        #[allow(clippy::upper_case_acronyms)]
        SRGB = 0x2, "sRGB", ["s", "srgb"],
        AdobeRGB = 0x1, "Adobe RGB", ["adobe", "adobergb"],
    }
}

fuji_enum! {
    #[derive(SerializeDisplay, DeserializeFromStr)]
    UsbMode, {
        RawConversion = 0x6, "Raw Conversion", ["raw", "rawconversion"],
    }
}

fuji_i16!(MonochromaticColorShift, -18, 18, 1, 10i16);
fuji_i16!(WhiteBalanceShift, -9, 9, 1, 1i16);
fuji_i16!(WhiteBalanceTemperature, 2500, 10000, 10, 1i16);
fuji_i16!(Color, -4, 4, 1, 10i16);
fuji_i16!(Sharpness, -4, 4, 1, 10i16);
fuji_i16!(Clarity, -5, 5, 1, 10i16);

fuji_i16_frac!(HighlightTone, -2.0, 4.0, 0.5, 10i16);
fuji_i16_frac!(ShadowTone, -2.0, 4.0, 0.5, 10i16);

fuji_bool!(WhiteBalanceAsShot, True, False);
fuji_bool!(LensModulationOptimizer, On, Off);
fuji_bool!(Teleconverter, On, Off);

#[derive(
    Debug, Clone, PartialEq, Eq, PtpSerialize, PtpDeserialize, SerializeDisplay, DeserializeFromStr,
)]
pub struct CustomSettingName(String);

impl CustomSettingName {
    pub const MAX_LEN: usize = 25;
}

impl fmt::Display for CustomSettingName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &**self)
    }
}

impl Deref for CustomSettingName {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CustomSettingName {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromStr for CustomSettingName {
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
pub enum ExposureOffset {
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

impl ExposureOffset {
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

impl fmt::Display for ExposureOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_float())
    }
}

impl FromStr for ExposureOffset {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s
            .clean()
            .parse::<f32>()
            .with_context(|| format!("Invalid numeric value '{s}'"))?;

        Self::try_from_float(input)
    }
}

impl Serialize for ExposureOffset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f32(self.to_float())
    }
}

impl<'de> Deserialize<'de> for ExposureOffset {
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
pub enum NoiseReduction {
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

impl NoiseReduction {
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

impl fmt::Display for NoiseReduction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_int() {
            0 => write!(f, "0"),
            n => write!(f, "{n:+}"),
        }
    }
}

impl FromStr for NoiseReduction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let value = s
            .clean()
            .parse::<i16>()
            .with_context(|| format!("Invalid numeric value '{s}'"))?;

        Self::try_from_int(value)
    }
}

impl Serialize for NoiseReduction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i16(self.to_int())
    }
}

impl<'de> Deserialize<'de> for NoiseReduction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = i16::deserialize(deserializer)?;
        Self::try_from_int(value).map_err(serde::de::Error::custom)
    }
}

fuji_try_conv_bits!(ImageSize, u32, u16);
fuji_try_conv_bits!(ImageQuality, u32, u16);
fuji_try_conv_bits!(DynamicRange, u32, u16);
fuji_try_conv_bits!(DynamicRangePriority, u32, u16);
fuji_try_conv_bits!(FilmSimulation, u32, u16);
fuji_try_conv_bits!(FileType, u32, u16);
fuji_try_conv_bits!(GrainEffect, u32, u16);
fuji_try_conv_bits!(ColorChromeEffect, u32, u16);
fuji_try_conv_bits!(ColorChromeFXBlue, u32, u16);
fuji_try_conv_bits!(SmoothSkinEffect, u32, u16);
fuji_try_conv_bits!(WhiteBalance, u32, u16);
fuji_try_conv_bits!(NoiseReduction, u32, u16);
fuji_try_conv_bits!(ColorSpace, u32, u16);
fuji_try_conv_bits!(ExposureOffset, i32, i16);
fuji_try_conv_bits!(MonochromaticColorShift, i32, i16);
fuji_try_conv_bits!(WhiteBalanceShift, i32, i16);
fuji_try_conv_bits!(WhiteBalanceTemperature, i32, i16);
fuji_try_conv_bits!(Color, i32, i16);
fuji_try_conv_bits!(Sharpness, i32, i16);
fuji_try_conv_bits!(Clarity, i32, i16);

fuji_try_conv_bits!(HighlightTone, i32, i16);
fuji_try_conv_bits!(ShadowTone, i32, i16);

fuji_try_conv_bits!(WhiteBalanceAsShot, u32, u16);
fuji_try_conv_bits!(LensModulationOptimizer, u32, u16);
fuji_try_conv_bits!(Teleconverter, u32, u16);

// NOTE: Naively assuming that all cameras support backup/restore using the same structs.
pub struct BackupObjectInfo {
    compressed_size: u32,
}

impl BackupObjectInfo {
    pub fn new(buffer_len: usize) -> anyhow::Result<Self> {
        Ok(Self {
            compressed_size: u32::try_from(buffer_len)?,
        })
    }
}

impl PtpSerialize for BackupObjectInfo {
    fn try_into_ptp(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.try_write_ptp(&mut buf)?;
        Ok(buf)
    }

    fn try_write_ptp(&self, buf: &mut Vec<u8>) -> io::Result<()> {
        let object_info = ObjectInfo {
            object_format: ObjectFormat::FujiBackup,
            compressed_size: self.compressed_size,
            ..Default::default()
        };

        object_info.try_write_ptp(buf)?;
        buf.write_all(&[0x0u8; 1020])?;

        Ok(())
    }
}
