use crate::driver::RdmaDriver;
use crate::{work, GatherList, ScatterList};

use rdma::ah::{AddressHandle, GlobalRoute};
use rdma::ctx::Context;
use rdma::device::{Gid, GidEntry, LinkLayer, Mtu, PortAttr};
use rdma::mr::AccessFlags;
use rdma::qp::{self, QueuePair, QueuePairState};

use std::io;
use std::net::SocketAddr;

use anyhow::{anyhow, Context as _, Result};
use numeric_cast::NumericCast;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

const DEFAULT_IB_PORT: u8 = 1;
const DEFAULT_GID_INDEX: u32 = 2;

fn rc_build_qp(driver: &RdmaDriver) -> io::Result<QueuePair> {
    let ctx = &driver.ctx;
    let pd = &driver.pd;
    let cq = &driver.cq;

    let cap = qp::QueuePairCapacity {
        max_send_wr: 512,
        max_recv_wr: 512,
        max_send_sge: 1,
        max_recv_sge: 1,
        max_inline_data: 0,
    };

    let qp = {
        let mut options = QueuePair::options();
        options
            .send_cq(cq)
            .recv_cq(cq)
            .cap(cap)
            .qp_type(qp::QueuePairType::RC)
            .sq_sig_all(true)
            .pd(pd);

        QueuePair::create(ctx, options)?
    };

    {
        let mut options = qp::ModifyOptions::default();

        options
            .qp_state(qp::QueuePairState::Initialize)
            .pkey_index(0)
            .port_num(1)
            .qp_access_flags(AccessFlags::empty());

        qp.modify(options)?;
    }
    Ok(qp)
}

fn rc_activate(
    qp: &QueuePair,
    local_dest: &Dest,
    remote_dest: &Dest,
    ib_port: u8,
    gid_idx: u32,
) -> Result<()> {
    {
        let mut options = qp::ModifyOptions::default();

        let mut ah_attr = AddressHandle::options();
        ah_attr.dest_lid(remote_dest.lid).port_num(ib_port);

        if remote_dest.gid.interface_id() != 0 {
            ah_attr.global_route_header(GlobalRoute {
                dest_gid: remote_dest.gid,
                flow_label: 0,
                sgid_index: gid_idx.numeric_cast(),
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

#[derive(Debug, Serialize, Deserialize)]
struct Dest {
    qpn: u32,
    psn: u32,
    lid: u16,
    gid: Gid,
}

fn local_dest(ctx: &Context, qp: &QueuePair, ib_port: u8, gid_index: u32) -> Result<Dest> {
    let qpn = qp.qp_num();
    let psn = rand::random();

    let port_attr = PortAttr::query(ctx, ib_port)?;
    let lid = port_attr.lid();
    if port_attr.link_layer() != LinkLayer::Ethernet && lid == 0 {
        return Err(anyhow!("Can not get local LID"));
    }

    let gid_entry = GidEntry::query(ctx, ib_port.into(), gid_index)?;
    let gid = gid_entry.gid();

    Ok(Dest { qpn, psn, lid, gid })
}

async fn exchange_dest(stream: &mut TcpStream, local_dest: &Dest) -> Result<Dest> {
    let mut msg_buf = Vec::new();

    {
        msg_buf.clear();
        bincode::serialize_into(&mut msg_buf, local_dest)?;
        let msg_size: u8 = msg_buf.len().numeric_cast();
        stream.write_all(&[msg_size]).await?;
        stream.write_all(&msg_buf).await?;
        stream.flush().await?;
    }

    {
        let mut msg_size = [0u8];
        stream.read_exact(&mut msg_size).await?;
        msg_buf.clear();
        msg_buf.resize(msg_size[0].into(), 0);
        stream.read_exact(&mut *msg_buf).await?;
        let dest = bincode::deserialize::<Dest>(&*msg_buf)?;
        Ok(dest)
    }
}

pub struct RdmaConnection {
    qp: QueuePair,
}

impl RdmaConnection {
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let driver = RdmaDriver::global();
        let qp = rc_build_qp(&driver)?;

        let local_dest = local_dest(&driver.ctx, &qp, DEFAULT_IB_PORT, DEFAULT_GID_INDEX)?;
        let mut stream = TcpStream::connect(addr).await?;
        let remote_dest = exchange_dest(&mut stream, &local_dest).await?;

        rc_activate(
            &qp,
            &local_dest,
            &remote_dest,
            DEFAULT_IB_PORT,
            DEFAULT_GID_INDEX,
        )?;

        Ok(Self { qp })
    }

    pub async fn send<T>(&self, slist: T) -> (Result<()>, T)
    where
        T: ScatterList + Send + Sync,
    {
        work::send(self.qp.clone(), slist).await
    }

    pub async fn recv<T>(&self, glist: T) -> (Result<usize>, T)
    where
        T: GatherList + Send + Sync,
    {
        work::recv(self.qp.clone(), glist).await
    }
}

pub struct RdmaListener {
    tcp: TcpListener,
}

impl RdmaListener {
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let tcp = TcpListener::bind(addr).await?;
        Ok(Self { tcp })
    }

    pub async fn accept(&self) -> Result<(RdmaConnection, SocketAddr)> {
        let driver = RdmaDriver::global();

        let (mut stream, remote_addr) = self.tcp.accept().await?;
        let qp = rc_build_qp(&driver)?;

        let local_dest = local_dest(&driver.ctx, &qp, DEFAULT_IB_PORT, DEFAULT_GID_INDEX)?;
        let remote_dest = exchange_dest(&mut stream, &local_dest).await?;

        rc_activate(
            &qp,
            &local_dest,
            &remote_dest,
            DEFAULT_IB_PORT,
            DEFAULT_GID_INDEX,
        )?;

        Ok((RdmaConnection { qp }, remote_addr))
    }
}
