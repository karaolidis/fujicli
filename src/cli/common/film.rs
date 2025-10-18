use clap::Args;

use crate::camera::ptp::hex::{
    FujiClarity, FujiColor, FujiColorChromeEffect, FujiColorChromeFXBlue, FujiCustomSettingName,
    FujiDynamicRange, FujiDynamicRangePriority, FujiFilmSimulation, FujiGrainEffect, FujiHighISONR,
    FujiHighlightTone, FujiImageQuality, FujiImageSize, FujiShadowTone, FujiSharpness,
    FujiWhiteBalance, FujiWhiteBalanceShift, FujiWhiteBalanceTemperature,
};

#[derive(Args, Debug)]
pub struct FilmSimulationOptions {
    /// The name of the slot
    #[clap(long)]
    pub name: Option<FujiCustomSettingName>,

    /// The Fujifilm film simulation to use
    #[clap(long)]
    pub simulation: Option<FujiFilmSimulation>,

    /// The output image resolution
    #[clap(long)]
    pub size: Option<FujiImageSize>,

    /// The output image quality (JPEG compression level)
    #[clap(long)]
    pub quality: Option<FujiImageQuality>,

    /// Highlight Tone
    #[clap(long, allow_hyphen_values(true))]
    pub highlight: Option<FujiHighlightTone>,

    /// Shadow Tone
    #[clap(long, allow_hyphen_values(true))]
    pub shadow: Option<FujiShadowTone>,

    /// Color
    #[clap(long, allow_hyphen_values(true))]
    pub color: Option<FujiColor>,

    /// Sharpness
    #[clap(long, allow_hyphen_values(true))]
    pub sharpness: Option<FujiSharpness>,

    /// Clarity
    #[clap(long, allow_hyphen_values(true))]
    pub clarity: Option<FujiClarity>,

    /// White Balance
    #[clap(long)]
    pub white_balance: Option<FujiWhiteBalance>,

    /// White Balance Shift Red
    #[clap(long, allow_hyphen_values(true))]
    pub white_balance_shift_red: Option<FujiWhiteBalanceShift>,

    /// White Balance Shift Blue
    #[clap(long, allow_hyphen_values(true))]
    pub white_balance_shift_blue: Option<FujiWhiteBalanceShift>,

    /// White Balance Temperature (Only used if WB is set to 'Temperature')
    #[clap(long)]
    pub white_balance_temperature: Option<FujiWhiteBalanceTemperature>,

    /// Dynamic Range
    #[clap(long)]
    pub dynamic_range: Option<FujiDynamicRange>,

    /// Dynamic Range Priority
    #[clap(long)]
    pub dynamic_range_priority: Option<FujiDynamicRangePriority>,

    /// High ISO Noise Reduction
    #[clap(long, allow_hyphen_values(true))]
    pub noise_reduction: Option<FujiHighISONR>,

    /// Grain Effect
    #[clap(long)]
    pub grain: Option<FujiGrainEffect>,

    /// Color Chrome Effect
    #[clap(long)]
    pub color_chrome_effect: Option<FujiColorChromeEffect>,

    /// Color Chrome FX Blue
    #[clap(long)]
    pub color_chrome_fx_blue: Option<FujiColorChromeFXBlue>,
}
