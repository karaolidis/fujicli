use std::io::Cursor;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use super::{
    enums::{CommandCode, ContainerCode, ContainerType},
    read::Read,
};

#[allow(dead_code)]
#[derive(Debug)]
pub struct DeviceInfo {
    pub version: u16,
    pub vendor_ex_id: u32,
    pub vendor_ex_version: u16,
    pub vendor_extension_desc: String,
    pub functional_mode: u16,
    pub operations_supported: Vec<u16>,
    pub events_supported: Vec<u16>,
    pub device_properties_supported: Vec<u16>,
    pub capture_formats: Vec<u16>,
    pub image_formats: Vec<u16>,
    pub manufacturer: String,
    pub model: String,
    pub device_version: String,
    pub serial_number: String,
}

impl TryFrom<&[u8]> for DeviceInfo {
    type Error = anyhow::Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let mut cur = Cursor::new(buf);

        Ok(Self {
            version: cur.read_ptp_u16()?,
            vendor_ex_id: cur.read_ptp_u32()?,
            vendor_ex_version: cur.read_ptp_u16()?,
            vendor_extension_desc: cur.read_ptp_str()?,
            functional_mode: cur.read_ptp_u16()?,
            operations_supported: cur.read_ptp_u16_vec()?,
            events_supported: cur.read_ptp_u16_vec()?,
            device_properties_supported: cur.read_ptp_u16_vec()?,
            capture_formats: cur.read_ptp_u16_vec()?,
            image_formats: cur.read_ptp_u16_vec()?,
            manufacturer: cur.read_ptp_str()?,
            model: cur.read_ptp_str()?,
            device_version: cur.read_ptp_str()?,
            serial_number: cur.read_ptp_str()?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ContainerInfo {
    pub total_len: u32,
    pub kind: ContainerType,
    pub code: ContainerCode,
    pub transaction_id: Option<u32>,
}

impl ContainerInfo {
    const BASE_SIZE: usize = size_of::<u32>() + size_of::<u16>() + size_of::<u16>();
    pub const SIZE: usize = Self::BASE_SIZE + size_of::<u32>();

    pub fn new(
        kind: ContainerType,
        code: CommandCode,
        transaction_id: Option<u32>,
        payload: &[u8],
    ) -> anyhow::Result<Self> {
        let mut total_len = if transaction_id.is_some() {
            Self::SIZE
        } else {
            Self::BASE_SIZE
        };
        total_len += payload.len();

        Ok(Self {
            total_len: u32::try_from(total_len)?,
            kind,
            code: ContainerCode::Command(code),
            transaction_id,
        })
    }

    pub const fn len(&self) -> usize {
        if self.transaction_id.is_some() {
            Self::SIZE
        } else {
            Self::BASE_SIZE
        }
    }

    pub const fn payload_len(&self) -> usize {
        self.total_len as usize - self.len()
    }
}

impl TryFrom<&[u8]> for ContainerInfo {
    type Error = anyhow::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let mut r = Cursor::new(bytes);

        let total_len = r.read_u32::<LittleEndian>()?;
        let kind = ContainerType::try_from(r.read_u16::<LittleEndian>()?)?;
        let code = ContainerCode::try_from(r.read_u16::<LittleEndian>()?)?;
        let transaction_id = Some(r.read_u32::<LittleEndian>()?);

        Ok(Self {
            total_len,
            kind,
            code,
            transaction_id,
        })
    }
}

impl TryFrom<ContainerInfo> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(val: ContainerInfo) -> Result<Self, Self::Error> {
        let mut buf = Self::with_capacity(val.len());
        buf.write_u32::<LittleEndian>(val.total_len)?;
        buf.write_u16::<LittleEndian>(val.kind as u16)?;
        buf.write_u16::<LittleEndian>(val.code.into())?;
        if let Some(transaction_id) = val.transaction_id {
            buf.write_u32::<LittleEndian>(transaction_id)?;
        }
        Ok(buf)
    }
}
