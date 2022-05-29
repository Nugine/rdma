#![deny(clippy::all)]

use rdma::ah::{AddressHandle, GlobalRoute};
use rdma::cc::CompChannel;
use rdma::cq::CompletionQueue;
use rdma::ctx::Context;
use rdma::device::{Device, DeviceList, Gid, GidEntry, LinkLayer, PortAttr};
use rdma::mr::{AccessFlags, MemoryRegion};
use rdma::pd::ProtectionDomain;
use rdma::qp::{self, Mtu, QueuePair};
use rdma::qp::{QueuePairCapacity, QueuePairState, QueuePairType};
use rdma::wc::WorkCompletion;
use rdma::wr;

use std::io::{Read, Write};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::net::{IpAddr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Instant;
use std::{env, slice};

use anyhow::{anyhow, Context as _, Result};
use clap::Parser;
use numeric_cast::NumericCast;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

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

    /// listen on/connect to port
    #[clap(short = 'p', long, default_value = "18515")]
    port: u16,

    /// number of exchanges
    #[clap(short = 'n', long, default_value = "1000")]
    iters: usize,
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

    run(args, server)
}

fn run(args: Args, server: Option<IpAddr>) -> Result<()> {
    let mut pp = PingPong::new(args, server)?;

    unsafe { pp.post_recv(pp.args.rx_depth)? };

    pp.cq.req_notify_all()?;

    pp.handshake()?;

    unsafe { pp.post_send()? };

    let mut recv_cnt = 0;
    let mut send_cnt = 0;
    let mut recv_wr_cnt = pp.args.rx_depth;
    const UNINIT_WC: MaybeUninit<WorkCompletion> = MaybeUninit::uninit();
    let mut wc_buf = [UNINIT_WC; 64];

    info!("start iteration");

    let t0 = Instant::now();

    while recv_cnt < pp.args.iters || send_cnt < pp.args.iters {
        debug!(?recv_cnt, ?send_cnt);

        pp.cc.wait_cq_event()?;
        pp.cq.ack_cq_events(1);

        pp.cq.req_notify_all()?;

        let wcs = pp.cq.poll(&mut wc_buf)?;

        debug!("poll {} wcs", wcs.len());

        for wc in wcs {
            wc.status()?;

            match wc.wr_id() {
                PingPong::SEND_WRID => {
                    send_cnt += 1;
                    debug!(?send_cnt);
                    if send_cnt < pp.args.iters {
                        unsafe { pp.post_send()? }
                    }
                }
                PingPong::RECV_WRID => {
                    recv_cnt += 1;
                    debug!(?recv_cnt);

                    recv_wr_cnt -= 1;
                    if recv_wr_cnt <= 1 {
                        unsafe { pp.post_recv(pp.args.rx_depth)? }
                        recv_wr_cnt += pp.args.rx_depth;
                    }
                }
                _ => panic!("unknown wr id: {}", wc.wr_id()),
            }
        }
    }

    let t1 = Instant::now();

    info!("end iteration");

    let time = (t1 - t0).as_secs_f64();
    let bytes = pp.args.size * pp.args.iters * 2;
    println!(
        "{} bytes in {:.2} seconds = {:.2} Mbps",
        bytes,
        time,
        (bytes * 8) as f64 / time
    );
    println!(
        "{} iters in {:.2} seconds = {:.2} us/iter",
        pp.args.iters,
        time,
        time * 1e6 / (pp.args.iters as f64)
    );

    drop(pp);

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Dest {
    qpn: u32,
    psn: u32,
    lid: u16,
    gid: Gid,
}

struct PingPong {
    args: Args,
    server: Option<IpAddr>,

    send_mr: ManuallyDrop<MemoryRegion>,
    recv_mr: ManuallyDrop<MemoryRegion>,

    qp: QueuePair,
    cq: CompletionQueue,
    _pd: ProtectionDomain,
    cc: CompChannel,
    ctx: Context,

    _send_buf: Vec<u8>,
    _recv_buf: Vec<u8>,
}

impl Drop for PingPong {
    fn drop(&mut self) {
        // deregister the memory region firstly
        unsafe {
            ManuallyDrop::drop(&mut self.send_mr);
            ManuallyDrop::drop(&mut self.recv_mr)
        }
    }
}

impl PingPong {
    const RECV_WRID: u64 = 1 << 0;
    const SEND_WRID: u64 = 1 << 1;

    fn new(args: Args, server: Option<IpAddr>) -> Result<Self> {
        assert_ne!(args.size, 0);
        let mut send_buf: Vec<u8> = vec![0; args.size];
        let mut recv_buf: Vec<u8> = vec![0; args.size];

        let ctx: _ = {
            let dev_list = DeviceList::available()?;
            let dev = Self::choose_device(&dev_list, args.ib_dev.as_deref())?;
            info!("device name: {}", dev.name());
            Context::open(dev)?
        };

        let cc = CompChannel::create(&ctx)?;

        let pd = ProtectionDomain::alloc(&ctx)?;

        let send_mr = unsafe {
            let addr = send_buf.as_mut_ptr();
            let length = send_buf.len();
            let access_flags = AccessFlags::LOCAL_WRITE;
            MemoryRegion::register(&pd, addr, length, access_flags)?
        };
        let recv_mr = unsafe {
            let addr = recv_buf.as_mut_ptr();
            let length = recv_buf.len();
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
            let cap = QueuePairCapacity {
                max_send_wr: 1,
                max_recv_wr: args.rx_depth.numeric_cast(),
                max_send_sge: 1,
                max_recv_sge: 1,
                max_inline_data: 0,
            };
            let mut options = QueuePair::options();
            options
                .send_cq(&cq)
                .recv_cq(&cq)
                .cap(cap)
                .qp_type(QueuePairType::RC)
                .sq_sig_all(true);
            QueuePair::create(&pd, options)?
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
            send_mr: ManuallyDrop::new(send_mr),
            recv_mr: ManuallyDrop::new(recv_mr),
            qp,
            cq,
            _pd: pd,
            cc,
            ctx,
            _send_buf: send_buf,
            _recv_buf: recv_buf,
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
            addr: self.recv_mr.addr_u64(),
            length: self.recv_mr.length().numeric_cast(),
            lkey: self.recv_mr.lkey(),
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

    unsafe fn post_send(&mut self) -> Result<()> {
        let mut sge = wr::Sge {
            addr: self.send_mr.addr_u64(),
            length: self.send_mr.length().numeric_cast(),
            lkey: self.send_mr.lkey(),
        };

        let mut send_wr = wr::SendRequest::zeroed();

        send_wr
            .id(Self::SEND_WRID)
            .sg_list(slice::from_mut(&mut sge));

        self.qp.post_send(&mut send_wr)?;

        Ok(())
    }

    fn local_dest(&self) -> Result<Dest> {
        let qpn = self.qp.qp_num();
        let psn = 0x123456;

        let port_attr = PortAttr::query(&self.ctx, self.args.ib_port)?;
        let lid = port_attr.lid();
        if port_attr.link_layer() != LinkLayer::Ethernet && lid == 0 {
            return Err(anyhow!("can not get local LID"));
        }

        let gid_entry = GidEntry::query(&self.ctx, self.args.ib_port.into(), self.args.gid_idx)?;
        let gid = gid_entry.gid();

        Ok(Dest { qpn, psn, lid, gid })
    }

    fn forward_qp(&mut self, local_dest: &Dest, remote_dest: &Dest) -> Result<()> {
        {
            let mut options = qp::ModifyOptions::default();

            let mut ah_attr = AddressHandle::options();
            ah_attr
                .dest_lid(remote_dest.lid)
                .port_num(self.args.ib_port);

            if remote_dest.gid.interface_id() != 0 {
                ah_attr.global_route_header(GlobalRoute {
                    dest_gid: remote_dest.gid,
                    flow_label: 0,
                    sgid_index: self.args.gid_idx.numeric_cast(),
                    hop_limit: 1,
                    traffic_class: 0,
                });
            }

            options
                .qp_state(QueuePairState::ReadyToReceive)
                .path_mtu(Mtu::Mtu1024)
                .dest_qp_num(remote_dest.qpn)
                .rq_psn(remote_dest.psn)
                .max_dest_rd_atomic(1)
                .min_rnr_timer(12)
                .ah_attr(ah_attr);

            self.qp
                .modify(options)
                .context("failed to modify QP to RTR")?;
        }

        {
            let mut options = qp::ModifyOptions::default();
            options
                .qp_state(QueuePairState::ReadyToSend)
                .timeout(14)
                .retry_cnt(7)
                .rnr_retry(7)
                .sq_psn(local_dest.psn)
                .max_rd_atomic(1);

            self.qp
                .modify(options)
                .context("failed to modify QP to RTS")?;
        }

        Ok(())
    }

    fn handshake(&mut self) -> Result<()> {
        let local_dest = self.local_dest()?;
        info!("local dest:\n{:#?}", local_dest);

        let mut stream = match self.server {
            Some(ip) => {
                // client side
                let server_addr = SocketAddr::from((ip, self.args.port));
                info!("connecting to {}", server_addr);
                TcpStream::connect(&server_addr)?
            }
            None => {
                // server side
                let server_addr = SocketAddr::from((Ipv6Addr::UNSPECIFIED, self.args.port));
                let listener = TcpListener::bind(server_addr)?;
                info!("listening on port {}", self.args.port);
                let (stream, peer_addr) = listener.accept()?;
                info!("accepted connection from {}", peer_addr);
                stream
            }
        };

        let send_dest = |stream: &mut TcpStream, msg_buf: &mut Vec<u8>, dest: &Dest| {
            msg_buf.clear();
            bincode::serialize_into(&mut *msg_buf, &dest)?;
            let msg_size: u8 = msg_buf.len().numeric_cast();
            stream.write_all(&[msg_size])?;
            stream.write_all(msg_buf)?;
            stream.flush()?;
            anyhow::Result::<()>::Ok(())
        };

        let recv_dest = |stream: &mut TcpStream, msg_buf: &mut Vec<u8>| {
            let mut msg_size = [0u8];
            stream.read_exact(&mut msg_size)?;
            msg_buf.clear();
            msg_buf.resize(msg_size[0].into(), 0);
            stream.read_exact(&mut *msg_buf)?;
            let dest = bincode::deserialize::<Dest>(&*msg_buf)?;
            anyhow::Result::<Dest>::Ok(dest)
        };

        let mut msg_buf = Vec::new();
        send_dest(&mut stream, &mut msg_buf, &local_dest)?;
        let remote_dest = recv_dest(&mut stream, &mut msg_buf)?;
        info!("remote dest:\n{:#?}", remote_dest);

        self.forward_qp(&local_dest, &remote_dest)?;

        Ok(())
    }
}
