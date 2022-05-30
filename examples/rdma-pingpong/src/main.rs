#![deny(clippy::all)]

use rdma::ah::{AddressHandle, GlobalRoute};
use rdma::cc::CompChannel;
use rdma::cq::CompletionQueue;
use rdma::ctx::Context;
use rdma::device::{Device, DeviceList, Gid, GidEntry, LinkLayer, Mtu, PortAttr, PortState};
use rdma::mr::{AccessFlags, MemoryRegion};
use rdma::pd::ProtectionDomain;
use rdma::qp::{self, QueuePair};
use rdma::qp::{QueuePairCapacity, QueuePairState, QueuePairType};
use rdma::wc::WorkCompletion;
use rdma::wr;

use std::env;
use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::slice;
use std::time::Instant;

use anyhow::{anyhow, ensure, Context as _, Result};
use clap::Parser;
use numeric_cast::NumericCast;
use serde::{Deserialize, Serialize};
use tracing::{info, trace};

#[derive(Debug, clap::Parser)]
struct Args {
    /// IB device (default first device found)
    #[clap(short = 'd', long)]
    ib_dev: Option<String>,

    /// size of message to exchange
    #[clap(short = 's', long, default_value = "1024")]
    size: usize,

    /// number of receives to post at a time
    #[clap(short = 'r', long, default_value = "500")]
    rx_depth: usize,

    /// port of IB device
    #[clap(short = 'i', long, default_value = "1")]
    ib_port: u8,

    /// local port gid index
    #[clap(short = 'g', long, default_value = "2")]
    gid_idx: u32,

    /// listen on/connect to port
    #[clap(short = 'p', long, default_value = "18515")]
    port: u16,

    /// number of exchanges
    #[clap(short = 'n', long, default_value = "1000")]
    iters: usize,

    #[clap(parse(try_from_str = parse_qp_type))]
    qp_type: QueuePairType,

    server: Option<IpAddr>,
}

fn parse_qp_type(s: &str) -> Result<QueuePairType> {
    match s {
        "rc" => Ok(QueuePairType::RC),
        "ud" => Ok(QueuePairType::UD),
        _ => Err(anyhow!("unsupported qp type")),
    }
}

fn main() -> Result<()> {
    if env::var("RUST_BACKTRACE").is_err() {
        env::set_var("RUST_BACKTRACE", "full")
    }
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "rdma_pingpong=trace,rdma=trace")
    }

    tracing_subscriber::fmt::init();

    let args = Args::parse();

    info!("args:\n{:#?}", args);

    match args.server {
        Some(server) => info!(?server, "run client"),
        None => info!("run server"),
    }

    run(args)
}

#[derive(Debug, Serialize, Deserialize)]
struct Dest {
    qpn: u32,
    psn: u32,
    lid: u16,
    gid: Gid,
}

const RECV_WRID: u64 = 1;
const SEND_WRID: u64 = 2;
const UD_QKEY: u32 = 0x11111111;

const UNINIT_WC: MaybeUninit<WorkCompletion> = MaybeUninit::uninit();

