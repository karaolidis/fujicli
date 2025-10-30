use ptp_macro::{PtpDeserialize, PtpSerialize};

use super::hex::{CommandCode, ContainerCode, ContainerType, ObjectFormat};

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

#[derive(Debug, Clone, Default, PtpSerialize, PtpDeserialize)]
pub struct ObjectInfo {
    pub storage_id: u32,
    pub object_format: ObjectFormat,
    pub protection_status: u16,
    pub compressed_size: u32,
    pub thumb_format: u16,
    pub thumb_compressed_size: u32,
    pub thumb_width: u32,
    pub thumb_height: u32,
    pub image_width: u32,
    pub image_height: u32,
    pub image_bit_depth: u32,
    pub parent_object: u32,
    pub association_type: u16,
    pub association_desc: u32,
    pub sequence_number: u32,
    pub filename: String,
    pub date_created: String,
    pub date_modified: String,
    pub keywords: String,
}
