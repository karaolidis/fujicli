use num_enum::{IntoPrimitive, TryFromPrimitive};
use ptp_macro::{PtpDeserialize, PtpSerialize};

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
