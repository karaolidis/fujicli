use std::{
    fmt,
    io::{self, Cursor},
    ops::{Deref, DerefMut},
    str::FromStr,
};

use anyhow::{Context, bail};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ptp_cursor::{PtpDeserialize, PtpSerialize, Read};
use ptp_macro::{PtpDeserialize, PtpSerialize};
use serde::{Serialize, Serializer};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[repr(u16)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, PtpSerialize, PtpDeserialize,
)]
pub enum CommandCode {
    GetDeviceInfo = 0x1001,
    OpenSession = 0x1002,
    CloseSession = 0x1003,
    GetObjectInfo = 0x1008,
    GetObject = 0x1009,
    SendObjectInfo = 0x100C,
    SendObject = 0x100D,
    GetDevicePropValue = 0x1015,
    SetDevicePropValue = 0x1016,
    FujiSendObjectInfo = 0x900c,
    FujiSendObject = 0x900d,
}

#[repr(u16)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, PtpSerialize, PtpDeserialize,
)]
pub enum ResponseCode {
    Undefined = 0x2000,
    Ok = 0x2001,
    GeneralError = 0x2002,
    SessionNotOpen = 0x2003,
    InvalidTransactionId = 0x2004,
    OperationNotSupported = 0x2005,
    ParameterNotSupported = 0x2006,
    IncompleteTransfer = 0x2007,
    InvalidStorageId = 0x2008,
    InvalidObjectHandle = 0x2009,
    DevicePropNotSupported = 0x200A,
    InvalidObjectFormatCode = 0x200B,
    StoreFull = 0x200C,
    ObjectWriteProtected = 0x200D,
    StoreReadOnly = 0x200E,
    AccessDenied = 0x200F,
    NoThumbnailPresent = 0x2010,
    SelfTestFailed = 0x2011,
    PartialDeletion = 0x2012,
    StoreNotAvailable = 0x2013,
    SpecificationByFormatUnsupported = 0x2014,
    NoValidObjectInfo = 0x2015,
    InvalidCodeFormat = 0x2016,
    UnknownVendorCode = 0x2017,
    CaptureAlreadyTerminated = 0x2018,
    DeviceBusy = 0x2019,
    InvalidParentObject = 0x201A,
    InvalidDevicePropFormat = 0x201B,
    InvalidDevicePropValue = 0x201C,
    InvalidParameter = 0x201D,
    SessionAlreadyOpen = 0x201E,
    TransactionCancelled = 0x201F,
    SpecificationOfDestinationUnsupported = 0x2020,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerCode {
    Command(CommandCode),
    Response(ResponseCode),
}

impl From<ContainerCode> for u16 {
    fn from(code: ContainerCode) -> Self {
        match code {
            ContainerCode::Command(cmd) => cmd.into(),
            ContainerCode::Response(resp) => resp.into(),
        }
    }
}

impl TryFrom<u16> for ContainerCode {
    type Error = anyhow::Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if let Ok(cmd) = CommandCode::try_from(value) {
            return Ok(Self::Command(cmd));
        }

        if let Ok(resp) = ResponseCode::try_from(value) {
            return Ok(Self::Response(resp));
        }

        bail!("Unknown container code '{value:x?}'");
    }
}

impl PtpSerialize for ContainerCode {
    fn try_into_ptp(&self) -> io::Result<Vec<u8>> {
        let value: u16 = (*self).into();
        value.try_into_ptp()
    }

    fn try_write_ptp(&self, buf: &mut Vec<u8>) -> io::Result<()> {
        let value: u16 = (*self).into();
        value.try_write_ptp(buf)
    }
}

impl PtpDeserialize for ContainerCode {
    fn try_from_ptp(buf: &[u8]) -> io::Result<Self> {
        let mut cur = Cursor::new(buf);
        let value = Self::try_read_ptp(&mut cur)?;
        cur.expect_end()?;
        io::Result::Ok(value)
    }

