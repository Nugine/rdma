use rdma::ctx::Context;
use rdma::device::{Device, DeviceList};

use std::env;
use std::net::SocketAddr;

use anyhow::{anyhow, Result};
use clap::Parser;
use tracing::info;

#[derive(Debug, clap::Parser)]
struct Opt {
    #[clap(flatten)]
    args: Args,

    server: Option<SocketAddr>,
}

#[derive(Debug, clap::Args)]
struct Args {
    /// IB device (default first device found)
    #[clap(short = 'd', long)]
    ib_dev: Option<String>,
}

fn main() -> Result<()> {
    if env::var("RUST_BACKTRACE").is_err() {
        env::set_var("RUST_BACKTRACE", "full")
    }
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "rdma_rcpp=trace,rdma=trace")
    }

    tracing_subscriber::fmt::init();

    let Opt { args, server } = Opt::parse();

    info!("args:\n{:#?}", args);

    match server {
        Some(server) => info!(?server, "run client"),
        None => info!("run server"),
    }

    let ctx = {
        let dev_list = DeviceList::available()?;
        let dev = choose_device(&dev_list, args.ib_dev.as_deref())?;
        Context::open(dev)?
    };

    Ok(())
}

fn choose_device<'dl>(dev_list: &'dl DeviceList, name: Option<&str>) -> Result<&'dl Device> {
    let dev = match name {
        Some(name) => dev_list.iter().find(|d| d.name() == name),
        None => dev_list.get(0),
    };
    if let Some(dev) = dev {
        return Ok(dev);
    }
    if dev_list.is_empty() {
        return Err(anyhow!("no available rdma devices"));
    }
    Err(anyhow!("can not find device with name: {}", name.unwrap()))
}
