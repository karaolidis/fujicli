use std::io::{self, Cursor};

use anyhow::bail;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ptp_cursor::{PtpDeserialize, PtpSerialize, Read};
use ptp_macro::{PtpDeserialize, PtpSerialize};

#[repr(u16)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, PtpSerialize, PtpDeserialize,
)]
pub enum CommandCode {
    GetDeviceInfo = 0x1001,
    OpenSession = 0x1002,
    CloseSession = 0x1003,
    GetObjectHandles = 0x1007,
    GetObjectInfo = 0x1008,
    GetObject = 0x1009,
    DeleteObject = 0x100B,
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
