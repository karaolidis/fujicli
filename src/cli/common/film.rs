use clap::Args;

use crate::camera::ptp::hex::{
    FujiClarity, FujiColor, FujiColorChromeEffect, FujiColorChromeFXBlue, FujiDynamicRange,
    FujiFilmSimulation, FujiGrainEffect, FujiHighISONR, FujiHighlightTone, FujiImageQuality,
    FujiImageSize, FujiShadowTone, FujiSharpness, FujiStillDynamicRangePriority, FujiWhiteBalance,
    FujiWhiteBalanceShift, FujiWhiteBalanceTemperature,
};

#[derive(Args, Debug)]
pub struct FilmSimulationOptions {
    /// The name of the slot
    #[clap(long)]
    pub name: Option<String>,

    /// The Fujifilm film simulation to use
    #[clap(long)]
    pub simulation: Option<FujiFilmSimulation>,

    /// The output image resolution
    #[clap(long, alias = "size")]
    pub resolution: Option<FujiImageSize>,

    /// The output image quality (JPEG compression level)
    #[clap(long, value_parser)]
    pub quality: Option<FujiImageQuality>,

    /// Highlight Tone
    #[clap(long, value_parser)]
    pub highlights: Option<FujiHighlightTone>,

    /// Shadow Tone
    #[clap(long, value_parser)]
    pub shadows: Option<FujiShadowTone>,

    /// Color
    #[clap(long, value_parser)]
    pub color: Option<FujiColor>,

    /// Sharpness
    #[clap(long, value_parser)]
    pub sharpness: Option<FujiSharpness>,

    /// Clarity
    #[clap(long, value_parser)]
    pub clarity: Option<FujiClarity>,

    /// White Balance
    #[clap(long, value_parser)]
    pub white_balance: Option<FujiWhiteBalance>,

    /// White Balance Shift Red
    #[clap(long, value_parser)]
    pub white_balance_shift_red: Option<FujiWhiteBalanceShift>,

    /// White Balance Shift Blue
    #[clap(long, value_parser)]
    pub white_balance_shift_blue: Option<FujiWhiteBalanceShift>,

    /// White Balance Temperature (Only used if WB is set to 'Temperature')
    #[clap(long, value_parser)]
    pub white_balance_temperature: Option<FujiWhiteBalanceTemperature>,

    /// Dynamic Range
    #[clap(long, value_parser)]
    pub dynamic_range: Option<FujiDynamicRange>,

    /// Dynamic Range Priority
    #[clap(long, value_parser)]
    pub dynamic_ranga_priority: Option<FujiStillDynamicRangePriority>,

    /// High ISO Noise Reduction
    #[clap(long, value_parser)]
    pub noise_reduction: Option<FujiHighISONR>,

    /// Grain Effect
    #[clap(long, value_parser)]
    pub grain: Option<FujiGrainEffect>,

    /// Color Chrome Effect
    #[clap(long, value_parser)]
    pub color_chrome_effect: Option<FujiColorChromeEffect>,

    /// Color Chrome FX Blue
    #[clap(long, value_parser)]
    pub color_chrome_fx_blue: Option<FujiColorChromeFXBlue>,
}
