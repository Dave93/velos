use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;
use velos_core::VelosError;

pub fn run(shell: String) -> Result<(), VelosError> {
    let shell: Shell = shell.parse().map_err(|_| {
        VelosError::ProtocolError(format!(
            "Unknown shell: {shell}. Supported: bash, zsh, fish, elvish, powershell"
        ))
    })?;
    let mut cmd = crate::Cli::command();
    generate(shell, &mut cmd, "velos", &mut io::stdout());
    Ok(())
}