    fn try_read_ptp<R: ptp_cursor::Read>(cur: &mut R) -> io::Result<Self> {
        let value = <u16>::try_read_ptp(cur)?;
        Self::try_from(value)
            .map_err(|e: anyhow::Error| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

#[repr(u32)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, PtpSerialize, PtpDeserialize,
)]
pub enum DevicePropCode {
    FujiUsbMode = 0xd16e,
    FujiRawConversionRun = 0xD183,
    FujiRawConversionProfile = 0xD185,
    FujiCustomSetting = 0xD18C,
    FujiCustomSettingName = 0xD18D,
    FujiCustomSettingImageSize = 0xD18E,
    FujiCustomSettingImageQuality = 0xD18F,
    FujiCustomSettingDynamicRange = 0xD190,
    FujiCustomSettingDynamicRangePriority = 0xD191,
    FujiCustomSettingFilmSimulation = 0xD192,
    FujiCustomSettingMonochromaticColorTemperature = 0xD193,
    FujiCustomSettingMonochromaticColorTint = 0xD194,
    FujiCustomSettingGrainEffect = 0xD195,
    FujiCustomSettingColorChromeEffect = 0xD196,
    FujiCustomSettingColorChromeFXBlue = 0xD197,
    FujiCustomSettingSmoothSkinEffect = 0xD198,
    FujiCustomSettingWhiteBalance = 0xD199,
    FujiCustomSettingWhiteBalanceShiftRed = 0xD19A,
    FujiCustomSettingWhiteBalanceShiftBlue = 0xD19B,
    FujiCustomSettingWhiteBalanceTemperature = 0xD19C,
    FujiCustomSettingHighlightTone = 0xD19D,
    FujiCustomSettingShadowTone = 0xD19E,
    FujiCustomSettingColor = 0xD19F,
    FujiCustomSettingSharpness = 0xD1A0,
    FujiCustomSettingHighISONR = 0xD1A1,
    FujiCustomSettingClarity = 0xD1A2,
    FujiCustomSettingLensModulationOptimizer = 0xD1A3,
    FujiCustomSettingColorSpace = 0xD1A4,
    // TODO: 0xD1A5 All 7s
    FujiBatteryInfo2 = 0xD36B,
}

const SIMILARITY_THRESHOLD: usize = 8;

fn suggest_closest<'a, I, S>(input: &str, choices: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a S>,
    S: AsRef<str> + 'a,
{
    let mut best_score = usize::MAX;
    let mut best_match: Option<&'a str> = None;

    for choice in choices {
        let choice_str = choice.as_ref();
        let dist = strsim::damerau_levenshtein(&input.to_lowercase(), &choice_str.to_lowercase());

        if dist < best_score {
            best_score = dist;
            best_match = Some(choice_str);
        }
    }

    println!("{best_score}");
    if best_score <= SIMILARITY_THRESHOLD {
        best_match
    } else {
        None
    }
}

#[repr(u16)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, PtpSerialize, PtpDeserialize,
)]
pub enum ObjectFormat {
    None = 0x0,
    FujiBackup = 0x5000,
    FujiRAF = 0xf802,
}

impl Default for ObjectFormat {
    fn default() -> Self {
        Self::None
    }
}

#[repr(u16)]
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
pub enum FujiCustomSetting {
    C1 = 0x1,
    C2 = 0x2,
    C3 = 0x3,
    C4 = 0x4,
    C5 = 0x5,
    C6 = 0x6,
    C7 = 0x7,
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

impl Serialize for FujiCustomSetting {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16((*self).into())
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, PtpSerialize, PtpDeserialize)]
pub struct FujiCustomSettingName(String);

impl FujiCustomSettingName {
    pub const MAX_LEN: usize = 25;
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

impl TryFrom<String> for FujiCustomSettingName {
    type Error = anyhow::Error;

