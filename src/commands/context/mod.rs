pub mod init;
pub mod profiles;
pub mod render;
pub mod show;

use crate::cli::ContextCommands;
use std::path::Path;

pub fn handle(cwd: &Path, command: ContextCommands) -> anyhow::Result<()> {
    match command {
        ContextCommands::Init { force } => init::handle(cwd, force),
        ContextCommands::Show => show::handle(cwd),
        ContextCommands::Profiles => profiles::handle(cwd),
        ContextCommands::Render { profile } => render::handle(cwd, profile),
    }
}
