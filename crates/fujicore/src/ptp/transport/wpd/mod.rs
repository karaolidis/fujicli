mod com;

use std::{cmp::min, num::NonZeroU32};

use log::{debug, trace};
use windows::{
    Win32::{
        Devices::PortableDevices::{
            WPD_CLIENT_NAME, WPD_COMMAND_MTP_EXT_END_DATA_TRANSFER,
            WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_READ,
            WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_WRITE,
            WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITHOUT_DATA_PHASE, WPD_COMMAND_MTP_EXT_READ_DATA,
            WPD_COMMAND_MTP_EXT_WRITE_DATA, WPD_PROPERTY_MTP_EXT_OPERATION_CODE,
            WPD_PROPERTY_MTP_EXT_OPERATION_PARAMS,
            WPD_PROPERTY_MTP_EXT_OPTIMAL_TRANSFER_BUFFER_SIZE, WPD_PROPERTY_MTP_EXT_RESPONSE_CODE,
            WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT, WPD_PROPERTY_MTP_EXT_TRANSFER_DATA,
            WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_READ,
            WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_READ,
            WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_WRITE,
            WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_WRITTEN,
            WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE,
        },
        Foundation::PROPERTYKEY,
    },
    core::HSTRING,
};

use crate::{
    CoreError, CoreResult, UsbId,
    ptp::{CommandCode, PtpError, ResponseCode},
    usb::FUJIFILM_VENDOR_ID,
};
use com::{PortableDevice, Values};

const CLIENT_NAME: &str = "fujicli";

/// MTP's "unknown size" marker, used for data phases of 4 GiB or more.
const UNKNOWN_DATA_SIZE: u64 = u32::MAX as u64;

pub struct Device {
    id: String,
    usb_id: UsbId,
    serial: Option<String>,
}

pub fn devices() -> CoreResult<Vec<Device>> {
    let devices = com::device_ids()?
        .into_iter()
        .filter_map(|id| {
            let usb_id = parse_usb_id(&id).filter(|u| u.vendor == FUJIFILM_VENDOR_ID)?;
            let serial = parse_serial(&id);
            trace!("Found WPD device {id} ({usb_id}, serial {serial:?})");
            Some(Device { id, usb_id, serial })
        })
        .collect();

    Ok(devices)
}

fn parse_usb_id(id: &str) -> Option<UsbId> {
    let id = id.to_ascii_lowercase();
    let hex_after = |marker: &str| {
        let start = id.find(marker)? + marker.len();
        u16::from_str_radix(id.get(start..start + 4)?, 16).ok()
    };

    Some(UsbId {
        vendor: hex_after("vid_")?,
        product: hex_after("pid_")?,
    })
}

/// Windows generates an instance ID containing `&` for devices without a
/// serial number.
fn parse_serial(id: &str) -> Option<String> {
    let instance = id.split('#').nth(2)?;
    (!instance.is_empty() && !instance.contains('&')).then(|| instance.to_owned())
}

impl Device {
    #[must_use]
    pub const fn usb_id(&self) -> UsbId {
        self.usb_id
    }

    #[must_use]
    pub fn serial(&self) -> Option<&str> {
        self.serial.as_deref()
    }

    pub fn open(&self, chunk_size: NonZeroU32) -> CoreResult<Transport> {
        let client_info = Values::new()?;
        client_info.set_str(&WPD_CLIENT_NAME, &HSTRING::from(CLIENT_NAME))?;

        let device = PortableDevice::open(&self.id, &client_info)?;
        debug!("Opened WPD device {}", self.id);

        Ok(Transport {
            device,
            chunk_size: chunk_size.get(),
        })
    }
}

pub struct Transport {
    device: PortableDevice,
    chunk_size: u32,
}

impl Transport {
    pub fn command(&mut self, code: CommandCode, params: &[u32]) -> CoreResult<()> {
        let parameters = operation(
            &WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITHOUT_DATA_PHASE,
            code,
            params,
        )?;
        check_response(&self.device.send(&parameters)?)
    }

    pub fn read(&mut self, code: CommandCode, params: &[u32]) -> CoreResult<Vec<u8>> {
        let parameters = operation(
            &WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_READ,
            code,
            params,
        )?;
        let results = self.device.send(&parameters)?;
        let context = results.string(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT)?;

        let total = results
            .u64(&WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE)
            .ok()
            .filter(|&total| total != UNKNOWN_DATA_SIZE)
            .and_then(|total| usize::try_from(total).ok());

        let data = self.read_data(&context, total, self.transfer_len(&results));
        finish(data, self.end_data_transfer(&context))
    }

    pub fn write(&mut self, code: CommandCode, params: &[u32], data: &[u8]) -> CoreResult<()> {
        let parameters = operation(
            &WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_WRITE,
            code,
            params,
        )?;
        parameters.set_u64(
            &WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE,
            data.len() as u64,
        )?;
        let results = self.device.send(&parameters)?;
        let context = results.string(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT)?;

        let written = self.write_data(&context, data, self.transfer_len(&results));
        finish(written, self.end_data_transfer(&context))
    }

    /// The driver's optimal transfer size, capped by the camera's chunk size.
    fn transfer_len(&self, results: &Values) -> u32 {
        match results.u32(&WPD_PROPERTY_MTP_EXT_OPTIMAL_TRANSFER_BUFFER_SIZE) {
            Ok(optimal) if optimal > 0 => min(optimal, self.chunk_size),
            _ => self.chunk_size,
        }
    }

