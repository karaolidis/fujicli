use ptp_macro::{PtpDeserialize, PtpSerialize};

use super::hex::{CommandCode, ContainerCode, ContainerType};

#[allow(dead_code)]
#[derive(Debug, PtpSerialize, PtpDeserialize)]
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

#[derive(Debug, Clone, Copy, PtpSerialize, PtpDeserialize)]
pub struct ContainerInfo {
    pub total_len: u32,
    pub kind: ContainerType,
    pub code: ContainerCode,
    pub transaction_id: u32,
}

impl ContainerInfo {
    pub const SIZE: usize =
        size_of::<u32>() + size_of::<u16>() + size_of::<u16>() + size_of::<u32>();

    pub fn new(
        kind: ContainerType,
        code: CommandCode,
        transaction_id: u32,
        payload_len: usize,
    ) -> anyhow::Result<Self> {
        let total_len = u32::try_from(Self::SIZE + payload_len)?;
        let code = ContainerCode::Command(code);

        Ok(Self {
            total_len,
            kind,
            code,
            transaction_id,
        })
    }

    pub const fn payload_len(&self) -> usize {
        self.total_len as usize - Self::SIZE
    }
}
