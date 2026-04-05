use clap::CommandFactory;
use clap_complete::{generate, Shell};

use crate::cli::Cli;
use crate::error::Result;

pub(crate) fn run(shell: Shell, bin_name: &str) -> Result<()> {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
    Ok(())
}
