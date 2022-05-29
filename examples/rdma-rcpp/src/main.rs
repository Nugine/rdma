#![deny(clippy::all)]

use rdma::cc::CompChannel;
use rdma::cq::CompletionQueue;
use rdma::ctx::Context;
use rdma::device::{Device, DeviceList};
use rdma::mr::{AccessFlags, MemoryRegion};
use rdma::pd::ProtectionDomain;
use rdma::qp::{self, QueuePair};
use rdma::qp::{QueuePairCapacity, QueuePairState, QueuePairType};
use rdma::query::{Gid, GidEntry, LinkLayer, PortAttr};
use rdma::wr;

use std::mem::ManuallyDrop;
use std::net::IpAddr;
use std::{env, slice};

use anyhow::{anyhow, Result};
use clap::Parser;
use numeric_cast::NumericCast;
use serde::{Deserialize, Serialize};
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

    /// local port gid index
    #[clap(short = 'g', long, default_value = "0")]
    gid_idx: u32,
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

    let mut pp = PingPong::new(args, server)?;

    unsafe { pp.post_recv(pp.args.rx_depth)? }

    pp.cq.req_notify_all()?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Dest {
    lid: u16,
    gid: Gid,
    qpn: u32,
    psn: u32,
}

struct PingPong {
    args: Args,
    server: Option<IpAddr>,

    mr: ManuallyDrop<MemoryRegion>,
    qp: QueuePair,
    cq: CompletionQueue,
    pd: ProtectionDomain,
    cc: CompChannel,
    ctx: Context,

    can_send_inline: bool,
}

impl Drop for PingPong {
    fn drop(&mut self) {
        // deregister the memory region firstly
        unsafe { ManuallyDrop::drop(&mut self.mr) }
    }
}

impl PingPong {
    const RECV_WRID: u64 = 1 << 0;
    const SEND_WRID: u64 = 1 << 1;

    fn new(args: Args, server: Option<IpAddr>) -> Result<Self> {
        let mut buf: Vec<u8> = {
            assert_ne!(args.size, 0);
            vec![0xcc; args.size]
        };

        let ctx: _ = {
            let dev_list = DeviceList::available()?;
            let dev = Self::choose_device(&dev_list, args.ib_dev.as_deref())?;
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
                .cap(QueuePairCapacity {
                    max_send_wr: 1,
                    max_recv_wr: args.rx_depth.numeric_cast(),
                    max_send_sge: 1,
                    max_recv_sge: 1,
                    max_inline_data: 0,
                })
                .qp_type(QueuePairType::RC);
            QueuePair::create(&pd, options)?
        };

        let can_send_inline = {
            let mut options = qp::QueryOptions::default();
            options.cap();
            let qp_attr = qp.query(options)?;
            let cap = qp_attr.cap().unwrap();
            cap.max_inline_data.numeric_cast::<usize>() >= args.rx_depth
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

        Ok(Self {
            args,
            server,
            mr: ManuallyDrop::new(mr),
            qp,
            cq,
            pd,
            cc,
            ctx,
            can_send_inline,
        })
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

    unsafe fn post_recv(&mut self, n: usize) -> Result<()> {
        let mut sge = wr::Sge {
            addr: self.mr.addr_u64(),
            length: self.mr.length().numeric_cast(),
            lkey: self.mr.lkey(),
        };
        let mut recv_wr = wr::RecvRequest::zeroed();

        recv_wr
            .id(Self::RECV_WRID)
            .sg_list(slice::from_mut(&mut sge));

        for _ in 0..n {
            self.qp.post_recv(&mut recv_wr)?;
        }

        Ok(())
    }

    fn self_dest(&self) -> Result<Dest> {
        let port_attr = PortAttr::query(&self.ctx, self.args.ib_port)?;
        let lid = port_attr.lid();
        if port_attr.link_layer() != LinkLayer::Ethernet && lid == 0 {
            return Err(anyhow!("can not get local LID"));
        }

        let gid_entry = GidEntry::query(&self.ctx, self.args.ib_port.into(), self.args.gid_idx)?;
        let gid = gid_entry.gid();

        let qpn = self.qp.qp_num();

        let psn = 0x123456;

        Ok(Dest { lid, gid, qpn, psn })
    }
}
