//! Driver-free Windows Portable Devices (WPD/MTP) PTP transport.
//!
//! The Microsoft inbox MTP driver frames PTP itself, so this transport never
//! builds PTP containers and never tracks transaction IDs. Standard and
//! vendor-extended operations both go through the `WPD_COMMAND_MTP_EXT_*`
//! command set.

use std::{cell::Cell, cmp::min, iter::once, thread::sleep, time::Duration};

use anyhow::{anyhow, bail};
use log::{debug, trace, warn};
use windows::{
    Win32::{
        Devices::PortableDevices::{
            IPortableDevice, IPortableDeviceManager, IPortableDevicePropVariantCollection,
            IPortableDeviceValues, PortableDeviceFTM, PortableDeviceManager,
            PortableDevicePropVariantCollection, PortableDeviceValues, WPD_CLIENT_DESIRED_ACCESS,
            WPD_CLIENT_MAJOR_VERSION, WPD_CLIENT_MINOR_VERSION, WPD_CLIENT_NAME,
            WPD_CLIENT_REVISION, WPD_COMMAND_MTP_EXT_END_DATA_TRANSFER,
            WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_READ,
            WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_WRITE,
            WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITHOUT_DATA_PHASE,
            WPD_COMMAND_MTP_EXT_GET_SUPPORTED_VENDOR_OPCODES, WPD_COMMAND_MTP_EXT_READ_DATA,
            WPD_COMMAND_MTP_EXT_WRITE_DATA, WPD_PROPERTY_COMMON_COMMAND_CATEGORY,
            WPD_PROPERTY_COMMON_COMMAND_ID, WPD_PROPERTY_COMMON_HRESULT,
            WPD_PROPERTY_MTP_EXT_OPERATION_CODE, WPD_PROPERTY_MTP_EXT_OPERATION_PARAMS,
            WPD_PROPERTY_MTP_EXT_OPTIMAL_TRANSFER_BUFFER_SIZE, WPD_PROPERTY_MTP_EXT_RESPONSE_CODE,
            WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT, WPD_PROPERTY_MTP_EXT_TRANSFER_DATA,
            WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_READ,
            WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_READ,
            WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_WRITE,
            WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_WRITTEN,
            WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE,
            WPD_PROPERTY_MTP_EXT_VENDOR_OPERATION_CODES,
        },
        Foundation::{PROPERTYKEY, RPC_E_CHANGED_MODE},
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree, StructuredStorage::PROPVARIANT,
            },
            Variant::VT_UI4,
        },
    },
    core::{PCWSTR, PWSTR},
};

use crate::ptp::{CommandCode, ResponseCode, error, transport::PtpTransport};

const CLIENT_NAME: &str = "fujicli";

/// Access rights requested from the WPD driver. Write access is mandatory for
/// `SetDevicePropValue`, `SendObject`, and `DeleteObject`.
const GENERIC_READ_WRITE: u32 = 0x8000_0000 | 0x4000_0000;

/// Mirrors `FTL_SetDeviceBusyRetryInfo` in Fujifilm's own WPD transport.
const BUSY_RETRIES: u32 = 10;
const BUSY_DELAY: Duration = Duration::from_millis(200);

/// Conservative fallback until the camera definition supplies a chunk size.
const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

/// The device reports an unknown data-phase length.
const UNKNOWN_DATA_SIZE: u64 = u32::MAX as u64;

