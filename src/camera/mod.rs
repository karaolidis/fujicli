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
        FujiColorChromeFXBlue, FujiColorSpace, FujiCustomSetting, FujiCustomSettingName,
        FujiDynamicRange, FujiDynamicRangePriority, FujiFilmSimulation, FujiGrainEffect,
        FujiHighISONR, FujiHighlightTone, FujiImageQuality, FujiImageSize,
        FujiLensModulationOptimizer, FujiMonochromaticColorTemperature, FujiMonochromaticColorTint,
        FujiShadowTone, FujiSharpness, FujiSmoothSkinEffect, FujiWhiteBalance,
        FujiWhiteBalanceShift, FujiWhiteBalanceTemperature, ObjectFormat, UsbMode,
    },
    structs::{DeviceInfo, ObjectInfo},
};
use ptp_cursor::{PtpDeserialize, PtpSerialize};
use rusb::{GlobalContext, constants::LIBUSB_CLASS_IMAGE};

use crate::usb::find_endpoint;

const SESSION: u32 = 1;

pub struct Camera {
    pub r#impl: Box<dyn CameraImpl<GlobalContext>>,
    pub ptp: Ptp,
}

macro_rules! camera_with_ptp {
    ($($fn_name:ident => $ret:ty),* $(,)?) => {
        $(
            #[allow(dead_code)]
            pub fn $fn_name(&mut self) -> anyhow::Result<$ret> {
                self.r#impl.$fn_name(&mut self.ptp)
            }
        )*
    };
    ($($fn_name:ident($($arg:ident : $arg_ty:ty),*) => $ret:ty),* $(,)?) => {
        $(
            #[allow(dead_code)]
            pub fn $fn_name(&mut self, $($arg: $arg_ty),*) -> anyhow::Result<$ret> {
                self.r#impl.$fn_name(&mut self.ptp, $($arg),*)
            }
        )*
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

    camera_with_ptp! {
        get_info => DeviceInfo,
        get_usb_mode => UsbMode,
        get_battery_info => u32,
    }

    camera_with_ptp! {
        export_backup => Vec<u8>,
        get_active_custom_setting => FujiCustomSetting,
        get_custom_setting_name => FujiCustomSettingName,
        get_image_size => FujiImageSize,
        get_image_quality => FujiImageQuality,
        get_dynamic_range => FujiDynamicRange,
        get_dynamic_range_priority => FujiDynamicRangePriority,
        get_film_simulation => FujiFilmSimulation,
        get_monochromatic_color_temperature => FujiMonochromaticColorTemperature,
        get_monochromatic_color_tint => FujiMonochromaticColorTint,
        get_grain_effect => FujiGrainEffect,
        get_white_balance => FujiWhiteBalance,
        get_high_iso_nr => FujiHighISONR,
        get_highlight_tone => FujiHighlightTone,
        get_shadow_tone => FujiShadowTone,
        get_color => FujiColor,
        get_sharpness => FujiSharpness,
        get_clarity => FujiClarity,
        get_white_balance_shift_red => FujiWhiteBalanceShift,
        get_white_balance_shift_blue => FujiWhiteBalanceShift,
        get_white_balance_temperature => FujiWhiteBalanceTemperature,
        get_color_chrome_effect => FujiColorChromeEffect,
        get_color_chrome_fx_blue => FujiColorChromeFXBlue,
        get_smooth_skin_effect => FujiSmoothSkinEffect,
        get_lens_modulation_optimizer => FujiLensModulationOptimizer,
        get_color_space => FujiColorSpace,
    }

    camera_with_ptp! {
        import_backup(buffer: &[u8]) => (),
        set_active_custom_setting(value: &FujiCustomSetting) => (),
        set_custom_setting_name(value: &FujiCustomSettingName) => (),
        set_image_size(value: &FujiImageSize) => (),
        set_image_quality(value: &FujiImageQuality) => (),
        set_dynamic_range(value: &FujiDynamicRange) => (),
        set_dynamic_range_priority(value: &FujiDynamicRangePriority) => (),
        set_film_simulation(value: &FujiFilmSimulation) => (),
        set_monochromatic_color_temperature(value: &FujiMonochromaticColorTemperature) => (),
        set_monochromatic_color_tint(value: &FujiMonochromaticColorTint) => (),
        set_grain_effect(value: &FujiGrainEffect) => (),
        set_white_balance(value: &FujiWhiteBalance) => (),
        set_high_iso_nr(value: &FujiHighISONR) => (),
        set_highlight_tone(value: &FujiHighlightTone) => (),
        set_shadow_tone(value: &FujiShadowTone) => (),
        set_color(value: &FujiColor) => (),
        set_sharpness(value: &FujiSharpness) => (),
        set_clarity(value: &FujiClarity) => (),
        set_white_balance_shift_red(value: &FujiWhiteBalanceShift) => (),
        set_white_balance_shift_blue(value: &FujiWhiteBalanceShift) => (),
        set_white_balance_temperature(value: &FujiWhiteBalanceTemperature) => (),
        set_color_chrome_effect(value: &FujiColorChromeEffect) => (),
        set_color_chrome_fx_blue(value: &FujiColorChromeFXBlue) => (),
        set_smooth_skin_effect(value: &FujiSmoothSkinEffect) => (),
        set_lens_modulation_optimizer(value: &FujiLensModulationOptimizer) => (),
        set_color_space(value: &FujiColorSpace) => (),
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        debug!("Closing session");
        if let Err(e) = self.ptp.close_session(SESSION, self.r#impl.timeout()) {
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
            let () = ptp.open_session(SESSION, r#impl.timeout())?;
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
                    let bytes = ptp.get_prop_value($code, self.timeout())?;
                    let result = <$type>::try_from_ptp(&bytes)?;
                    Ok(result)
                }

                #[allow(dead_code)]
                fn [<set_ $name>](&self, ptp: &mut Ptp, value: &$type) -> anyhow::Result<()> {
                    let bytes = value.try_into_ptp()?;
                    ptp.set_prop_value($code, &bytes, self.timeout())?;
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
        // Conservative estimate. Could go up to 15.75 * 1024^2 on the X-T5 but only gained 200ms.
        1024 * 1024
    }

    fn get_info(&mut self, ptp: &mut Ptp) -> anyhow::Result<DeviceInfo> {
        let info = ptp.get_info(self.timeout())?;
        Ok(info)
    }

    fn get_usb_mode(&mut self, ptp: &mut Ptp) -> anyhow::Result<UsbMode> {
        let data = ptp.get_prop_value(DevicePropCode::FujiUsbMode, self.timeout())?;
        let result = UsbMode::try_from_ptp(&data)?;
        Ok(result)
    }

    fn get_battery_info(&mut self, ptp: &mut Ptp) -> anyhow::Result<u32> {
        let data = ptp.get_prop_value(DevicePropCode::FujiBatteryInfo2, self.timeout())?;

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

        let object_info = ObjectInfo {
            object_format: ObjectFormat::FujiBackup,
            compressed_size: u32::try_from(buffer.len())?,
            ..Default::default()
        };
        object_info.try_write_ptp(&mut header)?;
        // TODO: What is this?
        for _ in 0..1020 {
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
        active_custom_setting: FujiCustomSetting => DevicePropCode::FujiCustomSetting,
        custom_setting_name: FujiCustomSettingName => DevicePropCode::FujiCustomSettingName,
        image_size: FujiImageSize => DevicePropCode::FujiCustomSettingImageSize,
        image_quality: FujiImageQuality => DevicePropCode::FujiCustomSettingImageQuality,
        dynamic_range: FujiDynamicRange => DevicePropCode::FujiCustomSettingDynamicRange,
        dynamic_range_priority: FujiDynamicRangePriority => DevicePropCode::FujiCustomSettingDynamicRangePriority,
        film_simulation: FujiFilmSimulation => DevicePropCode::FujiCustomSettingFilmSimulation,
        monochromatic_color_temperature: FujiMonochromaticColorTemperature => DevicePropCode::FujiCustomSettingMonochromaticColorTemperature,
        monochromatic_color_tint: FujiMonochromaticColorTint => DevicePropCode::FujiCustomSettingMonochromaticColorTint,
        grain_effect: FujiGrainEffect => DevicePropCode::FujiCustomSettingGrainEffect,
        white_balance: FujiWhiteBalance => DevicePropCode::FujiCustomSettingWhiteBalance,
        high_iso_nr: FujiHighISONR => DevicePropCode::FujiCustomSettingHighISONR,
        highlight_tone: FujiHighlightTone => DevicePropCode::FujiCustomSettingHighlightTone,
        shadow_tone: FujiShadowTone => DevicePropCode::FujiCustomSettingShadowTone,
        color: FujiColor => DevicePropCode::FujiCustomSettingColor,
        sharpness: FujiSharpness => DevicePropCode::FujiCustomSettingSharpness,
        clarity: FujiClarity => DevicePropCode::FujiCustomSettingClarity,
        white_balance_shift_red: FujiWhiteBalanceShift => DevicePropCode::FujiCustomSettingWhiteBalanceShiftRed,
        white_balance_shift_blue: FujiWhiteBalanceShift => DevicePropCode::FujiCustomSettingWhiteBalanceShiftBlue,
        white_balance_temperature: FujiWhiteBalanceTemperature => DevicePropCode::FujiCustomSettingWhiteBalanceTemperature,
        color_chrome_effect: FujiColorChromeEffect => DevicePropCode::FujiCustomSettingColorChromeEffect,
        color_chrome_fx_blue: FujiColorChromeFXBlue => DevicePropCode::FujiCustomSettingColorChromeFXBlue,
        smooth_skin_effect: FujiSmoothSkinEffect => DevicePropCode::FujiCustomSettingSmoothSkinEffect,
        lens_modulation_optimizer: FujiLensModulationOptimizer => DevicePropCode::FujiCustomSettingLensModulationOptimizer,
        color_space: FujiColorSpace => DevicePropCode::FujiCustomSettingColorSpace,
    }
}
