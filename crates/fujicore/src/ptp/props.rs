use std::fmt;

use num_enum::{IntoPrimitive, TryFromPrimitive};

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
pub enum DevicePropCode {
    FujiRawConversionRun = 0xD183,
    FujiRawConversionProfileCode = 0xD184,
    FujiRawConversionProfile = 0xD185,
    FujiRawConversionConditionCode = 0xD186,
    FujiRawConversionCompatibilityCode = 0xD187,
    FujiBatteryInfo2 = 0xD36B,
}

impl fmt::Display for DevicePropCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?} (0x{:04x})", u16::from(*self))
    }
}
