use std::time::Duration;

use clap::Parser;

#[derive(Parser, Clone)]
pub(crate) struct Cli {
    /// Seconds to wait before printing
    #[arg(long, default_value_t = 3)]
    pub(crate) output_timeout: u32,
    /// Also print to stdout
    #[arg(long)]
    pub(crate) stdout_print: bool,
}

impl Cli {
    fn run(&self) {
        loop {
            if self.stdout_print {
                for _ in 1..50 {
                    println!("Printing to stdout");
                }
            }
            eprintln!("Printing to stderr {}", self.output_timeout);
            std::thread::sleep(Duration::from_secs(self.output_timeout.into()));
        }
    }
}

fn main() {
    let cli = Cli::parse();
    cli.run()
}
