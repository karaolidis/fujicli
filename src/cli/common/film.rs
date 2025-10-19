use std::{fmt, ops::Deref, str::FromStr};

use anyhow::{Context, bail};
use clap::Args;
use serde::{Serialize, Serializer};
use strum::IntoEnumIterator;

use crate::{
    camera::ptp::hex::{
        FujiClarity, FujiColor, FujiColorChromeEffect, FujiColorChromeFXBlue, FujiColorSpace,
        FujiCustomSetting, FujiCustomSettingName, FujiDynamicRange, FujiDynamicRangePriority,
        FujiFilmSimulation, FujiGrainEffect, FujiHighISONR, FujiHighlightTone, FujiImageQuality,
        FujiImageSize, FujiLensModulationOptimizer, FujiMonochromaticColorTemperature,
        FujiMonochromaticColorTint, FujiShadowTone, FujiSharpness, FujiSmoothSkinEffect,
        FujiWhiteBalance, FujiWhiteBalanceShift, FujiWhiteBalanceTemperature, UsbMode,
    },
    cli::common::suggest::get_closest,
};

#[derive(Args, Debug)]
pub struct FilmSimulationOptions {
    /// Fujifilm Film Simulation
    #[clap(long)]
    pub simulation: Option<FujiFilmSimulation>,

    /// Monochromatic Color Temperature (only applicable to B&W film simulations)
    #[clap(long)]
    pub monochromatic_color_temperature: Option<FujiMonochromaticColorTemperature>,

    /// Monochromatic Color Tint (only applicable to B&W film simulations)
    #[clap(long)]
    pub monochromatic_color_tint: Option<FujiMonochromaticColorTint>,

    /// The output image resolution
    #[clap(long)]
    pub size: Option<FujiImageSize>,

    /// The output image quality (JPEG compression level)
    #[clap(long)]
    pub quality: Option<FujiImageQuality>,

    /// Highlight Tone
    #[clap(long, allow_hyphen_values(true))]
    pub highlight: Option<FujiHighlightTone>,

    /// Shadow Tone
    #[clap(long, allow_hyphen_values(true))]
    pub shadow: Option<FujiShadowTone>,

    /// Color
    #[clap(long, allow_hyphen_values(true))]
    pub color: Option<FujiColor>,

    /// Sharpness
    #[clap(long, allow_hyphen_values(true))]
    pub sharpness: Option<FujiSharpness>,

    /// Clarity
    #[clap(long, allow_hyphen_values(true))]
    pub clarity: Option<FujiClarity>,

    /// White Balance
    #[clap(long)]
    pub white_balance: Option<FujiWhiteBalance>,

    /// White Balance Shift Red
    #[clap(long, allow_hyphen_values(true))]
    pub white_balance_shift_red: Option<FujiWhiteBalanceShift>,

    /// White Balance Shift Blue
    #[clap(long, allow_hyphen_values(true))]
    pub white_balance_shift_blue: Option<FujiWhiteBalanceShift>,

    /// White Balance Temperature (Only used if WB is set to 'Temperature')
    #[clap(long)]
    pub white_balance_temperature: Option<FujiWhiteBalanceTemperature>,

    /// Dynamic Range
    #[clap(long)]
    pub dynamic_range: Option<FujiDynamicRange>,

    /// Dynamic Range Priority
    #[clap(long)]
    pub dynamic_range_priority: Option<FujiDynamicRangePriority>,

    /// High ISO Noise Reduction
    #[clap(long, allow_hyphen_values(true))]
    pub noise_reduction: Option<FujiHighISONR>,

    /// Grain Effect
    #[clap(long)]
    pub grain: Option<FujiGrainEffect>,

    /// Color Chrome Effect
    #[clap(long)]
    pub color_chrome_effect: Option<FujiColorChromeEffect>,

    /// Color Chrome FX Blue
    #[clap(long)]
    pub color_chrome_fx_blue: Option<FujiColorChromeFXBlue>,

