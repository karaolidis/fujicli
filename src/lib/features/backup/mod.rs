pub mod ptp;

use log::debug;
use ptp::FujiBackupObjectInfo;
use ptp_cursor::PtpSerialize;

use crate::ptp::{Ptp, hex::CommandCode};

use super::base::CameraBase;

pub const OBJECT_HANDLE: [u32; 1] = [0x0];
pub const EXPORT_OBJECT_INFO_HANDLE: [u32; 1] = [0x0];
pub const IMPORT_OBJECT_INFO_HANDLE: [u32; 2] = [0x0, 0x0];

// NOTE: Naively assuming that all cameras backup/restore in the same way.
// The default functions and blanket impl should be removed if this is not the case.
pub trait CameraBackups: CameraBase {
    fn export_backup(&self, ptp: &mut Ptp) -> anyhow::Result<Vec<u8>> {
        debug!("Starting backup export");
        let _ = ptp.send(CommandCode::GetObjectInfo, &EXPORT_OBJECT_INFO_HANDLE, None)?;
        let response = ptp.send(CommandCode::GetObject, &OBJECT_HANDLE, None)?;
        debug!("Backup export completed");

        Ok(response)
    }

    fn import_backup(&self, ptp: &mut Ptp, buffer: &[u8]) -> anyhow::Result<()> {
        debug!("Starting backup import");
        let object_info = FujiBackupObjectInfo::new(buffer.len())?;
        let _ = ptp.send(
            CommandCode::SendObjectInfo,
            &IMPORT_OBJECT_INFO_HANDLE,
            Some(&object_info.try_into_ptp()?),
        )?;
        let _ = ptp.send(CommandCode::SendObject, &OBJECT_HANDLE, Some(buffer))?;
        debug!("Backup import completed");

        Ok(())
    }
}

impl<T> CameraBackups for T where T: CameraBase {}
