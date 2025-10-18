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
        FujiWhiteBalance, FujiWhiteBalanceShift, FujiWhiteBalanceTemperature, UsbMode,
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

    pub fn set_active_custom_setting(&mut self, slot: FujiCustomSetting) -> anyhow::Result<()> {
        self.r#impl.set_custom_setting_slot(&mut self.ptp, slot)
    }

    pub fn get_custom_setting_name(&mut self) -> anyhow::Result<FujiCustomSettingName> {
        self.r#impl.get_custom_setting_name(&mut self.ptp)
    }

    pub fn set_custom_setting_name(&mut self, value: &str) -> anyhow::Result<()> {
        self.r#impl.set_custom_setting_name(&mut self.ptp, value)
    }

    pub fn get_image_size(&mut self) -> anyhow::Result<FujiImageSize> {
        self.r#impl.get_image_size(&mut self.ptp)
    }

    pub fn set_image_size(&mut self, value: FujiImageSize) -> anyhow::Result<()> {
        self.r#impl.set_image_size(&mut self.ptp, value)
    }

    pub fn get_image_quality(&mut self) -> anyhow::Result<FujiImageQuality> {
        self.r#impl.get_image_quality(&mut self.ptp)
    }

    pub fn set_image_quality(&mut self, value: FujiImageQuality) -> anyhow::Result<()> {
        self.r#impl.set_image_quality(&mut self.ptp, value)
    }

    pub fn get_dynamic_range(&mut self) -> anyhow::Result<FujiDynamicRange> {
        self.r#impl.get_dynamic_range(&mut self.ptp)
    }

    pub fn set_dynamic_range(&mut self, value: FujiDynamicRange) -> anyhow::Result<()> {
        self.r#impl.set_dynamic_range(&mut self.ptp, value)
    }

    pub fn get_dynamic_range_priority(&mut self) -> anyhow::Result<FujiDynamicRangePriority> {
        self.r#impl.get_dynamic_range_priority(&mut self.ptp)
    }

    pub fn set_dynamic_range_priority(
        &mut self,
        value: FujiDynamicRangePriority,
    ) -> anyhow::Result<()> {
        self.r#impl.set_dynamic_range_priority(&mut self.ptp, value)
    }

    pub fn get_film_simulation(&mut self) -> anyhow::Result<FujiFilmSimulation> {
        self.r#impl.get_film_simulation(&mut self.ptp)
    }

    pub fn set_film_simulation(&mut self, value: FujiFilmSimulation) -> anyhow::Result<()> {
        self.r#impl.set_film_simulation(&mut self.ptp, value)
    }

    pub fn get_grain_effect(&mut self) -> anyhow::Result<FujiGrainEffect> {
        self.r#impl.get_grain_effect(&mut self.ptp)
    }

    pub fn set_grain_effect(&mut self, value: FujiGrainEffect) -> anyhow::Result<()> {
        self.r#impl.set_grain_effect(&mut self.ptp, value)
    }

    pub fn get_white_balance(&mut self) -> anyhow::Result<FujiWhiteBalance> {
        self.r#impl.get_white_balance(&mut self.ptp)
    }

    pub fn set_white_balance(&mut self, value: FujiWhiteBalance) -> anyhow::Result<()> {
        self.r#impl.set_white_balance(&mut self.ptp, value)
    }

    pub fn get_high_iso_nr(&mut self) -> anyhow::Result<FujiHighISONR> {
        self.r#impl.get_high_iso_nr(&mut self.ptp)
    }

    pub fn set_high_iso_nr(&mut self, value: FujiHighISONR) -> anyhow::Result<()> {
        self.r#impl.set_high_iso_nr(&mut self.ptp, value)
    }

    pub fn get_highlight_tone(&mut self) -> anyhow::Result<FujiHighlightTone> {
        self.r#impl.get_highlight_tone(&mut self.ptp)
    }

    pub fn set_highlight_tone(&mut self, value: FujiHighlightTone) -> anyhow::Result<()> {
        self.r#impl.set_highlight_tone(&mut self.ptp, value)
    }

    pub fn get_shadow_tone(&mut self) -> anyhow::Result<FujiShadowTone> {
        self.r#impl.get_shadow_tone(&mut self.ptp)
    }

    pub fn set_shadow_tone(&mut self, value: FujiShadowTone) -> anyhow::Result<()> {
        self.r#impl.set_shadow_tone(&mut self.ptp, value)
    }

    pub fn get_color(&mut self) -> anyhow::Result<FujiColor> {
        self.r#impl.get_color(&mut self.ptp)
    }

    pub fn set_color(&mut self, value: FujiColor) -> anyhow::Result<()> {
        self.r#impl.set_color(&mut self.ptp, value)
    }

    pub fn get_sharpness(&mut self) -> anyhow::Result<FujiSharpness> {
        self.r#impl.get_sharpness(&mut self.ptp)
    }

    pub fn set_sharpness(&mut self, value: FujiSharpness) -> anyhow::Result<()> {
        self.r#impl.set_sharpness(&mut self.ptp, value)
    }

    pub fn get_clarity(&mut self) -> anyhow::Result<FujiClarity> {
        self.r#impl.get_clarity(&mut self.ptp)
    }

    pub fn set_clarity(&mut self, value: FujiClarity) -> anyhow::Result<()> {
        self.r#impl.set_clarity(&mut self.ptp, value)
    }

    pub fn get_white_balance_shift_red(&mut self) -> anyhow::Result<FujiWhiteBalanceShift> {
        self.r#impl.get_white_balance_shift_red(&mut self.ptp)
    }

    pub fn set_white_balance_shift_red(
        &mut self,
        value: FujiWhiteBalanceShift,
    ) -> anyhow::Result<()> {
        self.r#impl
            .set_white_balance_shift_red(&mut self.ptp, value)
    }

    pub fn get_white_balance_shift_blue(&mut self) -> anyhow::Result<FujiWhiteBalanceShift> {
        self.r#impl.get_white_balance_shift_blue(&mut self.ptp)
    }

    pub fn set_white_balance_shift_blue(
        &mut self,
        value: FujiWhiteBalanceShift,
    ) -> anyhow::Result<()> {
        self.r#impl
            .set_white_balance_shift_blue(&mut self.ptp, value)
    }

    pub fn get_white_balance_temperature(&mut self) -> anyhow::Result<FujiWhiteBalanceTemperature> {
        self.r#impl.get_white_balance_temperature(&mut self.ptp)
    }

    pub fn set_white_balance_temperature(
        &mut self,
        value: FujiWhiteBalanceTemperature,
    ) -> anyhow::Result<()> {
        self.r#impl
            .set_white_balance_temperature(&mut self.ptp, value)
    }

    pub fn get_color_chrome_effect(&mut self) -> anyhow::Result<FujiColorChromeEffect> {
        self.r#impl.get_color_chrome_effect(&mut self.ptp)
    }

    pub fn set_color_chrome_effect(&mut self, value: FujiColorChromeEffect) -> anyhow::Result<()> {
        self.r#impl.set_color_chrome_effect(&mut self.ptp, value)
    }

    pub fn get_color_chrome_fx_blue(&mut self) -> anyhow::Result<FujiColorChromeFXBlue> {
        self.r#impl.get_color_chrome_fx_blue(&mut self.ptp)
    }

    pub fn set_color_chrome_fx_blue(&mut self, value: FujiColorChromeFXBlue) -> anyhow::Result<()> {
        self.r#impl.set_color_chrome_fx_blue(&mut self.ptp, value)
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

    fn set_custom_setting_slot(
        &self,
        ptp: &mut Ptp,
        slot: FujiCustomSetting,
    ) -> anyhow::Result<()> {
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSetting,
            &slot.try_into_ptp()?,
        )?;
        Ok(())
    }

    fn get_custom_setting_name(&self, ptp: &mut Ptp) -> anyhow::Result<FujiCustomSettingName> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingName)?;
        let result = FujiCustomSettingName::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_custom_setting_name(&self, ptp: &mut Ptp, value: &str) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(ptp, DevicePropCode::FujiStillCustomSettingName, &bytes)?;
        Ok(())
    }

    fn get_image_size(&self, ptp: &mut Ptp) -> anyhow::Result<FujiImageSize> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingImageSize)?;
        let result = FujiImageSize::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_image_size(&self, ptp: &mut Ptp, value: FujiImageSize) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(ptp, DevicePropCode::FujiStillCustomSettingImageSize, &bytes)?;
        Ok(())
    }

    fn get_image_quality(&self, ptp: &mut Ptp) -> anyhow::Result<FujiImageQuality> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingImageQuality)?;
        let result = FujiImageQuality::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_image_quality(&self, ptp: &mut Ptp, value: FujiImageQuality) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingImageQuality,
            &bytes,
        )?;
        Ok(())
    }

    fn get_dynamic_range(&self, ptp: &mut Ptp) -> anyhow::Result<FujiDynamicRange> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingDynamicRange)?;
        let result = FujiDynamicRange::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_dynamic_range(&self, ptp: &mut Ptp, value: FujiDynamicRange) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingDynamicRange,
            &bytes,
        )?;
        Ok(())
    }

    fn get_dynamic_range_priority(
        &self,
        ptp: &mut Ptp,
    ) -> anyhow::Result<FujiDynamicRangePriority> {
        let bytes = self.get_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingDynamicRangePriority,
        )?;
        let result = FujiDynamicRangePriority::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_dynamic_range_priority(
        &self,
        ptp: &mut Ptp,
        value: FujiDynamicRangePriority,
    ) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingDynamicRangePriority,
            &bytes,
        )?;
        Ok(())
    }

    fn get_film_simulation(&self, ptp: &mut Ptp) -> anyhow::Result<FujiFilmSimulation> {
        let bytes =
            self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingFilmSimulation)?;
        let result = FujiFilmSimulation::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_film_simulation(&self, ptp: &mut Ptp, value: FujiFilmSimulation) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingFilmSimulation,
            &bytes,
        )?;
        Ok(())
    }

    fn get_grain_effect(&self, ptp: &mut Ptp) -> anyhow::Result<FujiGrainEffect> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingGrainEffect)?;
        let result = FujiGrainEffect::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_grain_effect(&self, ptp: &mut Ptp, value: FujiGrainEffect) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingGrainEffect,
            &bytes,
        )?;
        Ok(())
    }

    fn get_white_balance(&self, ptp: &mut Ptp) -> anyhow::Result<FujiWhiteBalance> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingWhiteBalance)?;
        let result = FujiWhiteBalance::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_white_balance(&self, ptp: &mut Ptp, value: FujiWhiteBalance) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingWhiteBalance,
            &bytes,
        )?;
        Ok(())
    }

    fn get_high_iso_nr(&self, ptp: &mut Ptp) -> anyhow::Result<FujiHighISONR> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingHighISONR)?;
        let result = FujiHighISONR::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_high_iso_nr(&self, ptp: &mut Ptp, value: FujiHighISONR) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(ptp, DevicePropCode::FujiStillCustomSettingHighISONR, &bytes)?;
        Ok(())
    }

    fn get_highlight_tone(&self, ptp: &mut Ptp) -> anyhow::Result<FujiHighlightTone> {
        let bytes =
            self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingHighlightTone)?;
        let result = FujiHighlightTone::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_highlight_tone(&self, ptp: &mut Ptp, value: FujiHighlightTone) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingHighlightTone,
            &bytes,
        )?;
        Ok(())
    }

    fn get_shadow_tone(&self, ptp: &mut Ptp) -> anyhow::Result<FujiShadowTone> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingShadowTone)?;
        let result = FujiShadowTone::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_shadow_tone(&self, ptp: &mut Ptp, value: FujiShadowTone) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingShadowTone,
            &bytes,
        )?;
        Ok(())
    }

    fn get_color(&self, ptp: &mut Ptp) -> anyhow::Result<FujiColor> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingColor)?;
        let result = FujiColor::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_color(&self, ptp: &mut Ptp, value: FujiColor) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(ptp, DevicePropCode::FujiStillCustomSettingColor, &bytes)?;
        Ok(())
    }

    fn get_sharpness(&self, ptp: &mut Ptp) -> anyhow::Result<FujiSharpness> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingSharpness)?;
        let result = FujiSharpness::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_sharpness(&self, ptp: &mut Ptp, value: FujiSharpness) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(ptp, DevicePropCode::FujiStillCustomSettingSharpness, &bytes)?;
        Ok(())
    }

    fn get_clarity(&self, ptp: &mut Ptp) -> anyhow::Result<FujiClarity> {
        let bytes = self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingClarity)?;
        let result = FujiClarity::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_clarity(&self, ptp: &mut Ptp, value: FujiClarity) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(ptp, DevicePropCode::FujiStillCustomSettingClarity, &bytes)?;
        Ok(())
    }

    fn get_white_balance_shift_red(&self, ptp: &mut Ptp) -> anyhow::Result<FujiWhiteBalanceShift> {
        let bytes = self.get_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingWhiteBalanceShiftRed,
        )?;
        let result = FujiWhiteBalanceShift::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_white_balance_shift_red(
        &self,
        ptp: &mut Ptp,
        value: FujiWhiteBalanceShift,
    ) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingWhiteBalanceShiftRed,
            &bytes,
        )?;
        Ok(())
    }

    fn get_white_balance_shift_blue(&self, ptp: &mut Ptp) -> anyhow::Result<FujiWhiteBalanceShift> {
        let bytes = self.get_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingWhiteBalanceShiftBlue,
        )?;
        let result = FujiWhiteBalanceShift::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_white_balance_shift_blue(
        &self,
        ptp: &mut Ptp,
        value: FujiWhiteBalanceShift,
    ) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingWhiteBalanceShiftBlue,
            &bytes,
        )?;
        Ok(())
    }

    fn get_white_balance_temperature(
        &self,
        ptp: &mut Ptp,
    ) -> anyhow::Result<FujiWhiteBalanceTemperature> {
        let bytes = self.get_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingWhiteBalanceTemperature,
        )?;
        let result = FujiWhiteBalanceTemperature::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_white_balance_temperature(
        &self,
        ptp: &mut Ptp,
        value: FujiWhiteBalanceTemperature,
    ) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingWhiteBalanceTemperature,
            &bytes,
        )?;
        Ok(())
    }

    fn get_color_chrome_effect(&self, ptp: &mut Ptp) -> anyhow::Result<FujiColorChromeEffect> {
        let bytes =
            self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingColorChromeEffect)?;
        let result = FujiColorChromeEffect::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_color_chrome_effect(
        &self,
        ptp: &mut Ptp,
        value: FujiColorChromeEffect,
    ) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingColorChromeEffect,
            &bytes,
        )?;
        Ok(())
    }

    fn get_color_chrome_fx_blue(&self, ptp: &mut Ptp) -> anyhow::Result<FujiColorChromeFXBlue> {
        let bytes =
            self.get_prop_value(ptp, DevicePropCode::FujiStillCustomSettingColorChromeFXBlue)?;
        let result = FujiColorChromeFXBlue::try_from_ptp(&bytes)?;
        Ok(result)
    }

    fn set_color_chrome_fx_blue(
        &self,
        ptp: &mut Ptp,
        value: FujiColorChromeFXBlue,
    ) -> anyhow::Result<()> {
        let bytes = value.try_into_ptp()?;
        self.set_prop_value(
            ptp,
            DevicePropCode::FujiStillCustomSettingColorChromeFXBlue,
            &bytes,
        )?;
        Ok(())
    }
}