    /// Smooth Skin Effect
    #[clap(long)]
    pub smooth_skin_effect: Option<FujiSmoothSkinEffect>,

    /// Lens Modulation Optimizer
    #[clap(long)]
    pub lens_modulation_optimizer: Option<FujiLensModulationOptimizer>,

    /// Color Space
    #[clap(long)]
    pub color_space: Option<FujiColorSpace>,
}

impl fmt::Display for FujiCustomSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::C1 => write!(f, "C1"),
            Self::C2 => write!(f, "C2"),
            Self::C3 => write!(f, "C3"),
            Self::C4 => write!(f, "C4"),
            Self::C5 => write!(f, "C5"),
            Self::C6 => write!(f, "C6"),
            Self::C7 => write!(f, "C7"),
        }
    }
}

impl FromStr for FujiCustomSetting {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase();

        let variant = match input.as_str() {
            "c1" | "1" => Self::C1,
            "c2" | "2" => Self::C2,
            "c3" | "3" => Self::C3,
            "c4" | "4" => Self::C4,
            "c5" | "5" => Self::C5,
            "c6" | "6" => Self::C6,
            "c7" | "7" => Self::C7,
            _ => bail!("Unknown custom setting '{s}'"),
        };

        Ok(variant)
    }
}

impl Serialize for FujiCustomSetting {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16((*self).into())
    }
}

impl fmt::Display for FujiCustomSettingName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &**self)
    }
}

impl FromStr for FujiCustomSettingName {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        if s.len() > Self::MAX_LEN {
            bail!("Value '{}' exceeds max length of {}", s, Self::MAX_LEN);
        }
        Ok(unsafe { Self::new_unchecked(s.to_string()) })
    }
}

impl fmt::Display for FujiImageSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::R7728x5152 => write!(f, "7728x5152"),
            Self::R7728x4344 => write!(f, "7728x4344"),
            Self::R5152x5152 => write!(f, "5152x5152"),
            Self::R6864x5152 => write!(f, "6864x5152"),
            Self::R6432x5152 => write!(f, "6432x5152"),
            Self::R5472x3648 => write!(f, "5472x3648"),
            Self::R5472x3080 => write!(f, "5472x3080"),
            Self::R3648x3648 => write!(f, "3648x3648"),
            Self::R4864x3648 => write!(f, "4864x3648"),
            Self::R4560x3648 => write!(f, "4560x3648"),
            Self::R3888x2592 => write!(f, "3888x2592"),
            Self::R3888x2184 => write!(f, "3888x2184"),
            Self::R2592x2592 => write!(f, "2592x2592"),
            Self::R3456x2592 => write!(f, "3456x2592"),
            Self::R3264x2592 => write!(f, "3264x2592"),
        }
    }
}

impl FromStr for FujiImageSize {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase();

        match input.as_str() {
            "max" | "maximum" | "full" | "largest" => return Ok(Self::R7728x5152),
            _ => {}
        }

        let input = s.replace(' ', "x").replace("by", "x");
        if let Some((w_str, h_str)) = input.split_once('x')
            && let (Ok(w), Ok(h)) = (w_str.trim().parse::<u32>(), h_str.trim().parse::<u32>())
        {
            match (w, h) {
                (7728, 5152) => return Ok(Self::R7728x5152),
                (7728, 4344) => return Ok(Self::R7728x4344),
                (5152, 5152) => return Ok(Self::R5152x5152),
                (6864, 5152) => return Ok(Self::R6864x5152),
                (6432, 5152) => return Ok(Self::R6432x5152),
                (5472, 3648) => return Ok(Self::R5472x3648),
                (5472, 3080) => return Ok(Self::R5472x3080),
                (3648, 3648) => return Ok(Self::R3648x3648),
                (4864, 3648) => return Ok(Self::R4864x3648),
                (4560, 3648) => return Ok(Self::R4560x3648),
                (3888, 2592) => return Ok(Self::R3888x2592),
                (3888, 2184) => return Ok(Self::R3888x2184),
                (2592, 2592) => return Ok(Self::R2592x2592),
                (3456, 2592) => return Ok(Self::R3456x2592),
                (3264, 2592) => return Ok(Self::R3264x2592),
                _ => {}
            }
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown image size '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown image size '{s}'. Expected a resolution (e.g., '5472x3648') or 'maximum'.");
    }
}

