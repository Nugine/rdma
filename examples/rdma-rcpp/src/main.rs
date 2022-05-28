#![deny(clippy::all)]

use numeric_cast::NumericCast;
use rdma::cc::CompChannel;
use rdma::cq::CompletionQueue;
use rdma::ctx::Context;
use rdma::device::{Device, DeviceList};
use rdma::mr::{AccessFlags, MemoryRegion};
use rdma::pd::ProtectionDomain;
use rdma::qp::{self, QueuePair, QueuePairState, QueuePairType};

use std::env;
use std::net::{IpAddr, SocketAddr};

use anyhow::{anyhow, Result};
use clap::Parser;
use tracing::info;

#[derive(Debug, clap::Parser)]
struct Opt {
    #[clap(flatten)]
    args: Args,

    server: Option<IpAddr>,
}

#[derive(Debug, clap::Args)]
struct Args {
    /// IB device (default first device found)
    #[clap(short = 'd', long)]
    ib_dev: Option<String>,

    /// size of message to exchange
    #[clap(short = 's', long, default_value = "4096")]
    size: usize,

    /// number of receives to post at a time
    #[clap(short = 'r', long, default_value = "500")]
    rx_depth: usize,

    /// port of IB device
    #[clap(short = 'i', long, default_value = "1")]
    ib_port: u8,
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

    run(&args, server)
}

fn run(args: &Args, server: Option<IpAddr>) -> Result<()> {
    let mut buf: Vec<u8> = {
        assert_ne!(args.size, 0);
        vec![0xcc; args.size]
    };

    let ctx: _ = {
        let dev_list = DeviceList::available()?;
        let dev = choose_device(&dev_list, args.ib_dev.as_deref())?;
        Context::open(dev)?
    };

    let cc = CompChannel::create(&ctx)?;

    let pd = ProtectionDomain::alloc(&ctx)?;

    let mr = unsafe {
        let addr = buf.as_mut_ptr();
        let length = buf.len();
        let access_flags = AccessFlags::LOCAL_WRITE;
        MemoryRegion::register(&pd, addr, length, access_flags)?
    };

    let cq = {
        let mut options = CompletionQueue::options();
        options.cqe(args.rx_depth.checked_add(1).unwrap());
        options.channel(&cc);
        CompletionQueue::create(&ctx, options)?
    };

    let qp = {
        let mut options = QueuePair::options();
        options
            .send_cq(&cq)
            .recv_cq(&cq)
            .max_send_wr(1)
            .max_recv_wr(args.rx_depth.numeric_cast())
            .max_send_sge(1)
            .max_recv_sge(1)
            .qp_type(QueuePairType::RC);
        QueuePair::create(&pd, options)?
    };

    let can_send_inline = {
        let mut options = qp::QueryOptions::default();
        options.cap();
        let qp_attr = qp.query(options)?;
        let max_inline_data = qp_attr.max_inline_data().unwrap().numeric_cast::<usize>();
        max_inline_data >= args.rx_depth
    };

    {
        let mut options = qp::ModifyOptions::default();
        options
            .qp_state(QueuePairState::Initialize)
            .pkey_index(0)
            .port_num(args.ib_port)
            .qp_access_flags(AccessFlags::empty());
        qp.modify(options)?;
    }

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
