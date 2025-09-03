mod cli;
mod engine;
mod session;

use cli::interactive::InteractiveCLI;

fn main() {
    let mut cli = InteractiveCLI::new();
    cli.run();
}