impl Serialize for FujiImageSize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl fmt::Display for FujiImageQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FineRaw => write!(f, "Fine + RAW"),
            Self::Fine => write!(f, "Fine"),
            Self::NormalRaw => write!(f, "Normal + RAW"),
            Self::Normal => write!(f, "Normal"),
            Self::Raw => write!(f, "RAW"),
        }
    }
}

impl FromStr for FujiImageQuality {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase().replace(['+', ' '].as_ref(), "");

        match input.as_str() {
            "fineraw" => return Ok(Self::FineRaw),
            "fine" => return Ok(Self::Fine),
            "normalraw" => return Ok(Self::NormalRaw),
            "normal" => return Ok(Self::Normal),
            "raw" => return Ok(Self::Raw),
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown image quality '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown image quality '{s}'");
    }
}

impl fmt::Display for FujiDynamicRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "Auto"),
            Self::HDR100 => write!(f, "HDR100"),
            Self::HDR200 => write!(f, "HDR200"),
            Self::HDR400 => write!(f, "HDR400"),
        }
    }
}

impl FromStr for FujiDynamicRange {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase().replace(['-', ' '].as_ref(), "");

        match input.as_str() {
            "auto" | "hdrauto" | "drauto" => return Ok(Self::Auto),
            "100" | "hdr100" | "dr100" => return Ok(Self::HDR100),
            "200" | "hdr200" | "dr200" => return Ok(Self::HDR200),
            "400" | "hdr400" | "dr400" => return Ok(Self::HDR400),
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown dynamic range '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown dynamic range '{s}'");
    }
}

impl fmt::Display for FujiDynamicRangePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "Auto"),
            Self::Strong => write!(f, "Strong"),
            Self::Weak => write!(f, "Weak"),
            Self::Off => write!(f, "Off"),
        }
    }
}

impl FromStr for FujiDynamicRangePriority {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase().replace(['-', ' '].as_ref(), "");

        match input.as_str() {
            "auto" | "drpauto" => return Ok(Self::Auto),
            "strong" | "drpstrong" => return Ok(Self::Strong),
            "weak" | "drpweak" => return Ok(Self::Weak),
            "off" | "drpoff" => return Ok(Self::Off),
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown dynamic range priority '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown dynamic range priority '{s}'");
    }
}

impl fmt::Display for FujiFilmSimulation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provia => write!(f, "Provia"),
            Self::Velvia => write!(f, "Velvia"),
            Self::Astia => write!(f, "Astia"),
            Self::PRONegHi => write!(f, "PRO Neg. Hi"),
            Self::PRONegStd => write!(f, "PRO Neg. Std"),
            Self::Monochrome => write!(f, "Monochrome"),
            Self::MonochromeYe => write!(f, "Monochrome + Ye"),
            Self::MonochromeR => write!(f, "Monochrome + R"),
            Self::MonochromeG => write!(f, "Monochrome + G"),
            Self::Sepia => write!(f, "Sepia"),
            Self::ClassicChrome => write!(f, "Classic Chrome"),
            Self::AcrosSTD => write!(f, "Acros"),
            Self::AcrosYe => write!(f, "Acros + Ye"),
            Self::AcrosR => write!(f, "Acros + R"),
            Self::AcrosG => write!(f, "Acros + G"),
            Self::Eterna => write!(f, "Eterna"),
            Self::ClassicNegative => write!(f, "Classic Negative"),
            Self::NostalgicNegative => write!(f, "Nostalgic Negative"),
            Self::EternaBleachBypass => write!(f, "Eterna Bleach Bypass"),
            Self::RealaAce => write!(f, "Reala Ace"),
        }
    }
}

