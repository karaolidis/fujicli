pub mod devices;
pub mod error;
pub mod ptp;

use std::time::Duration;

use anyhow::{anyhow, bail};
use devices::SupportedCamera;
use log::{debug, error};
use ptp::{
    Ptp,
    hex::{
        CommandCode, DevicePropCode, FujiClarity, FujiColor, FujiColorChromeEffect,
        FujiColorChromeFXBlue, FujiCustomSetting, FujiCustomSettingName, FujiDynamicRange,
        FujiDynamicRangePriority, FujiFilmSimulation, FujiGrainEffect, FujiHighISONR,
        FujiHighlightTone, FujiImageQuality, FujiImageSize, FujiShadowTone, FujiSharpness,
        FujiSmoothSkinEffect, FujiWhiteBalance, FujiWhiteBalanceShift, FujiWhiteBalanceTemperature,
        UsbMode,
    },
    structs::DeviceInfo,
};
use ptp_cursor::{PtpDeserialize, PtpSerialize};
use rusb::{GlobalContext, constants::LIBUSB_CLASS_IMAGE};

use crate::usb::find_endpoint;

const SESSION: u32 = 1;

pub struct Camera {
    r#impl: Box<dyn CameraImpl<GlobalContext>>,
    ptp: Ptp,
}

macro_rules! camera_custom_settings {
    ($( $name:ident : $type:ty => $code:expr ),+ $(,)?) => {
        $(
            paste::paste! {
                #[allow(dead_code)]
                pub fn [<get_ $name>](&mut self) -> anyhow::Result<$type> {
                    self.r#impl.[<get_ $name>](&mut self.ptp)
                }

                #[allow(dead_code)]
                pub fn [<set_ $name>](&mut self, value: &$type) -> anyhow::Result<()> {
                    self.r#impl.[<set_ $name>](&mut self.ptp, value)
                }
            }
        )+
    };
}

impl Camera {
    pub fn name(&self) -> &'static str {
        self.r#impl.supported_camera().name
    }

    pub fn vendor_id(&self) -> u16 {
        self.r#impl.supported_camera().vendor
    }

    pub fn product_id(&self) -> u16 {
        self.r#impl.supported_camera().product
    }

    pub fn connected_usb_id(&self) -> String {
        format!("{}.{}", self.ptp.bus, self.ptp.address)
    }

    pub fn get_info(&mut self) -> anyhow::Result<DeviceInfo> {
        let info = self.r#impl.get_info(&mut self.ptp)?;
        Ok(info)
    }

    pub fn get_usb_mode(&mut self) -> anyhow::Result<UsbMode> {
        let data = self
            .r#impl
            .get_prop_value(&mut self.ptp, DevicePropCode::FujiUsbMode)?;
        let result = UsbMode::try_from_ptp(&data)?;
        Ok(result)
    }

    pub fn get_battery_info(&mut self) -> anyhow::Result<u32> {
        let data = self
            .r#impl
            .get_prop_value(&mut self.ptp, DevicePropCode::FujiBatteryInfo2)?;

        debug!("Raw battery data: {data:?}");

        let raw_string = String::try_from_ptp(&data)?;
        debug!("Decoded raw string: {raw_string}");

        let percentage: u32 = raw_string
            .split(',')
            .next()
            .ok_or_else(|| anyhow!("Failed to parse battery percentage"))?
            .parse()?;

        Ok(percentage)
    }

    pub fn export_backup(&mut self) -> anyhow::Result<Vec<u8>> {
        self.r#impl.export_backup(&mut self.ptp)
    }

    pub fn import_backup(&mut self, backup: &[u8]) -> anyhow::Result<()> {
        self.r#impl.import_backup(&mut self.ptp, backup)
    }

    camera_custom_settings! {
        active_custom_setting: FujiCustomSetting => DevicePropCode::FujiStillCustomSetting,
        custom_setting_name: FujiCustomSettingName => DevicePropCode::FujiStillCustomSettingName,
        image_size: FujiImageSize => DevicePropCode::FujiStillCustomSettingImageSize,
        image_quality: FujiImageQuality => DevicePropCode::FujiStillCustomSettingImageQuality,
        dynamic_range: FujiDynamicRange => DevicePropCode::FujiStillCustomSettingDynamicRange,
        dynamic_range_priority: FujiDynamicRangePriority => DevicePropCode::FujiStillCustomSettingDynamicRangePriority,
        film_simulation: FujiFilmSimulation => DevicePropCode::FujiStillCustomSettingFilmSimulation,
        grain_effect: FujiGrainEffect => DevicePropCode::FujiStillCustomSettingGrainEffect,
        white_balance: FujiWhiteBalance => DevicePropCode::FujiStillCustomSettingWhiteBalance,
        high_iso_nr: FujiHighISONR => DevicePropCode::FujiStillCustomSettingHighISONR,
        highlight_tone: FujiHighlightTone => DevicePropCode::FujiStillCustomSettingHighlightTone,
        shadow_tone: FujiShadowTone => DevicePropCode::FujiStillCustomSettingShadowTone,
        color: FujiColor => DevicePropCode::FujiStillCustomSettingColor,
        sharpness: FujiSharpness => DevicePropCode::FujiStillCustomSettingSharpness,
        clarity: FujiClarity => DevicePropCode::FujiStillCustomSettingClarity,
        white_balance_shift_red: FujiWhiteBalanceShift => DevicePropCode::FujiStillCustomSettingWhiteBalanceShiftRed,
        white_balance_shift_blue: FujiWhiteBalanceShift => DevicePropCode::FujiStillCustomSettingWhiteBalanceShiftBlue,
        white_balance_temperature: FujiWhiteBalanceTemperature => DevicePropCode::FujiStillCustomSettingWhiteBalanceTemperature,
        color_chrome_effect: FujiColorChromeEffect => DevicePropCode::FujiStillCustomSettingColorChromeEffect,
        color_chrome_fx_blue: FujiColorChromeFXBlue => DevicePropCode::FujiStillCustomSettingColorChromeFXBlue,
        smooth_skin_effect: FujiSmoothSkinEffect => DevicePropCode::FujiStillCustomSettingSmoothSkinEffect,
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        debug!("Closing session");
        if let Err(e) = self.r#impl.close_session(&mut self.ptp, SESSION) {
            error!("Error closing session: {e}");
        }
        debug!("Session closed");
    }
}

