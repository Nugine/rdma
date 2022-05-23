mod devices;

use std::env;

use clap::StructOpt;

#[derive(clap::Parser)]
enum Opt {
    Devices,
}

fn main() -> anyhow::Result<()> {
    if env::var("RUST_BACKTRACE").is_err() {
        env::set_var("RUST_BACKTRACE", "1");
    }

    let opt = Opt::parse();
    match opt {
        Opt::Devices => devices::run()?,
    }

    Ok(())
}