impl FromStr for FujiFilmSimulation {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s
            .trim()
            .to_lowercase()
            .replace([' ', '.', '+'].as_ref(), "");

        match input.as_str() {
            "provia" => return Ok(Self::Provia),
            "velvia" => return Ok(Self::Velvia),
            "astia" => return Ok(Self::Astia),
            "proneghi" | "proneghigh" => {
                return Ok(Self::PRONegHi);
            }
            "pronegstd" | "pronegstandard" => {
                return Ok(Self::PRONegStd);
            }
            "mono" | "monochrome" => return Ok(Self::Monochrome),
            "monoy" | "monoye" | "monoyellow" | "monochromey" | "monochromeye"
            | "monochromeyellow" => {
                return Ok(Self::MonochromeYe);
            }
            "monor" | "monored" | "monochromer" | "monochromered" => {
                return Ok(Self::MonochromeR);
            }
            "monog" | "monogreen" | "monochromeg" | "monochromegreen" => {
                return Ok(Self::MonochromeG);
            }
            "sepia" => return Ok(Self::Sepia),
            "classicchrome" => return Ok(Self::ClassicChrome),
            "acros" => return Ok(Self::AcrosSTD),
            "acrosy" | "acrosye" | "acrosyellow" => {
                return Ok(Self::AcrosYe);
            }
            "acrossr" | "acrossred" => {
                return Ok(Self::AcrosR);
            }
            "acrossg" | "acrossgreen" => {
                return Ok(Self::AcrosG);
            }
            "eterna" => return Ok(Self::Eterna),
            "classicneg" | "classicnegative" => {
                return Ok(Self::ClassicNegative);
            }
            "nostalgicneg" | "nostalgicnegative" => {
                return Ok(Self::NostalgicNegative);
            }
            "eternabb" | "eternableach" | "eternableachbypass" => {
                return Ok(Self::EternaBleachBypass);
            }
            "realaace" => {
                return Ok(Self::RealaAce);
            }
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown value '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown value '{input}'");
    }
}

impl fmt::Display for FujiGrainEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StrongLarge => write!(f, "Strong Large"),
            Self::WeakLarge => write!(f, "Weak Large"),
            Self::StrongSmall => write!(f, "Strong Small"),
            Self::WeakSmall => write!(f, "Weak Small"),
            Self::Off => write!(f, "Off"),
        }
    }
}

impl FromStr for FujiGrainEffect {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s
            .trim()
            .to_lowercase()
            .replace(['+', '-', ',', ' '].as_ref(), "");

        match input.as_str() {
            "stronglarge" | "largestrong" => return Ok(Self::StrongLarge),
            "weaklarge" | "largeweak" => return Ok(Self::WeakLarge),
            "strongsmall" | "smallstrong" => return Ok(Self::StrongSmall),
            "weaksmall" | "smallweak" => return Ok(Self::WeakSmall),
            "off" => return Ok(Self::Off),
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(&input, &choices) {
            bail!("Unknown grain effect '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown grain effect '{s}'");
    }
}

impl fmt::Display for FujiColorChromeEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strong => write!(f, "Strong"),
            Self::Weak => write!(f, "Weak"),
            Self::Off => write!(f, "Off"),
        }
    }
}

impl FromStr for FujiColorChromeEffect {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase();

        match input.as_str() {
            "strong" => return Ok(Self::Strong),
            "weak" => return Ok(Self::Weak),
            "off" => return Ok(Self::Off),
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown color chrome effect '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown color chrome effect '{s}'");
    }
}