    fn read_data(
        &self,
        context: &HSTRING,
        total: Option<usize>,
        chunk: u32,
    ) -> CoreResult<Vec<u8>> {
        let mut data = Vec::with_capacity(total.unwrap_or(0));

        loop {
            let want = total.map_or(chunk, |total| {
                u32::try_from(total - data.len()).map_or(chunk, |remaining| min(chunk, remaining))
            });
            if want == 0 {
                return Ok(data);
            }

            let bytes = self.read_chunk(context, want)?;
            if bytes.is_empty() {
                return match total {
                    Some(total) => Err(PtpError::Malformed(format!(
                        "WPD transfer ended after {} of {total} bytes",
                        data.len()
                    ))
                    .into()),
                    None => Ok(data),
                };
            }

            let short = bytes.len() < want as usize;
            data.extend_from_slice(&bytes);
            trace!("WPD read: {} bytes ({} total)", bytes.len(), data.len());

            if total.is_none() && short {
                return Ok(data);
            }
        }
    }

    fn read_chunk(&self, context: &HSTRING, len: u32) -> CoreResult<Vec<u8>> {
        let parameters = Values::for_command(&WPD_COMMAND_MTP_EXT_READ_DATA)?;
        parameters.set_str(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT, context)?;
        parameters.set_u32(&WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_READ, len)?;
        // The driver copies the data into a buffer supplied by the caller.
        parameters.set_buffer(&WPD_PROPERTY_MTP_EXT_TRANSFER_DATA, &vec![0; len as usize])?;

        let results = self.device.send(&parameters)?;
        let read = results.u32(&WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_READ)?;
        let mut bytes = results.buffer(&WPD_PROPERTY_MTP_EXT_TRANSFER_DATA)?;
        bytes.truncate(min(read, len) as usize);
        Ok(bytes)
    }

    fn write_data(&self, context: &HSTRING, mut data: &[u8], chunk: u32) -> CoreResult<()> {
        while !data.is_empty() {
            let written = self.write_chunk(context, &data[..min(chunk as usize, data.len())])?;
            if written == 0 {
                return Err(PtpError::Malformed("WPD device accepted 0 bytes".to_owned()).into());
            }

            data = &data[written..];
            trace!("WPD write: {written} bytes ({} left)", data.len());
        }

        Ok(())
    }

    fn write_chunk(&self, context: &HSTRING, chunk: &[u8]) -> CoreResult<usize> {
        let len =
            u32::try_from(chunk.len()).map_err(|_| CoreError::PayloadTooLarge(chunk.len()))?;

        let parameters = Values::for_command(&WPD_COMMAND_MTP_EXT_WRITE_DATA)?;
        parameters.set_str(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT, context)?;
        parameters.set_u32(&WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_WRITE, len)?;
        parameters.set_buffer(&WPD_PROPERTY_MTP_EXT_TRANSFER_DATA, chunk)?;

        let results = self.device.send(&parameters)?;
        let written = results.u32(&WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_WRITTEN)?;
        Ok(min(written as usize, chunk.len()))
    }

    fn end_data_transfer(&self, context: &HSTRING) -> CoreResult<()> {
        let parameters = Values::for_command(&WPD_COMMAND_MTP_EXT_END_DATA_TRANSFER)?;
        parameters.set_str(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT, context)?;
        check_response(&self.device.send(&parameters)?)
    }
}

fn operation(command: &PROPERTYKEY, code: CommandCode, params: &[u32]) -> CoreResult<Values> {
    trace!("WPD transaction: code={code:?}, params={params:?}");

    let parameters = Values::for_command(command)?;
    parameters.set_u32(&WPD_PROPERTY_MTP_EXT_OPERATION_CODE, u16::from(code).into())?;
    parameters.set_u32_list(&WPD_PROPERTY_MTP_EXT_OPERATION_PARAMS, params)?;
    Ok(parameters)
}

fn check_response(results: &Values) -> CoreResult<()> {
    let code = results.u32(&WPD_PROPERTY_MTP_EXT_RESPONSE_CODE)?;
    let code = u16::try_from(code)
        .map_err(|_| PtpError::Malformed(format!("invalid MTP response code 0x{code:08x}")))?;

    match ResponseCode::try_from(code) {
        Ok(ResponseCode::Ok) => Ok(()),
        Ok(code) => Err(PtpError::Response(code).into()),
        Err(_) => Err(PtpError::UnknownContainerCode(code).into()),
    }
}

/// The response code arrives when the transfer ends, so a camera rejection
/// takes precedence over the failure of the data phase it caused.
fn finish<T>(data: CoreResult<T>, end: CoreResult<()>) -> CoreResult<T> {
    match (data, end) {
        (_, Err(e @ CoreError::Ptp(PtpError::Response(_)))) | (Err(e), _) => Err(e),
        (Ok(data), end) => end.map(|()| data),
    }
}

#[cfg(test)]
mod tests {
    use super::{UsbId, parse_serial, parse_usb_id};

    const ID: &str = r"\\?\usb#vid_04cb&pid_02fc#59353638383124052215e310219577#{6ac27878-a6fa-4155-ba85-f98f491d4f33}";

    #[test]
    fn parses_usb_id() {
        let expected = UsbId {
            vendor: 0x04cb,
            product: 0x02fc,
        };
        assert_eq!(parse_usb_id(ID), Some(expected));
        assert_eq!(parse_usb_id(r"\\?\swd#wpdbusenum#foo"), None);
    }

    #[test]
    fn parses_serial() {
        assert_eq!(
            parse_serial(ID).as_deref(),
            Some("59353638383124052215e310219577")
        );
        assert_eq!(
            parse_serial(r"\\?\usb#vid_04cb&pid_02fc#5&1a2b3c4d&0&1#{guid}"),
            None
        );
    }
}
