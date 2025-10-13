use super::common::file::{Input, Output};
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum BackupCmd {
    /// Export backup
    #[command(alias = "e")]
    Export {
        /// Output file (use '-' to write to stdout)
        output_file: Output,
    },

    /// Import backup
    #[command(alias = "i")]
    Import {
        /// Input file (use '-' to read from stdin)
        input_file: Input,
    },
}