fn run(args: Args) -> Result<()> {
    ensure!(args.size > 0);

    let buf_size = match args.qp_type {
        QueuePairType::RC => args.size,
        QueuePairType::UD => args.size.checked_add(40).unwrap(),
        _ => unimplemented!(),
    };

    let mut send_buf: Vec<u8> = vec![0; buf_size];
    let mut recv_buf: Vec<u8> = vec![0; buf_size];

    let ctx = {
        let dev_list = DeviceList::available()?;
        let dev = choose_device(&dev_list, args.ib_dev.as_deref())?;
        info!("device name: {}", dev.name());
        Context::open(dev)?
    };

    {
        let port_attr = PortAttr::query(&ctx, args.ib_port)?;
        let port_state = port_attr.state();
        info!(?port_state);

        ensure!(
            port_state == PortState::Active,
            "ib port {} is not active ({:?})",
            args.ib_port,
            port_state
        );

        match args.qp_type {
            QueuePairType::RC => {}
            QueuePairType::UD => {
                let mtu_size = port_attr.active_mtu().size();
                info!(?mtu_size);

                ensure!(
                    args.size <= mtu_size,
                    "message size larger than port MTU ({})",
                    mtu_size
                );
            }
            _ => unimplemented!(),
        }
    }

    let cc = CompChannel::create(&ctx)?;

    let pd = ProtectionDomain::alloc(&ctx)?;

    let mut send_mr = unsafe {
        let addr = send_buf.as_mut_ptr();
        let length = send_buf.len();
        let access_flags = AccessFlags::LOCAL_WRITE;
        MemoryRegion::register(&pd, addr, length, access_flags)?
    };
    let mut recv_mr = unsafe {
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
            .qp_type(args.qp_type)
            .sq_sig_all(true)
            .pd(&pd);
        QueuePair::create(&ctx, options)?
    };

    initialize(&qp, &args)?;

    let local_dest = local_dest(&ctx, &qp, args.ib_port, args.gid_idx)?;
    info!("local dest:\n{:#?}", local_dest);

    let remote_dest = exchange_dest_over_tcp(args.server, args.port, &local_dest)?;
    info!("remote dest:\n{:#?}", remote_dest);

    let ah = match args.qp_type {
        QueuePairType::RC => {
            rc_activate(&qp, &local_dest, &remote_dest, &args)?;
            None
        }
        QueuePairType::UD => {
            let ah = ud_activate(&pd, &qp, &local_dest, &remote_dest, &args)?;
            Some(ah)
        }
        _ => unimplemented!(),
    };

    {
        match args.qp_type {
            QueuePairType::RC => {}
            _ => todo!(),
        }
    }

    let time_sec = {
        let mut recv_comp_cnt = 0;
        let mut send_comp_cnt = 0;
        let mut recv_req_cnt = 0;
        let mut send_req_cnt = 0;

        let mut wc_buf = [UNINIT_WC; 2];

        info!("start iteration");

        let t0 = Instant::now();

        loop {
            cq.req_notify_all()?;

            loop {
                if recv_req_cnt <= 1 {
                    unsafe { rc_post_recv(&qp, &mut recv_mr, args.rx_depth)? };
                    recv_req_cnt += args.rx_depth;
                }
                if send_req_cnt < 1 && send_comp_cnt < args.iters {
                    unsafe { rc_post_send(&qp, &mut send_mr)? };
                    send_req_cnt += 1;
                }

                let wcs = cq.poll(&mut wc_buf)?;

                for wc in &mut *wcs {
                    wc.status()?;

                    match wc.wr_id() {
                        SEND_WRID => {
                            send_comp_cnt += 1;
                            send_req_cnt -= 1;
                        }
                        RECV_WRID => {
                            recv_comp_cnt += 1;
                            recv_req_cnt -= 1;
                        }
                        _ => panic!("unknown wr id: {}", wc.wr_id()),
                    }
                }

                if wcs.is_empty() {
                    break;
                }
            }

            trace!(?send_comp_cnt, ?recv_comp_cnt, ?send_req_cnt, ?recv_req_cnt);

            if recv_comp_cnt >= args.iters && send_comp_cnt >= args.iters {
                break;
            }

            cc.wait_cq_event()?;
            cq.ack_cq_events(1);
        }

        let t1 = Instant::now();

        info!("end iteration");

        (t1 - t0).as_secs_f64()
    };

    {
        let bytes = args.size * args.iters * 2;
        print_statistics(time_sec, bytes, args.iters)
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
        return Err(anyhow!("No available rdma devices"));
    }
    Err(anyhow!("Can not find device with name: {}", name.unwrap()))
}

fn initialize(qp: &QueuePair, args: &Args) -> Result<()> {
    let mut options = qp::ModifyOptions::default();

    options
        .qp_state(QueuePairState::Initialize)
        .pkey_index(0)
        .port_num(args.ib_port);

    match args.qp_type {
        QueuePairType::RC => options.qp_access_flags(AccessFlags::empty()),
        QueuePairType::UD => options.qkey(UD_QKEY),
        _ => unimplemented!(),
    };

    qp.modify(options)?;
    Ok(())
}

fn local_dest(ctx: &Context, qp: &QueuePair, ib_port: u8, gid_index: u32) -> Result<Dest> {
    let qpn = qp.qp_num();
    let psn = 0x123456;

    let port_attr = PortAttr::query(ctx, ib_port)?;
    let lid = port_attr.lid();
    if port_attr.link_layer() != LinkLayer::Ethernet && lid == 0 {
        return Err(anyhow!("Can not get local LID"));
    }

    let gid_entry = GidEntry::query(ctx, ib_port.into(), gid_index)?;
    let gid = gid_entry.gid();

    Ok(Dest { qpn, psn, lid, gid })
}

