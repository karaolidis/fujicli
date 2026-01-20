use clap::Args;

use fujicli::ptp::fuji;

#[derive(Args, Debug)]
pub struct FilmSimulationOptions {
    /// Fujifilm Film Simulation
    #[clap(long)]
    pub simulation: Option<fuji::FilmSimulation>,

    /// Monochromatic Color Temperature (only applicable to B&W film simulations)
    #[clap(long)]
    pub monochromatic_color_temperature: Option<fuji::MonochromaticColorShift>,

    /// Monochromatic Color Tint (only applicable to B&W film simulations)
    #[clap(long)]
    pub monochromatic_color_tint: Option<fuji::MonochromaticColorShift>,

    /// The output image resolution
    #[clap(long)]
    pub size: Option<fuji::ImageSize>,

    /// The output image quality (JPEG compression level)
    #[clap(long)]
    pub quality: Option<fuji::ImageQuality>,

    /// Highlight Tone
    #[clap(long, allow_hyphen_values(true))]
    pub highlight: Option<fuji::HighlightTone>,

    /// Shadow Tone
    #[clap(long, allow_hyphen_values(true))]
    pub shadow: Option<fuji::ShadowTone>,

    /// Color
    #[clap(long, allow_hyphen_values(true))]
    pub color: Option<fuji::Color>,

    /// Sharpness
    #[clap(long, allow_hyphen_values(true))]
    pub sharpness: Option<fuji::Sharpness>,

    /// Clarity
    #[clap(long, allow_hyphen_values(true))]
    pub clarity: Option<fuji::Clarity>,

    /// White Balance
    #[clap(long)]
    pub white_balance: Option<fuji::WhiteBalance>,

    /// White Balance Shift Red
    #[clap(long, allow_hyphen_values(true))]
    pub white_balance_shift_red: Option<fuji::WhiteBalanceShift>,

    /// White Balance Shift Blue
    #[clap(long, allow_hyphen_values(true))]
    pub white_balance_shift_blue: Option<fuji::WhiteBalanceShift>,

    /// White Balance Temperature (Only used if WB is set to 'Temperature')
    #[clap(long)]
    pub white_balance_temperature: Option<fuji::WhiteBalanceTemperature>,

    /// Dynamic Range
    #[clap(long)]
    pub dynamic_range: Option<fuji::DynamicRange>,

    /// Dynamic Range Priority
    #[clap(long)]
    pub dynamic_range_priority: Option<fuji::DynamicRangePriority>,

    /// High ISO Noise Reduction
    #[clap(long, allow_hyphen_values(true))]
    pub noise_reduction: Option<fuji::NoiseReduction>,

    /// Grain Effect
    #[clap(long)]
    pub grain: Option<fuji::GrainEffect>,

    /// Color Chrome Effect
    #[clap(long)]
    pub color_chrome_effect: Option<fuji::ColorChromeEffect>,

    /// Color Chrome FX Blue
    #[clap(long)]
    pub color_chrome_fx_blue: Option<fuji::ColorChromeFXBlue>,

    /// Smooth Skin Effect
    #[clap(long)]
    pub smooth_skin_effect: Option<fuji::SmoothSkinEffect>,

    /// Lens Modulation Optimizer
    #[clap(long)]
    pub lens_modulation_optimizer: Option<fuji::LensModulationOptimizer>,

    /// Color Space
    #[clap(long)]
    pub color_space: Option<fuji::ColorSpace>,
}
