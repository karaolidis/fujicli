pub mod ptp;

use log::debug;
use ptp::FujiBackupObjectInfo;
use ptp_cursor::PtpSerialize;

use crate::camera::ptp::{Ptp, hex::CommandCode};

use super::base::CameraBase;

// NOTE: Naively assuming that all cameras backup/restore in the same way.
// The default functions and blanket impl should be removed if this is not the case.
pub trait CameraBackups: CameraBase {
    fn export_backup(&self, ptp: &mut Ptp) -> anyhow::Result<Vec<u8>> {
        const HANDLE: u32 = 0x0;

        debug!("Sending GetObjectInfo command for backup");
        let response = ptp.send(CommandCode::GetObjectInfo, &[HANDLE], None)?;
        debug!("Received response with {} bytes", response.len());

        debug!("Sending GetObject command for backup");
        let response = ptp.send(CommandCode::GetObject, &[HANDLE], None)?;
        debug!("Received response with {} bytes", response.len());

        Ok(response)
    }

    fn import_backup(&self, ptp: &mut Ptp, buffer: &[u8]) -> anyhow::Result<()> {
        debug!("Sending SendObjectInfo command for backup");
        let object_info = FujiBackupObjectInfo::new(buffer.len())?;
        let response = {
            ptp.send(
                CommandCode::SendObjectInfo,
                &[0x0, 0x0],
                Some(&object_info.try_into_ptp()?),
            )
        }?;
        debug!("Received response with {} bytes", response.len());

        debug!("Sending SendObject command for backup");
        let response = ptp.send(CommandCode::SendObject, &[0x0], Some(buffer))?;
        debug!("Received response with {} bytes", response.len());

        Ok(())
    }
}

impl<T> CameraBackups for T where T: CameraBase {}
