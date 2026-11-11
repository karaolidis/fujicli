use std::{cell::Cell, mem::ManuallyDrop, ptr, slice};

use windows::{
    Win32::{
        Devices::PortableDevices::{
            IPortableDevice, IPortableDeviceManager, IPortableDevicePropVariantCollection,
            IPortableDeviceValues, PortableDeviceFTM, PortableDeviceManager,
            PortableDevicePropVariantCollection, PortableDeviceValues,
            WPD_PROPERTY_COMMON_COMMAND_CATEGORY, WPD_PROPERTY_COMMON_COMMAND_ID,
            WPD_PROPERTY_COMMON_HRESULT,
        },
        Foundation::{PROPERTYKEY, RPC_E_CHANGED_MODE},
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree,
                StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0},
            },
            Variant::VT_UI4,
        },
    },
    core::{GUID, HSTRING, Interface, PWSTR},
};

use crate::{CoreError, CoreResult, ptp::PtpError};

/// COM interfaces are `!Send`, so initializing each thread once covers every
/// object created on it.
fn init() -> CoreResult<()> {
    thread_local! {
        static INITIALIZED: Cell<bool> = const { Cell::new(false) };
    }

    if INITIALIZED.get() {
        return Ok(());
    }

    // SAFETY: The reserved argument is null.
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr != RPC_E_CHANGED_MODE {
        hr.ok()?;
    }

    INITIALIZED.set(true);
    Ok(())
}

fn create<T: Interface>(class: &GUID) -> CoreResult<T> {
    init()?;
    // SAFETY: `class` outlives the call.
    Ok(unsafe { CoCreateInstance(class, None, CLSCTX_INPROC_SERVER)? })
}

/// Memory COM allocated and handed to us.
struct CoTaskMem<T>(*mut T);

impl<T> Drop for CoTaskMem<T> {
    fn drop(&mut self) {
        // SAFETY: We own the allocation, and null is a no-op.
        unsafe { CoTaskMemFree(Some(self.0.cast())) };
    }
}

impl CoTaskMem<u16> {
    fn decode(&self) -> CoreResult<String> {
        if self.0.is_null() {
            return Ok(String::new());
        }

        // SAFETY: COM strings are NUL-terminated.
        unsafe { PWSTR(self.0).to_string() }
            .map_err(|e| PtpError::Malformed(format!("invalid UTF-16 from WPD: {e}")).into())
    }

    fn to_hstring(&self) -> HSTRING {
        if self.0.is_null() {
            return HSTRING::new();
        }

        // SAFETY: COM strings are NUL-terminated.
        unsafe { PWSTR(self.0).to_hstring() }
    }
}

pub fn device_ids() -> CoreResult<Vec<String>> {
    let manager: IPortableDeviceManager = create(&PortableDeviceManager)?;

    let mut count = 0;
    // SAFETY: A null buffer only queries the count.
    unsafe { manager.GetDevices(ptr::null_mut(), &raw mut count)? };
    if count == 0 {
        return Ok(Vec::new());
    }

    let mut ids = vec![PWSTR::null(); count as usize];
    // SAFETY: `ids` has room for `count` strings.
    unsafe { manager.GetDevices(ids.as_mut_ptr(), &raw mut count)? };
    ids.truncate(count as usize);

    let ids: Vec<_> = ids.into_iter().map(|id| CoTaskMem(id.0)).collect();
    ids.iter().map(CoTaskMem::decode).collect()
}

fn ui4(value: u32) -> PROPVARIANT {
    let mut tagged = PROPVARIANT_0_0 {
        vt: VT_UI4,
        ..Default::default()
    };
    tagged.Anonymous.ulVal = value;

    PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(tagged),
        },
    }
}

pub struct Values(IPortableDeviceValues);

impl Values {
    pub fn new() -> CoreResult<Self> {
        create(&PortableDeviceValues).map(Self)
    }

    pub fn for_command(command: &PROPERTYKEY) -> CoreResult<Self> {
        let values = Self::new()?;
        // SAFETY: Both pointers outlive the call.
        unsafe {
            values.0.SetGuidValue(
                &WPD_PROPERTY_COMMON_COMMAND_CATEGORY,
                &raw const command.fmtid,
            )?;
        }
        values.set_u32(&WPD_PROPERTY_COMMON_COMMAND_ID, command.pid)?;
        Ok(values)
    }

