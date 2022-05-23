#![deny(clippy::all)]

mod auto_test;
mod devices;
mod rcpp;

use std::env;

use clap::StructOpt;

#[derive(clap::Parser)]
enum Opt {
    Devices,
    Rcpp(rcpp::Args),
    AutoTest,
}

fn main() -> anyhow::Result<()> {
    if env::var("RUST_BACKTRACE").is_err() {
        env::set_var("RUST_BACKTRACE", "1");
    }

    let opt = Opt::parse();
    match opt {
        Opt::Devices => devices::run()?,
        Opt::Rcpp(args) => rcpp::run(args)?,
        Opt::AutoTest => auto_test::run()?,
    }

    Ok(())
}