fn exchange_dest_over_tcp(server: Option<IpAddr>, port: u16, local_dest: &Dest) -> Result<Dest> {
    let mut stream = match server {
        Some(ip) => {
            // client side
            let server_addr = SocketAddr::from((ip, port));
            info!("connecting to {}", server_addr);
            TcpStream::connect(&server_addr)?
        }
        None => {
            // server side
            let server_addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
            let listener = TcpListener::bind(server_addr)?;
            info!("listening on port {}", port);
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
    send_dest(&mut stream, &mut msg_buf, local_dest)?;
    let remote_dest = recv_dest(&mut stream, &mut msg_buf)?;
    Ok(remote_dest)
}

fn rc_activate(qp: &QueuePair, local_dest: &Dest, remote_dest: &Dest, args: &Args) -> Result<()> {
    {
        let mut options = qp::ModifyOptions::default();

        let mut ah_attr = AddressHandle::options();
        ah_attr.dest_lid(remote_dest.lid).port_num(args.ib_port);

        if remote_dest.gid.interface_id() != 0 {
            ah_attr.global_route_header(GlobalRoute {
                dest_gid: remote_dest.gid,
                flow_label: 0,
                sgid_index: args.gid_idx.numeric_cast(),
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

        qp.modify(options).context("failed to modify QP to RTR")?;
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

        qp.modify(options).context("failed to modify QP to RTS")?;
    }

    Ok(())
}

fn ud_activate(
    pd: &ProtectionDomain,
    qp: &QueuePair,
    local_dest: &Dest,
    remote_dest: &Dest,
    args: &Args,
) -> Result<AddressHandle> {
    {
        let mut options = qp::ModifyOptions::default();
        options.qp_state(QueuePairState::ReadyToReceive);

        qp.modify(options).context("failed to modify QP to RTR")?;
    }

    {
        let mut options = qp::ModifyOptions::default();
        options
            .qp_state(QueuePairState::ReadyToSend)
            .sq_psn(local_dest.psn);

        qp.modify(options).context("failed to modify QP to RTS")?;
    }

    {
        let mut options = AddressHandle::options();
        options.dest_lid(remote_dest.lid).port_num(args.ib_port);

        if remote_dest.gid.interface_id() != 0 {
            options.global_route_header(GlobalRoute {
                dest_gid: remote_dest.gid,
                flow_label: 0,
                sgid_index: args.gid_idx.numeric_cast(),
                hop_limit: 1,
                traffic_class: 0,
            });
        }

        let ah = AddressHandle::create(pd, options)?;
        Ok(ah)
    }
}

unsafe fn rc_post_recv(qp: &QueuePair, recv_mr: &mut MemoryRegion, n: usize) -> Result<()> {
    let mut sge = wr::Sge {
        addr: recv_mr.addr_u64(),
        length: recv_mr.length().numeric_cast(),
        lkey: recv_mr.lkey(),
    };

    let mut recv_wr = wr::RecvRequest::zeroed();
    recv_wr.id(RECV_WRID).sg_list(slice::from_mut(&mut sge));

    for _ in 0..n {
        qp.post_recv(&mut recv_wr)?;
    }

    Ok(())
}

unsafe fn rc_post_send(qp: &QueuePair, send_mr: &mut MemoryRegion) -> Result<()> {
    let mut sge = wr::Sge {
        addr: send_mr.addr_u64(),
        length: send_mr.length().numeric_cast(),
        lkey: send_mr.lkey(),
    };

    let mut send_wr = wr::SendRequest::zeroed();

    send_wr
        .id(SEND_WRID)
        .sg_list(slice::from_mut(&mut sge))
        .opcode(wr::Opcode::Send);

    qp.post_send(&mut send_wr)?;

    Ok(())
}

fn print_statistics(time_sec: f64, bytes: usize, iters: usize) {
    println!(
        "{} bytes in {:.2} seconds = {:.2} Mbps",
        bytes,
        time_sec,
        (bytes * 8) as f64 / 1e6 / time_sec
    );
    println!(
        "{} iters in {:.2} seconds = {:.2} us/iter",
        iters,
        time_sec,
        time_sec * 1e6 / (iters as f64)
    );
}