impl TryFrom<&rusb::Device<GlobalContext>> for Camera {
    type Error = anyhow::Error;

    fn try_from(device: &rusb::Device<GlobalContext>) -> anyhow::Result<Self> {
        let descriptor = device.device_descriptor()?;

        let vendor = descriptor.vendor_id();
        let product = descriptor.product_id();

        for supported_camera in devices::SUPPORTED {
            if vendor != supported_camera.vendor || product != supported_camera.product {
                continue;
            }

            let r#impl = (supported_camera.impl_factory)();

            let bus = device.bus_number();
            let address = device.address();

            let config_descriptor = device.active_config_descriptor()?;

            let interface_descriptor = config_descriptor
                .interfaces()
                .flat_map(|i| i.descriptors())
                .find(|x| x.class_code() == LIBUSB_CLASS_IMAGE)
                .ok_or(rusb::Error::NotFound)?;

            let interface = interface_descriptor.interface_number();
            debug!("Found interface {interface}");

            let handle = device.open()?;
            handle.claim_interface(interface)?;

            let bulk_in = find_endpoint(
                &interface_descriptor,
                rusb::Direction::In,
                rusb::TransferType::Bulk,
            )?;
            let bulk_out = find_endpoint(
                &interface_descriptor,
                rusb::Direction::Out,
                rusb::TransferType::Bulk,
            )?;

            let transaction_id = 0;

            let chunk_size = r#impl.chunk_size();

            let mut ptp = Ptp {
                bus,
                address,
                interface,
                bulk_in,
                bulk_out,
                handle,
                transaction_id,
                chunk_size,
            };

            debug!("Opening session");
            let () = r#impl.open_session(&mut ptp, SESSION)?;
            debug!("Session opened");

            return Ok(Self { r#impl, ptp });
        }

        bail!("Device not supported");
    }
}

macro_rules! camera_impl_custom_settings {
    ($( $name:ident : $type:ty => $code:expr ),+ $(,)?) => {
        $(
            paste::paste! {
                #[allow(dead_code)]
                fn [<get_ $name>](&self, ptp: &mut Ptp) -> anyhow::Result<$type> {
                    let bytes = self.get_prop_value(ptp, $code)?;
                    let result = <$type>::try_from_ptp(&bytes)?;
                    Ok(result)
                }

                #[allow(dead_code)]
                fn [<set_ $name>](&self, ptp: &mut Ptp, value: &$type) -> anyhow::Result<()> {
                    let bytes = value.try_into_ptp()?;
                    self.set_prop_value(ptp, $code, &bytes)?;
                    Ok(())
                }
            }
        )+
    };
}

pub trait CameraImpl<P: rusb::UsbContext> {
    fn supported_camera(&self) -> &'static SupportedCamera<P>;

    fn timeout(&self) -> Duration {
        Duration::default()
    }

    fn chunk_size(&self) -> usize {
        1024 * 1024
    }

    fn open_session(&self, ptp: &mut Ptp, session_id: u32) -> anyhow::Result<()> {
        debug!("Sending OpenSession command");
        _ = ptp.send(
            CommandCode::OpenSession,
            &[session_id],
            None,
            self.timeout(),
        )?;
        Ok(())
    }

    fn close_session(&self, ptp: &mut Ptp, _: u32) -> anyhow::Result<()> {
        debug!("Sending CloseSession command");
        _ = ptp.send(CommandCode::CloseSession, &[], None, self.timeout())?;
        Ok(())
    }

    fn get_info(&self, ptp: &mut Ptp) -> anyhow::Result<DeviceInfo> {
        debug!("Sending GetDeviceInfo command");
        let response = ptp.send(CommandCode::GetDeviceInfo, &[], None, self.timeout())?;
        debug!("Received response with {} bytes", response.len());
        let info = DeviceInfo::try_from_ptp(&response)?;
        Ok(info)
    }

    fn get_prop_value(&self, ptp: &mut Ptp, prop: DevicePropCode) -> anyhow::Result<Vec<u8>> {
        debug!("Sending GetDevicePropValue command for property {prop:?}");
        let response = ptp.send(
            CommandCode::GetDevicePropValue,
            &[prop.into()],
            None,
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());
        Ok(response)
    }

    fn set_prop_value(
        &self,
        ptp: &mut Ptp,
        prop: DevicePropCode,
        value: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        debug!("Sending GetDevicePropValue command for property {prop:?}");
        let response = ptp.send(
            CommandCode::SetDevicePropValue,
            &[prop.into()],
            Some(value),
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());
        Ok(response)
    }

    fn export_backup(&self, ptp: &mut Ptp) -> anyhow::Result<Vec<u8>> {
        const HANDLE: u32 = 0x0;

        debug!("Sending GetObjectInfo command for backup");
        let response = ptp.send(CommandCode::GetObjectInfo, &[HANDLE], None, self.timeout())?;
        debug!("Received response with {} bytes", response.len());

        debug!("Sending GetObject command for backup");
        let response = ptp.send(CommandCode::GetObject, &[HANDLE], None, self.timeout())?;
        debug!("Received response with {} bytes", response.len());

        Ok(response)
    }

    fn import_backup(&self, ptp: &mut Ptp, buffer: &[u8]) -> anyhow::Result<()> {
        debug!("Preparing ObjectInfo header for backup");

        let mut header = Vec::with_capacity(1076);
        0x0u32.try_write_ptp(&mut header)?;
        0x5000u16.try_write_ptp(&mut header)?;
        0x0u16.try_write_ptp(&mut header)?;
        u32::try_from(buffer.len())?.try_write_ptp(&mut header)?;
        for _ in 0..1064 {
            0x0u8.try_write_ptp(&mut header)?;
        }

        debug!("Sending SendObjectInfo command for backup");
        let response = ptp.send(
            CommandCode::SendObjectInfo,
            &[0x0, 0x0],
            Some(&header),
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());

        debug!("Sending SendObject command for backup");
        let response = ptp.send(
            CommandCode::SendObject,
            &[0x0],
            Some(buffer),
            self.timeout(),
        )?;
        debug!("Received response with {} bytes", response.len());

        Ok(())
    }

    camera_impl_custom_settings! {
        active_custom_setting: FujiCustomSetting => DevicePropCode::FujiStillCustomSetting,
        custom_setting_name: FujiCustomSettingName => DevicePropCode::FujiStillCustomSettingName,
        image_size: FujiImageSize => DevicePropCode::FujiStillCustomSettingImageSize,
        image_quality: FujiImageQuality => DevicePropCode::FujiStillCustomSettingImageQuality,
        dynamic_range: FujiDynamicRange => DevicePropCode::FujiStillCustomSettingDynamicRange,
        dynamic_range_priority: FujiDynamicRangePriority => DevicePropCode::FujiStillCustomSettingDynamicRangePriority,
        film_simulation: FujiFilmSimulation => DevicePropCode::FujiStillCustomSettingFilmSimulation,
        grain_effect: FujiGrainEffect => DevicePropCode::FujiStillCustomSettingGrainEffect,
        white_balance: FujiWhiteBalance => DevicePropCode::FujiStillCustomSettingWhiteBalance,
        high_iso_nr: FujiHighISONR => DevicePropCode::FujiStillCustomSettingHighISONR,
        highlight_tone: FujiHighlightTone => DevicePropCode::FujiStillCustomSettingHighlightTone,
        shadow_tone: FujiShadowTone => DevicePropCode::FujiStillCustomSettingShadowTone,
        color: FujiColor => DevicePropCode::FujiStillCustomSettingColor,
        sharpness: FujiSharpness => DevicePropCode::FujiStillCustomSettingSharpness,
        clarity: FujiClarity => DevicePropCode::FujiStillCustomSettingClarity,
        white_balance_shift_red: FujiWhiteBalanceShift => DevicePropCode::FujiStillCustomSettingWhiteBalanceShiftRed,
        white_balance_shift_blue: FujiWhiteBalanceShift => DevicePropCode::FujiStillCustomSettingWhiteBalanceShiftBlue,
        white_balance_temperature: FujiWhiteBalanceTemperature => DevicePropCode::FujiStillCustomSettingWhiteBalanceTemperature,
        color_chrome_effect: FujiColorChromeEffect => DevicePropCode::FujiStillCustomSettingColorChromeEffect,
        color_chrome_fx_blue: FujiColorChromeFXBlue => DevicePropCode::FujiStillCustomSettingColorChromeFXBlue,
        smooth_skin_effect: FujiSmoothSkinEffect => DevicePropCode::FujiStillCustomSettingSmoothSkinEffect,
    }
}
