use std::io::{self, Write};

use ptp_cursor::PtpSerialize;

use crate::ptp::{hex::ObjectFormat, structs::ObjectInfo};

// NOTE: Naively assuming that all cameras support backup/restore
// using the same structs.
pub struct FujiBackupObjectInfo {
    compressed_size: u32,
}

impl FujiBackupObjectInfo {
    pub fn new(buffer_len: usize) -> anyhow::Result<Self> {
        Ok(Self {
            compressed_size: u32::try_from(buffer_len)?,
        })
    }
}

impl PtpSerialize for FujiBackupObjectInfo {
    fn try_into_ptp(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.try_write_ptp(&mut buf)?;
        Ok(buf)
    }

    fn try_write_ptp(&self, buf: &mut Vec<u8>) -> io::Result<()> {
        let object_info = ObjectInfo {
            object_format: ObjectFormat::FujiBackup,
            compressed_size: self.compressed_size,
            ..Default::default()
        };

        object_info.try_write_ptp(buf)?;
        buf.write_all(&[0x0u8; 1020])?;

        Ok(())
    }
}
