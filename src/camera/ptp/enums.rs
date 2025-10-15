use std::fmt;

use anyhow::bail;
use serde::Serialize;

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

impl TryFrom<u16> for CommandCode {
    type Error = anyhow::Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x1001 => Ok(Self::GetDeviceInfo),
            0x1002 => Ok(Self::OpenSession),
            0x1003 => Ok(Self::CloseSession),
            0x1008 => Ok(Self::GetObjectInfo),
            0x1009 => Ok(Self::GetObject),
            0x100C => Ok(Self::SendObjectInfo),
            0x100D => Ok(Self::SendObject),
            0x1015 => Ok(Self::GetDevicePropValue),
            0x1016 => Ok(Self::SetDevicePropValue),
            v => bail!("Unknown command code '{v}'"),
        }
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl std::convert::TryFrom<u16> for ResponseCode {
    type Error = anyhow::Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x2000 => Ok(Self::Undefined),
            0x2001 => Ok(Self::Ok),
            0x2002 => Ok(Self::GeneralError),
            0x2003 => Ok(Self::SessionNotOpen),
            0x2004 => Ok(Self::InvalidTransactionId),
            0x2005 => Ok(Self::OperationNotSupported),
            0x2006 => Ok(Self::ParameterNotSupported),
            0x2007 => Ok(Self::IncompleteTransfer),
            0x2008 => Ok(Self::InvalidStorageId),
            0x2009 => Ok(Self::InvalidObjectHandle),
            0x200A => Ok(Self::DevicePropNotSupported),
            0x200B => Ok(Self::InvalidObjectFormatCode),
            0x200C => Ok(Self::StoreFull),
            0x200D => Ok(Self::ObjectWriteProtected),
            0x200E => Ok(Self::StoreReadOnly),
            0x200F => Ok(Self::AccessDenied),
            0x2010 => Ok(Self::NoThumbnailPresent),
            0x2011 => Ok(Self::SelfTestFailed),
            0x2012 => Ok(Self::PartialDeletion),
            0x2013 => Ok(Self::StoreNotAvailable),
            0x2014 => Ok(Self::SpecificationByFormatUnsupported),
            0x2015 => Ok(Self::NoValidObjectInfo),
            0x2016 => Ok(Self::InvalidCodeFormat),
            0x2017 => Ok(Self::UnknownVendorCode),
            0x2018 => Ok(Self::CaptureAlreadyTerminated),
            0x2019 => Ok(Self::DeviceBusy),
            0x201A => Ok(Self::InvalidParentObject),
            0x201B => Ok(Self::InvalidDevicePropFormat),
            0x201C => Ok(Self::InvalidDevicePropValue),
            0x201D => Ok(Self::InvalidParameter),
            0x201E => Ok(Self::SessionAlreadyOpen),
            0x201F => Ok(Self::TransactionCancelled),
            0x2020 => Ok(Self::SpecificationOfDestinationUnsupported),
            v => bail!("Unknown response code '{v}'"),
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropCode {
    FujiUsbMode = 0xd16e,
    FujiBatteryInfo2 = 0xD36B,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum UsbMode {
    RawConversion,
    Unsupported,
}

impl From<u32> for UsbMode {
    fn from(val: u32) -> Self {
        match val {
            6 => Self::RawConversion,
            _ => Self::Unsupported,
        }
    }
}

impl fmt::Display for UsbMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::RawConversion => "USB RAW CONV./BACKUP RESTORE",
            Self::Unsupported => "Unsupported USB Mode",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ContainerType {
    Command = 1,
    Data = 2,
    Response = 3,
    Event = 4,
}

impl TryFrom<u16> for ContainerType {
    type Error = anyhow::Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Command),
            2 => Ok(Self::Data),
            3 => Ok(Self::Response),
            4 => Ok(Self::Event),
            v => bail!("Invalid message type '{v}'"),
        }
    }
}