    pub fn set_u32(&self, key: &PROPERTYKEY, value: u32) -> CoreResult<()> {
        // SAFETY: `key` outlives the call.
        unsafe { self.0.SetUnsignedIntegerValue(key, value)? };
        Ok(())
    }

    pub fn set_u64(&self, key: &PROPERTYKEY, value: u64) -> CoreResult<()> {
        // SAFETY: `key` outlives the call.
        unsafe { self.0.SetUnsignedLargeIntegerValue(key, value)? };
        Ok(())
    }

    pub fn set_str(&self, key: &PROPERTYKEY, value: &HSTRING) -> CoreResult<()> {
        // SAFETY: `key` and `value` outlive the call.
        unsafe { self.0.SetStringValue(key, value)? };
        Ok(())
    }

    pub fn set_buffer(&self, key: &PROPERTYKEY, value: &[u8]) -> CoreResult<()> {
        // `SetBufferValue` panics on buffers over `u32::MAX` bytes.
        u32::try_from(value.len()).map_err(|_| CoreError::PayloadTooLarge(value.len()))?;
        // SAFETY: `key` and `value` outlive the call.
        unsafe { self.0.SetBufferValue(key, value)? };
        Ok(())
    }

    pub fn set_u32_list(&self, key: &PROPERTYKEY, values: &[u32]) -> CoreResult<()> {
        let collection: IPortableDevicePropVariantCollection =
            create(&PortableDevicePropVariantCollection)?;

        for &value in values {
            let variant = ui4(value);
            // SAFETY: `variant` outlives the call.
            unsafe { collection.Add(&raw const variant)? };
        }

        // SAFETY: `key` outlives the call.
        unsafe {
            self.0
                .SetIPortableDevicePropVariantCollectionValue(key, &collection)?;
        }
        Ok(())
    }

    pub fn u32(&self, key: &PROPERTYKEY) -> CoreResult<u32> {
        // SAFETY: `key` outlives the call.
        Ok(unsafe { self.0.GetUnsignedIntegerValue(key)? })
    }

    pub fn u64(&self, key: &PROPERTYKEY) -> CoreResult<u64> {
        // SAFETY: `key` outlives the call.
        Ok(unsafe { self.0.GetUnsignedLargeIntegerValue(key)? })
    }

    pub fn string(&self, key: &PROPERTYKEY) -> CoreResult<HSTRING> {
        // SAFETY: `key` outlives the call, and we take ownership of the result.
        let value = CoTaskMem(unsafe { self.0.GetStringValue(key)? }.0);
        Ok(value.to_hstring())
    }

    pub fn buffer(&self, key: &PROPERTYKEY) -> CoreResult<Vec<u8>> {
        let mut data = ptr::null_mut();
        let mut len = 0;
        // SAFETY: `key` and both out-pointers outlive the call.
        unsafe { self.0.GetBufferValue(key, &raw mut data, &raw mut len)? };

        let data = CoTaskMem(data);
        if data.0.is_null() {
            return Ok(Vec::new());
        }

        // SAFETY: COM returned `len` initialized bytes at `data`.
        Ok(unsafe { slice::from_raw_parts(data.0, len as usize) }.to_vec())
    }
}

pub struct PortableDevice(IPortableDevice);

impl PortableDevice {
    pub fn open(id: &str, client_info: &Values) -> CoreResult<Self> {
        let device: IPortableDevice = create(&PortableDeviceFTM)?;
        // SAFETY: Both arguments outlive the call.
        unsafe { device.Open(&HSTRING::from(id), &client_info.0)? };
        Ok(Self(device))
    }

    pub fn send(&self, parameters: &Values) -> CoreResult<Values> {
        // SAFETY: `parameters` outlives the call.
        let results = Values(unsafe { self.0.SendCommand(0, &parameters.0)? });

        // SAFETY: The key outlives the call.
        if let Ok(hr) = unsafe { results.0.GetErrorValue(&WPD_PROPERTY_COMMON_HRESULT) } {
            hr.ok()?;
        }

        Ok(results)
    }
}
