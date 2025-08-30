mod cli;
mod engine;
mod session;

use clap::Parser;
use cli::commands::{handle_commands, Cli};

fn main() {
    let cli = Cli::parse();
    handle_commands(cli);
}
