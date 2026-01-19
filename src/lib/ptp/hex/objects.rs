use num_enum::{IntoPrimitive, TryFromPrimitive};
use ptp_macro::{PtpDeserialize, PtpSerialize};

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
    Default,
)]
pub enum ObjectFormat {
    #[default]
    None = 0x0,
    FujiBackup = 0x5000,
    FujiRAF = 0xf802,
}
