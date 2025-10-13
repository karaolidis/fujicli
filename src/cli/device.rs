use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum DeviceCmd {
    /// List devices
    #[command(alias = "l")]
    List,

    /// Dump device info
    #[command(alias = "i")]
    Info,
}
