use std::{
    io::{self, Cursor},
    ops::{Deref, DerefMut},
};

use anyhow::bail;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ptp_cursor::{PtpDeserialize, PtpSerialize, Read};
use ptp_macro::{PtpDeserialize, PtpSerialize};
use serde::Serialize;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, PtpSerialize, PtpDeserialize)]
pub struct FujiCustomSettingName(String);

impl FujiCustomSettingName {
    pub const MAX_LEN: usize = 25;

    pub const unsafe fn new_unchecked(value: String) -> Self {
        Self(value)
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

impl TryFrom<String> for FujiCustomSettingName {
    type Error = anyhow::Error;

    fn try_from(value: String) -> anyhow::Result<Self> {
        if value.len() > Self::MAX_LEN {
            bail!("Value '{}' exceeds max length of {}", value, Self::MAX_LEN);
        }
        Ok(Self(value))
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
    AsShot = 0x1,
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
#[allow(clippy::upper_case_acronyms)]
pub enum FujiColorSpace {
    SRGB = 0x2,
    AdobeRGB = 0x1,
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
