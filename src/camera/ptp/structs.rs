use std::io::Cursor;

use byteorder::{LittleEndian, ReadBytesExt};

use super::{enums::ContainerType, read::Read};

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
    pub kind: ContainerType,
    pub payload_len: u32,
    pub code: u16,
}

impl ContainerInfo {
    pub const SIZE: usize =
        size_of::<ContainerType>() + size_of::<u32>() + size_of::<u16>() + size_of::<u32>();
}

impl ContainerInfo {
    pub fn parse<R: ReadBytesExt>(mut r: R) -> anyhow::Result<Self> {
        let payload_len = r.read_u32::<LittleEndian>()? - Self::SIZE as u32;
        let kind = r.read_u16::<LittleEndian>()?;
        let kind = ContainerType::try_from(kind)?;
        let code = r.read_u16::<LittleEndian>()?;
        let _transaction_id = r.read_u32::<LittleEndian>()?;

        Ok(Self {
            kind,
            payload_len,
            code,
        })
    }
}