    fn try_from(value: String) -> anyhow::Result<Self> {
        if value.len() > Self::MAX_LEN {
            bail!("Value '{}' exceeds max length of {}", value, Self::MAX_LEN);
        }
        Ok(Self(value))
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

impl std::fmt::Display for FujiCustomSettingName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[repr(u16)]
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
pub enum FujiImageSize {
    R7728x5152 = 0x7,
    R7728x4344 = 0x8,
    R5152x5152 = 0x9,
    R6864x5152 = 0xe,
    R6432x5152 = 0x10,
    R5472x3648 = 0x4,
    R5472x3080 = 0x5,
    R3648x3648 = 0x6,
    R4864x3648 = 0x12,
    R4560x3648 = 0x14,
    R3888x2592 = 0x1,
    R3888x2184 = 0x2,
    R2592x2592 = 0x3,
    R3456x2592 = 0xa,
    R3264x2592 = 0xc,
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

impl Serialize for FujiImageSize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown image size '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown image size '{s}'. Expected a resolution (e.g., '5472x3648') or 'maximum'.");
    }
}

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiImageQuality {
    FineRaw = 0x4,
    Fine = 0x2,
    NormalRaw = 0x5,
    Normal = 0x3,
    Raw = 0x1,
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown image quality '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown image quality '{s}'");
    }
}

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiDynamicRange {
    Auto = 0xffff,
    HDR100 = 0x64,
    HDR200 = 0xc8,
    HDR400 = 0x190,
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown dynamic range '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown dynamic range '{s}'");
    }
}

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiDynamicRangePriority {
    Auto = 0x8000,
    Strong = 0x2,
    Weak = 0x1,
    Off = 0x0,
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown dynamic range priority '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown dynamic range priority '{s}'");
    }
}

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiFilmSimulation {
    Provia = 0x1,
    Velvia = 0x2,
    Astia = 0x3,
    PRONegHi = 0x4,
    PRONegStd = 0x5,
    Monochrome = 0x6,
    MonochromeYe = 0x7,
    MonochromeR = 0x8,
    MonochromeG = 0x9,
    Sepia = 0xa,
    ClassicChrome = 0xb,
    AcrosSTD = 0xc,
    AcrosYe = 0xd,
    AcrosR = 0xe,
    AcrosG = 0xf,
    Eterna = 0x10,
    ClassicNegative = 0x11,
    NostalgicNegative = 0x13,
    EternaBleachBypass = 0x12,
    RealaAce = 0x14,
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown value '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown value '{input}'");
    }
}

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiGrainEffect {
    StrongLarge = 0x5,
    WeakLarge = 0x4,
    StrongSmall = 0x3,
    WeakSmall = 0x2,
    Off = 0x6,
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
        if let Some(best) = suggest_closest(&input, &choices) {
            bail!("Unknown grain effect '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown grain effect '{s}'");
    }
}

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiColorChromeEffect {
    Strong = 0x3,
    Weak = 0x2,
    Off = 0x1,
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown color chrome effect '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown color chrome effect '{s}'");
    }
}

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiColorChromeFXBlue {
    Strong = 0x3,
    Weak = 0x2,
    Off = 0x1,
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown color chrome fx blue '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown color chrome fx blue '{s}'");
    }
}

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiSmoothSkinEffect {
    Strong = 0x3,
    Weak = 0x2,
    Off = 0x1,
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown smooth skin effect '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown smooth skin effect '{s}'");
    }
}

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiWhiteBalance {
    WhitePriority = 0x8020,
    Auto = 0x2,
    AmbiencePriority = 0x8021,
    Custom1 = 0x8008,
    Custom2 = 0x8009,
    Custom3 = 0x800A,
    Temperature = 0x8007,
    Daylight = 0x4,
    Shade = 0x8006,
    Fluorescent1 = 0x8001,
    Fluorescent2 = 0x8002,
    Fluorescent3 = 0x8003,
    Incandescent = 0x6,
    Underwater = 0x8,
}

impl fmt::Display for FujiWhiteBalance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            "auto" => return Ok(Self::Auto),
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown white balance '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown white balance '{s}'");
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

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiLensModulationOptimizer {
    Off = 0x2,
    On = 0x1,
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown lens modulation optimizer '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown lens modulation optimizer '{s}'");
    }
}

#[repr(u16)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    IntoPrimitive,
    TryFromPrimitive,
    PtpSerialize,
    PtpDeserialize,
    EnumIter,
)]
pub enum FujiColorSpace {
    SRGB = 0x2,
    AdobeRGB = 0x1,
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
        if let Some(best) = suggest_closest(s, &choices) {
            bail!("Unknown color space '{s}'. Did you mean '{best}'?");
        }

        bail!("Unknown color space '{s}'");
    }
}

macro_rules! fuji_i16 {
    ($name:ident, $min:expr, $max:expr, $step:expr, $scale:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PtpSerialize, PtpDeserialize)]
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

            pub const unsafe fn new_unchecked(value: i16) -> Self {
                Self(value)
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

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let value = (f32::from(self.0) / Self::SCALE);
                write!(f, "{}", value)
            }
        }

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
                let val = f32::from(self.0) / Self::SCALE;
                serializer.serialize_f32(val)
            }
        }
    };
}

fuji_i16!(FujiMonochromaticColorTemperature, -18.0, 18.0, 1.0, 10i16);
fuji_i16!(FujiMonochromaticColorTint, -18.0, 18.0, 1.0, 10i16);
fuji_i16!(FujiWhiteBalanceShift, -9.0, 9.0, 1.0, 1i16);
fuji_i16!(FujiWhiteBalanceTemperature, 2500.0, 10000.0, 10.0, 1i16);
fuji_i16!(FujiHighlightTone, -2.0, 4.0, 0.5, 10i16);
fuji_i16!(FujiShadowTone, -2.0, 4.0, 0.5, 10i16);
fuji_i16!(FujiColor, -4.0, 4.0, 1.0, 10i16);
fuji_i16!(FujiSharpness, -4.0, 4.0, 1.0, 10i16);
fuji_i16!(FujiClarity, -5.0, 5.0, 1.0, 10i16);

#[repr(u16)]
#[derive(
    Debug, Clone, Copy, Serialize, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, PtpDeserialize,
)]
pub enum UsbMode {
    RawConversion = 0x6,
}

impl fmt::Display for UsbMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::RawConversion => "USB RAW CONV./BACKUP RESTORE",
        };
        write!(f, "{s}")
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, PtpSerialize, PtpDeserialize,
)]
#[repr(u16)]
pub enum ContainerType {
    Command = 1,
    Data = 2,
    Response = 3,
    Event = 4,
}
