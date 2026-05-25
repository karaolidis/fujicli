use num_enum::{IntoPrimitive, TryFromPrimitive};

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
pub enum DevicePropCode {
    FujiRawConversionRun = 0xD183,
    FujiRawConversionProfile = 0xD185,
    FujiBatteryInfo2 = 0xD36B,
}