/// Take a `'static` pointer to a WPD property key.
///
/// The generated `PROPERTYKEY` items are `const`, so referencing them directly
/// would only ever yield the address of a temporary.
macro_rules! key {
    ($name:path) => {{
        static KEY: PROPERTYKEY = $name;
        &raw const KEY
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataPhase {
    None,
    In,
    Out,
}

impl DataPhase {
    fn classify(code: u16, has_data: bool) -> Self {
        if has_data {
            return Self::Out;
        }

        match CommandCode::try_from(code) {
            Ok(
                CommandCode::GetDeviceInfo
                | CommandCode::GetObjectHandles
                | CommandCode::GetObjectInfo
                | CommandCode::GetObject
                | CommandCode::GetDevicePropValue,
            ) => Self::In,
            _ => Self::None,
        }
    }
}

fn ensure_com() -> anyhow::Result<()> {
    thread_local! {
        static INITIALIZED: Cell<bool> = const { Cell::new(false) };
    }

    INITIALIZED.with(|initialized| {
        if initialized.get() {
            return Ok(());
        }

        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_err() && hr != RPC_E_CHANGED_MODE {
            bail!("Failed to initialize COM: {hr:?}");
        }

        initialized.set(true);
        Ok(())
    })
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(once(0)).collect()
}

/// Takes ownership of a `PWSTR` returned by WPD and frees it.
unsafe fn take_pwstr(value: PWSTR) -> anyhow::Result<String> {
    if value.is_null() {
        return Ok(String::new());
    }

    let result = unsafe { value.to_string() };
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    Ok(result?)
}

fn propvariant_u32(value: u32) -> PROPVARIANT {
    let mut variant = PROPVARIANT::default();
    let inner = unsafe { &mut *variant.Anonymous.Anonymous };
    inner.vt = VT_UI4;
    inner.Anonymous.ulVal = value;
    variant
}

fn propvariant_as_u32(variant: &PROPVARIANT) -> Option<u32> {
    let inner = unsafe { &*variant.Anonymous.Anonymous };
    if inner.vt == VT_UI4 {
        Some(unsafe { inner.Anonymous.ulVal })
    } else {
        None
    }
}

fn new_values() -> anyhow::Result<IPortableDeviceValues> {
    let values: IPortableDeviceValues =
        unsafe { CoCreateInstance(&PortableDeviceValues, None, CLSCTX_INPROC_SERVER)? };
    Ok(values)
}

fn new_params(params: &[u32]) -> anyhow::Result<IPortableDevicePropVariantCollection> {
    let collection: IPortableDevicePropVariantCollection = unsafe {
        CoCreateInstance(
            &PortableDevicePropVariantCollection,
            None,
            CLSCTX_INPROC_SERVER,
        )?
    };

    for param in params {
        let variant = propvariant_u32(*param);
        unsafe { collection.Add(&raw const variant)? };
    }

    Ok(collection)
}

/// Prepare the `IPortableDeviceValues` for a WPD command key.
fn command_values(command: &PROPERTYKEY) -> anyhow::Result<IPortableDeviceValues> {
    let values = new_values()?;
    unsafe {
        values.SetGuidValue(key!(WPD_PROPERTY_COMMON_COMMAND_CATEGORY), &command.fmtid)?;
        values.SetUnsignedIntegerValue(key!(WPD_PROPERTY_COMMON_COMMAND_ID), command.pid)?;
    }
    Ok(values)
}

pub struct WpdDevice {
    pub device_id: String,
    pub friendly_name: String,
    pub vendor: u16,
    pub product: u16,
}

impl WpdDevice {
    #[must_use]
    pub fn id(&self) -> String {
        self.device_id.clone()
    }

    pub fn open(&self) -> anyhow::Result<Wpd> {
        Wpd::open(&self.device_id)
    }
}

/// Pull `vid_xxxx` / `pid_xxxx` out of a WPD device ID.
fn parse_usb_ids(device_id: &str) -> Option<(u16, u16)> {
    let lower = device_id.to_ascii_lowercase();

    let extract = |marker: &str| -> Option<u16> {
        let index = lower.find(marker)? + marker.len();
        let digits: String = lower[index..].chars().take(4).collect();
        if digits.len() < 4 {
            return None;
        }
        u16::from_str_radix(&digits, 16).ok()
    };

    Some((extract("vid_")?, extract("pid_")?))
}

pub fn enumerate() -> anyhow::Result<Vec<WpdDevice>> {
    ensure_com()?;

    let manager: IPortableDeviceManager =
        unsafe { CoCreateInstance(&PortableDeviceManager, None, CLSCTX_INPROC_SERVER)? };

    let mut count: u32 = 0;
    unsafe { manager.GetDevices(std::ptr::null_mut(), &raw mut count)? };
    if count == 0 {
        return Ok(Vec::new());
    }

    let mut ids = vec![PWSTR::null(); count as usize];
    unsafe { manager.GetDevices(ids.as_mut_ptr(), &raw mut count)? };

    let mut devices = Vec::new();
    for id in ids.into_iter().take(count as usize) {
        let device_id = unsafe { take_pwstr(id)? };
        if device_id.is_empty() {
            continue;
        }

        trace!("Found WPD device {device_id}");

        let Some((vendor, product)) = parse_usb_ids(&device_id) else {
            trace!("WPD device {device_id} is not a USB device");
            continue;
        };

        let friendly_name = friendly_name(&manager, &device_id).unwrap_or_default();

        devices.push(WpdDevice {
            device_id,
            friendly_name,
            vendor,
            product,
        });
    }

    Ok(devices)
}

fn friendly_name(manager: &IPortableDeviceManager, device_id: &str) -> anyhow::Result<String> {
    let id = wide(device_id);
    let id = PCWSTR(id.as_ptr());

    let mut len: u32 = 0;
    unsafe { manager.GetDeviceFriendlyName(id, PWSTR::null(), &raw mut len)? };
    if len == 0 {
        return Ok(String::new());
    }

    let mut buffer = vec![0u16; len as usize];
    unsafe {
        manager.GetDeviceFriendlyName(id, PWSTR(buffer.as_mut_ptr()), &raw mut len)?;
    }

    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Ok(String::from_utf16_lossy(&buffer[..end]))
}

pub struct Wpd {
    device: IPortableDevice,
    device_id: String,
    chunk_size: usize,
}

impl Wpd {
    fn open(device_id: &str) -> anyhow::Result<Self> {
        ensure_com()?;

        let client_info = new_values()?;
        let name = wide(CLIENT_NAME);
        unsafe {
            client_info.SetStringValue(key!(WPD_CLIENT_NAME), PCWSTR(name.as_ptr()))?;
            client_info.SetUnsignedIntegerValue(key!(WPD_CLIENT_MAJOR_VERSION), 1)?;
            client_info.SetUnsignedIntegerValue(key!(WPD_CLIENT_MINOR_VERSION), 0)?;
            client_info.SetUnsignedIntegerValue(key!(WPD_CLIENT_REVISION), 0)?;
            client_info
                .SetUnsignedIntegerValue(key!(WPD_CLIENT_DESIRED_ACCESS), GENERIC_READ_WRITE)?;
        }

        let device: IPortableDevice =
            unsafe { CoCreateInstance(&PortableDeviceFTM, None, CLSCTX_INPROC_SERVER)? };

        let id = wide(device_id);
        unsafe { device.Open(PCWSTR(id.as_ptr()), &client_info)? };
        debug!("Opened WPD device {device_id}");

        let transport = Self {
            device,
            device_id: device_id.to_string(),
            chunk_size: DEFAULT_CHUNK_SIZE,
        };

        transport.log_vendor_opcodes();

        Ok(transport)
    }

    /// Purely diagnostic: MTP devices routinely under-report vendor opcodes, so
    /// this never gates a transaction.
    fn log_vendor_opcodes(&self) {
        let codes = match self.supported_vendor_opcodes() {
            Ok(codes) => codes,
            Err(e) => {
                debug!("Could not query supported vendor opcodes: {e}");
                return;
            }
        };

        debug!("Device advertises vendor opcodes: {codes:04x?}");

        for required in [CommandCode::FujiSendObjectInfo, CommandCode::FujiSendObject] {
            let code = u32::from(u16::from(required));
            if !codes.contains(&code) {
                warn!(
                    "Device does not advertise vendor opcode {required:?} (0x{code:04x}); attempting to use it anyway"
                );
            }
        }
    }

    fn supported_vendor_opcodes(&self) -> anyhow::Result<Vec<u32>> {
        let values = command_values(&WPD_COMMAND_MTP_EXT_GET_SUPPORTED_VENDOR_OPCODES)?;
        let results = self.send_command(&values)?;

        let collection = unsafe {
            results.GetIPortableDevicePropVariantCollectionValue(key!(
                WPD_PROPERTY_MTP_EXT_VENDOR_OPERATION_CODES
            ))?
        };

        let mut count: u32 = 0;
        unsafe { collection.GetCount(&raw mut count)? };

        let mut codes = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut variant = PROPVARIANT::default();
            unsafe { collection.GetAt(index, (&raw mut variant).cast_const())? };
            if let Some(code) = propvariant_as_u32(&variant) {
                codes.push(code);
            }
        }

        Ok(codes)
    }

    fn send_command(
        &self,
        values: &IPortableDeviceValues,
    ) -> anyhow::Result<IPortableDeviceValues> {
        let results = unsafe { self.device.SendCommand(0, values)? };

        let hr = unsafe { results.GetErrorValue(key!(WPD_PROPERTY_COMMON_HRESULT)) };
        if let Ok(hr) = hr
            && hr.is_err()
        {
            bail!("WPD command failed: {hr:?}");
        }

        Ok(results)
    }

    /// Populate the operation code and parameters shared by all three
    /// `EXECUTE_COMMAND` variants.
    fn set_operation(
        values: &IPortableDeviceValues,
        code: u16,
        params: &[u32],
    ) -> anyhow::Result<()> {
        let collection = new_params(params)?;
        unsafe {
            values.SetUnsignedIntegerValue(
                key!(WPD_PROPERTY_MTP_EXT_OPERATION_CODE),
                u32::from(code),
            )?;
            values.SetIPortableDevicePropVariantCollectionValue(
                key!(WPD_PROPERTY_MTP_EXT_OPERATION_PARAMS),
                &collection,
            )?;
        }
        Ok(())
    }

    fn transfer_context(results: &IPortableDeviceValues) -> anyhow::Result<String> {
        let context =
            unsafe { results.GetStringValue(key!(WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT))? };
        unsafe { take_pwstr(context) }
    }

    /// Effective transfer size: the driver's optimum, capped by the camera definition.
    fn effective_chunk(&self, optimal: u32) -> usize {
        let optimal = if optimal == 0 {
            self.chunk_size
        } else {
            optimal as usize
        };
        min(optimal, self.chunk_size).max(1)
    }

    fn end_data_transfer(&self, context: &str) -> anyhow::Result<u16> {
        let values = command_values(&WPD_COMMAND_MTP_EXT_END_DATA_TRANSFER)?;
        let context = wide(context);
        unsafe {
            values.SetStringValue(
                key!(WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT),
                PCWSTR(context.as_ptr()),
            )?;
        }

        let results = self.send_command(&values)?;
        Self::response_code(&results)
    }

    fn response_code(results: &IPortableDeviceValues) -> anyhow::Result<u16> {
        let code =
            unsafe { results.GetUnsignedIntegerValue(key!(WPD_PROPERTY_MTP_EXT_RESPONSE_CODE))? };
        u16::try_from(code).map_err(|_| anyhow!("Invalid MTP response code 0x{code:08x}"))
    }

    fn check_response(code: u16) -> anyhow::Result<()> {
        if code == u16::from(ResponseCode::Ok) {
            return Ok(());
        }
        Err(anyhow!(error::Error::Response(code)))
    }

    fn execute_without_data(&self, code: u16, params: &[u32]) -> anyhow::Result<Vec<u8>> {
        let values = command_values(&WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITHOUT_DATA_PHASE)?;
        Self::set_operation(&values, code, params)?;

        let results = self.send_command(&values)?;
        Self::check_response(Self::response_code(&results)?)?;

        Ok(Vec::new())
    }

    fn execute_read(&self, code: u16, params: &[u32]) -> anyhow::Result<Vec<u8>> {
        let values = command_values(&WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_READ)?;
        Self::set_operation(&values, code, params)?;

        let results = self.send_command(&values)?;
        let context = Self::transfer_context(&results)?;

        let total = unsafe {
            results
                .GetUnsignedLargeIntegerValue(key!(WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE))
        }
        .unwrap_or(UNKNOWN_DATA_SIZE);

        let optimal = unsafe {
            results.GetUnsignedIntegerValue(key!(WPD_PROPERTY_MTP_EXT_OPTIMAL_TRANSFER_BUFFER_SIZE))
        }
        .unwrap_or(0);

        let chunk = self.effective_chunk(optimal);
        let known = total != UNKNOWN_DATA_SIZE;
        trace!("WPD read: total={total}, optimal={optimal}, chunk={chunk}");

        let mut payload: Vec<u8> = Vec::new();
        if known {
            payload.reserve(usize::try_from(total).unwrap_or(0));
        }

        let result = (|| -> anyhow::Result<()> {
            loop {
                if known && payload.len() as u64 >= total {
                    break;
                }

                let want = if known {
                    let remaining = total - payload.len() as u64;
                    min(chunk as u64, remaining).try_into().unwrap_or(chunk)
                } else {
                    chunk
                };

                let read = self.read_chunk(&context, want, &mut payload)?;
                trace!("WPD read: chunk of {read} bytes ({} total)", payload.len());

                if read == 0 || (!known && read < want) {
                    break;
                }
            }

            Ok(())
        })();

        let response = self.end_data_transfer(&context);
        result?;
        Self::check_response(response?)?;

        Ok(payload)
    }

    fn read_chunk(
        &self,
        context: &str,
        want: usize,
        payload: &mut Vec<u8>,
    ) -> anyhow::Result<usize> {
        let values = command_values(&WPD_COMMAND_MTP_EXT_READ_DATA)?;
        let context = wide(context);
        let buffer = vec![0u8; want];

        unsafe {
            values.SetStringValue(
                key!(WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT),
                PCWSTR(context.as_ptr()),
            )?;
            values.SetUnsignedIntegerValue(
                key!(WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_READ),
                u32::try_from(want)?,
            )?;
            values.SetBufferValue(key!(WPD_PROPERTY_MTP_EXT_TRANSFER_DATA), &buffer)?;
        }

        let results = self.send_command(&values)?;

        let read = unsafe {
            results.GetUnsignedIntegerValue(key!(WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_READ))?
        } as usize;

        let mut data: *mut u8 = std::ptr::null_mut();
        let mut len: u32 = 0;
        unsafe {
            results.GetBufferValue(
                key!(WPD_PROPERTY_MTP_EXT_TRANSFER_DATA),
                &raw mut data,
                &raw mut len,
            )?;
        }

        if !data.is_null() {
            let len = min(read, len as usize);
            payload.extend_from_slice(unsafe { std::slice::from_raw_parts(data, len) });
            unsafe { CoTaskMemFree(Some(data.cast())) };
        }

        Ok(read)
    }

    fn execute_write(&self, code: u16, params: &[u32], data: &[u8]) -> anyhow::Result<Vec<u8>> {
        let values = command_values(&WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_WRITE)?;
        Self::set_operation(&values, code, params)?;
        unsafe {
            values.SetUnsignedLargeIntegerValue(
                key!(WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE),
                data.len() as u64,
            )?;
        }

        let results = self.send_command(&values)?;
        let context = Self::transfer_context(&results)?;

        let optimal = unsafe {
            results.GetUnsignedIntegerValue(key!(WPD_PROPERTY_MTP_EXT_OPTIMAL_TRANSFER_BUFFER_SIZE))
        }
        .unwrap_or(0);

        let chunk = self.effective_chunk(optimal);
        trace!(
            "WPD write: total={}, optimal={optimal}, chunk={chunk}",
            data.len()
        );

        let result = (|| -> anyhow::Result<()> {
            let mut offset = 0usize;
            while offset < data.len() {
                let end = min(offset + chunk, data.len());
                let written = self.write_chunk(&context, &data[offset..end])?;
                trace!("WPD write: chunk of {written} bytes ({offset} sent)");

                if written == 0 {
                    bail!("WPD device accepted 0 bytes, aborting transfer");
                }

                offset += written;
            }

            Ok(())
        })();

        let response = self.end_data_transfer(&context);
        result?;
        Self::check_response(response?)?;

        Ok(Vec::new())
    }

    fn write_chunk(&self, context: &str, data: &[u8]) -> anyhow::Result<usize> {
        let values = command_values(&WPD_COMMAND_MTP_EXT_WRITE_DATA)?;
        let context = wide(context);

        unsafe {
            values.SetStringValue(
                key!(WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT),
                PCWSTR(context.as_ptr()),
            )?;
            values.SetUnsignedIntegerValue(
                key!(WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_WRITE),
                u32::try_from(data.len())?,
            )?;
            values.SetBufferValue(key!(WPD_PROPERTY_MTP_EXT_TRANSFER_DATA), data)?;
        }

        let results = self.send_command(&values)?;

        let written = unsafe {
            results
                .GetUnsignedIntegerValue(key!(WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_WRITTEN))?
        } as usize;

        Ok(min(written, data.len()))
    }

    fn transact_once(
        &self,
        code: u16,
        params: &[u32],
        data: Option<&[u8]>,
    ) -> anyhow::Result<Vec<u8>> {
        match DataPhase::classify(code, data.is_some()) {
            DataPhase::None => self.execute_without_data(code, params),
            DataPhase::In => self.execute_read(code, params),
            DataPhase::Out => self.execute_write(code, params, data.unwrap_or_default()),
        }
    }
}

fn is_device_busy(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<error::Error>(),
        Some(error::Error::Response(code)) if *code == u16::from(ResponseCode::DeviceBusy)
    )
}

impl PtpTransport for Wpd {
    fn transact(
        &mut self,
        code: u16,
        params: &[u32],
        data: Option<&[u8]>,
    ) -> anyhow::Result<Vec<u8>> {
        trace!(
            "WPD transaction: code=0x{code:04x}, params={params:?}, data_len={}",
            data.map_or(0, <[u8]>::len)
        );

        for attempt in 0..=BUSY_RETRIES {
            let result = self.transact_once(code, params, data);

            match result {
                Err(e) if is_device_busy(&e) && attempt < BUSY_RETRIES => {
                    debug!("Device busy, retrying ({}/{BUSY_RETRIES})", attempt + 1);
                    sleep(BUSY_DELAY);
                }
                result => return result,
            }
        }

        unreachable!("retry loop always returns")
    }

    /// The WPD MTP driver owns the PTP session; a second `OpenSession` would
    /// fail with `SessionAlreadyOpen`.
    fn open_session(&mut self, _session_id: u32) -> anyhow::Result<()> {
        debug!("WPD driver owns the PTP session, skipping OpenSession");
        Ok(())
    }

    fn close_session(&mut self, _session_id: u32) -> anyhow::Result<()> {
        debug!("WPD driver owns the PTP session, skipping CloseSession");
        Ok(())
    }

    fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    fn set_chunk_size(&mut self, chunk_size: usize) {
        if chunk_size == 0 {
            warn!("Ignoring implausible chunk size {chunk_size}");
            return;
        }
        self.chunk_size = chunk_size;
    }

    fn id(&self) -> String {
        self.device_id.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{DataPhase, parse_usb_ids};

    #[test]
    fn parses_usb_ids() {
        let id = r"\\?\usb#vid_04cb&pid_02fb#593536#{6ac27878-a6fa-4155-ba85-f98f491d4f33}";
        assert_eq!(parse_usb_ids(id), Some((0x04cb, 0x02fb)));
        assert_eq!(parse_usb_ids(r"\\?\swd#wpdbusenum#foo"), None);
    }

    #[test]
    fn classifies_data_phases() {
        assert_eq!(DataPhase::classify(0x1001, false), DataPhase::In);
        assert_eq!(DataPhase::classify(0x100B, false), DataPhase::None);
        assert_eq!(DataPhase::classify(0x900D, true), DataPhase::Out);
        assert_eq!(DataPhase::classify(0x1016, true), DataPhase::Out);
    }
}
