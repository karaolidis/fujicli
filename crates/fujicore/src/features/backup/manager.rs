use std::io::{self, Write};

use log::debug;
use ptp_cursor::PtpSerialize;

use crate::{
    error::{CoreError, CoreResult},
    features::base::CameraBase,
    ptp::{CommandCode, ObjectFormat, ObjectInfo, Ptp},
};

pub const OBJECT_HANDLE: [u32; 1] = [0x0];
pub const EXPORT_OBJECT_INFO_HANDLE: [u32; 1] = [0x0];
pub const IMPORT_OBJECT_INFO_HANDLE: [u32; 2] = [0x0, 0x0];

// NOTE: Naively assuming that all cameras backup/restore in the same way.
pub trait CameraBackupManager: CameraBase {
    fn export_backup(&self, ptp: &mut Ptp) -> CoreResult<Vec<u8>> {
        debug!("Starting backup export");
        let _ = ptp.send(CommandCode::GetObjectInfo, &EXPORT_OBJECT_INFO_HANDLE, None)?;
        let response = ptp.send(CommandCode::GetObject, &OBJECT_HANDLE, None)?;
        debug!("Backup export completed");

        Ok(response)
    }

    fn import_backup(&self, ptp: &mut Ptp, buffer: &[u8]) -> CoreResult<()> {
        debug!("Starting backup import");
        let object_info = BackupObjectInfo::new(buffer.len())?;
        let serialized = object_info
            .try_into_ptp()
            .map_err(crate::ptp::PtpError::Io)?;
        let _ = ptp.send(
            CommandCode::SendObjectInfo,
            &IMPORT_OBJECT_INFO_HANDLE,
            Some(&serialized),
        )?;
        let _ = ptp.send(CommandCode::SendObject, &OBJECT_HANDLE, Some(buffer))?;
        debug!("Backup import completed");

        Ok(())
    }
}

impl<T> CameraBackupManager for T where T: CameraBase {}

// NOTE: Naively assuming that all cameras support backup/restore using the same structs.
pub struct BackupObjectInfo {
    compressed_size: u32,
}

impl BackupObjectInfo {
    pub fn new(buffer_len: usize) -> CoreResult<Self> {
        let compressed_size =
            u32::try_from(buffer_len).map_err(|_| CoreError::PayloadTooLarge(buffer_len))?;
        Ok(Self { compressed_size })
    }
}

impl PtpSerialize for BackupObjectInfo {
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