impl fmt::Display for FujiColorChromeFXBlue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strong => write!(f, "Strong"),
            Self::Weak => write!(f, "Weak"),
            Self::Off => write!(f, "Off"),
        }
    }
}

impl FromStr for FujiColorChromeFXBlue {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase();

        match input.as_str() {
            "strong" => return Ok(Self::Strong),
            "weak" => return Ok(Self::Weak),
            "off" => return Ok(Self::Off),
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown color chrome fx blue '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown color chrome fx blue '{s}'");
    }
}

impl fmt::Display for FujiSmoothSkinEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strong => write!(f, "Strong"),
            Self::Weak => write!(f, "Weak"),
            Self::Off => write!(f, "Off"),
        }
    }
}

impl FromStr for FujiSmoothSkinEffect {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase();

        match input.as_str() {
            "strong" => return Ok(Self::Strong),
            "weak" => return Ok(Self::Weak),
            "off" => return Ok(Self::Off),
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown smooth skin effect '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown smooth skin effect '{s}'");
    }
}

impl fmt::Display for FujiWhiteBalance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AsShot => write!(f, "As Shot"),
            Self::WhitePriority => write!(f, "White Priority"),
            Self::Auto => write!(f, "Auto"),
            Self::AmbiencePriority => write!(f, "Ambience Priority"),
            Self::Custom1 => write!(f, "Custom 1"),
            Self::Custom2 => write!(f, "Custom 2"),
            Self::Custom3 => write!(f, "Custom 3"),
            Self::Temperature => write!(f, "Temperature"),
            Self::Daylight => write!(f, "Daylight"),
            Self::Shade => write!(f, "Shade"),
            Self::Fluorescent1 => write!(f, "Fluorescent 1"),
            Self::Fluorescent2 => write!(f, "Fluorescent 2"),
            Self::Fluorescent3 => write!(f, "Fluorescent 3"),
            Self::Incandescent => write!(f, "Incandescent"),
            Self::Underwater => write!(f, "Underwater"),
        }
    }
}

impl FromStr for FujiWhiteBalance {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase().replace(['-', ' '].as_ref(), "");

        match input.as_str() {
            "whitepriority" | "white" => return Ok(Self::WhitePriority),
            // We can't set a film simulation to be "As Shot", so silently parse it to Auto
            "auto" | "shot" | "asshot" | "original" => return Ok(Self::Auto),
            "ambiencepriority" | "ambience" | "ambient" => {
                return Ok(Self::AmbiencePriority);
            }
            "custom1" | "c1" => return Ok(Self::Custom1),
            "custom2" | "c2" => return Ok(Self::Custom2),
            "custom3" | "c3" => return Ok(Self::Custom3),
            "temperature" | "k" | "kelvin" => return Ok(Self::Temperature),
            "daylight" | "sunny" => return Ok(Self::Daylight),
            "shade" | "cloudy" => return Ok(Self::Shade),
            "fluorescent1" => {
                return Ok(Self::Fluorescent1);
            }
            "fluorescent2" => {
                return Ok(Self::Fluorescent2);
            }
            "fluorescent3" => {
                return Ok(Self::Fluorescent3);
            }
            "incandescent" | "tungsten" => return Ok(Self::Incandescent),
            "underwater" => return Ok(Self::Underwater),
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown white balance '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown white balance '{s}'");
    }
}

impl fmt::Display for FujiHighISONR {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plus4 => write!(f, "+4"),
            Self::Plus3 => write!(f, "+3"),
            Self::Plus2 => write!(f, "+2"),
            Self::Plus1 => write!(f, "+1"),
            Self::Zero => write!(f, "0"),
            Self::Minus1 => write!(f, "-1"),
            Self::Minus2 => write!(f, "-2"),
            Self::Minus3 => write!(f, "-3"),
            Self::Minus4 => write!(f, "-4"),
        }
    }
}

impl FromStr for FujiHighISONR {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s
            .trim()
            .parse::<i16>()
            .with_context(|| format!("Invalid numeric value '{s}'"))?;

