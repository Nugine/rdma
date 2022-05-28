use std::env;
use std::net::SocketAddr;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, clap::Parser)]
struct Opt {
    #[clap(flatten)]
    args: Args,

    server: Option<SocketAddr>,
}

#[derive(Debug, clap::Args)]
struct Args {}

fn main() -> Result<()> {
    if env::var("RUST_BACKTRACE").is_err() {
        env::set_var("RUST_BACKTRACE", "full")
    }

    let opt = Opt::parse();

    {
        let args = &opt.args;
        println!("args:");
    }

    match opt.server {
        Some(server) => {
            println!("run client");
            run_client(opt.args, server)?;
        }
        None => {
            println!("run server");
            run_server(opt.args)?;
        }
    }

    Ok(())
}

fn run_server(args: Args) -> Result<()> {
    todo!()
}

fn run_client(args: Args, server: SocketAddr) -> Result<()> {
    todo!()
}
