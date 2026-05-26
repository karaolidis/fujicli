use std::{thread::sleep, time::Duration};

use log::debug;
use ptp_cursor::{PtpDeserialize, PtpSerialize};

use crate::{
    error::{CoreError, CoreResult},
    features::base::CameraBase,
    generated::renders::RenderBase,
    ptp::{CommandCode, DevicePropCode, ObjectFormat, ObjectInfo, Ptp},
};

pub const OUTGOING_OBJECT_HANDLE: [u32; 3] = [0x0, 0x0, 0x0];
pub const INCOMING_OBJECT_HANDLE: [u32; 3] = [u32::MAX, 0x0, 0x0];

// NOTE: Naively assuming that all cameras render in a similar way.
pub trait CameraRenderManager: CameraBase {
    fn send_image(&self, ptp: &mut Ptp, image: &[u8]) -> CoreResult<()> {
        debug!("Sending image to camera");
        let compressed_size =
            u32::try_from(image.len()).map_err(|_| CoreError::PayloadTooLarge(image.len()))?;
        let object_info = ObjectInfo {
            object_format: ObjectFormat::FujiRAF,
            compressed_size,
            filename: String::from("FUP_FILE.dat"),
            ..Default::default()
        };

        let serialized = object_info
            .try_into_ptp()
            .map_err(crate::ptp::PtpError::Io)?;
        ptp.send(
            CommandCode::FujiSendObjectInfo,
            &OUTGOING_OBJECT_HANDLE,
            Some(&serialized),
        )?;
        ptp.send(CommandCode::FujiSendObject, &[], Some(image))?;
        debug!("Sent image to camera");

        Ok(())
    }

    fn render_image(&self, ptp: &mut Ptp, draft: bool) -> CoreResult<Vec<u8>> {
        debug!("Starting image render");
        ptp.set_prop(DevicePropCode::FujiRawConversionRun, &u16::from(!draft))?;

        let handle;
        loop {
            debug!("Fetching rendered object handles");
            let response = ptp.send(CommandCode::GetObjectHandles, &[u32::MAX, 0, 0], None)?;
            let response = <Vec<u32>>::try_from_ptp(&response).map_err(crate::ptp::PtpError::Io)?;
            if let Some(&first) = response.first() {
                handle = first;
                break;
            }

            sleep(Duration::from_millis(100));
        }

        debug!("Fetching rendered image");
        let buf = ptp.send(CommandCode::GetObject, &[handle], None)?;
        debug!("Fetched rendered image");

        debug!("Cleaning up rendered image on camera");
        let _ = ptp.send(CommandCode::DeleteObject, &[handle], None)?;
        debug!("Cleaned up rendered image on camera");

        Ok(buf)
    }

    fn render(
        &self,
        ptp: &mut Ptp,
        image: &[u8],
        partial: &RenderBase,
        draft: bool,
    ) -> CoreResult<Vec<u8>>;
}