        match input {
            4 => Ok(Self::Plus4),
            3 => Ok(Self::Plus3),
            2 => Ok(Self::Plus2),
            1 => Ok(Self::Plus1),
            0 => Ok(Self::Zero),
            -1 => Ok(Self::Minus1),
            -2 => Ok(Self::Minus2),
            -3 => Ok(Self::Minus3),
            -4 => Ok(Self::Minus4),
            _ => bail!("Value {input} is out of range",),
        }
    }
}

impl Serialize for FujiHighISONR {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Plus4 => serializer.serialize_i16(4),
            Self::Plus3 => serializer.serialize_i16(3),
            Self::Plus2 => serializer.serialize_i16(2),
            Self::Plus1 => serializer.serialize_i16(1),
            Self::Zero => serializer.serialize_i16(0),
            Self::Minus1 => serializer.serialize_i16(-1),
            Self::Minus2 => serializer.serialize_i16(-2),
            Self::Minus3 => serializer.serialize_i16(-3),
            Self::Minus4 => serializer.serialize_i16(-4),
        }
    }
}

impl fmt::Display for FujiLensModulationOptimizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::On => write!(f, "On"),
        }
    }
}

impl FromStr for FujiLensModulationOptimizer {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase();

        match input.as_str() {
            "off" | "false" => return Ok(Self::Off),
            "on" | "true" => return Ok(Self::On),
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown lens modulation optimizer '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown lens modulation optimizer '{s}'");
    }
}

impl fmt::Display for FujiColorSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SRGB => write!(f, "sRGB"),
            Self::AdobeRGB => write!(f, "Adobe RGB"),
        }
    }
}

impl FromStr for FujiColorSpace {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let input = s.trim().to_lowercase();

        match input.as_str() {
            "s" | "srgb" => return Ok(Self::SRGB),
            "adobe" | "adobergb" => return Ok(Self::AdobeRGB),
            _ => {}
        }

        let choices: Vec<String> = Self::iter().map(|v| v.to_string()).collect();
        if let Some(best) = get_closest(s, &choices) {
            bail!("Unknown color space '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown color space '{s}'");
    }
}

macro_rules! fuji_i16_cli {
    ($name:ident) => {
        impl std::str::FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> anyhow::Result<Self> {
                use anyhow::Context;

                let input = s
                    .trim()
                    .parse::<f32>()
                    .with_context(|| format!("Invalid numeric value '{s}'"))?;

                if !(Self::MIN..=Self::MAX).contains(&input) {
                    anyhow::bail!("Value {} is out of range", input);
                }
                #[allow(clippy::modulo_one)]
                if (input - Self::MIN) % Self::STEP != 0.0 {
                    anyhow::bail!("Value {} is not aligned to step {}", input, Self::STEP);
                }

                #[allow(clippy::cast_possible_truncation)]
                let raw = (input * Self::SCALE).round() as i16;

                unsafe { Ok(Self::new_unchecked(raw)) }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let val = f32::from(*self.deref()) / Self::SCALE;
                serializer.serialize_f32(val)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let value = (f32::from(*self.deref()) / Self::SCALE);
                write!(f, "{}", value)
            }
        }
    };
}

fuji_i16_cli!(FujiMonochromaticColorTemperature);
fuji_i16_cli!(FujiMonochromaticColorTint);
fuji_i16_cli!(FujiWhiteBalanceShift);
fuji_i16_cli!(FujiWhiteBalanceTemperature);
fuji_i16_cli!(FujiHighlightTone);
fuji_i16_cli!(FujiShadowTone);
fuji_i16_cli!(FujiColor);
fuji_i16_cli!(FujiSharpness);
fuji_i16_cli!(FujiClarity);

impl fmt::Display for UsbMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::RawConversion => "USB RAW CONV./BACKUP RESTORE",
        };
        write!(f, "{s}")
    }
}
