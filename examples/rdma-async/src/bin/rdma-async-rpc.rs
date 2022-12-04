#![forbid(unsafe_code)]
#![deny(clippy::all)]

use rdma_async::{Buf, RdmaConnection, RdmaListener};

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::{env, io};

use anyhow::Result;
use numeric_cast::NumericCast;
use serde::{Deserialize, Serialize};
use tokio::spawn;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    if env::var("RUST_BACKTRACE").is_err() {
        env::set_var("RUST_BACKTRACE", "full")
    }
    if env::var("RUST_LOG").is_err() {
        env::set_var(
            "RUST_LOG",
            "rdma_async_rpc=trace,rdma_async=trace,rdma=trace",
        )
    }

    tracing_subscriber::fmt::init();

    let addr = SocketAddr::from(([127, 0, 0, 1], 18515));

    spawn(server(addr));

    tokio::time::sleep(Duration::from_millis(100)).await;

    let _ = spawn(client(addr)).await;

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Message {
    Ping(String),
    Pong(String),
    Exit,
}

async fn server(addr: SocketAddr) -> Result<()> {
    let listener = RdmaListener::bind(addr).await?;

    loop {
        let (conn, remote_addr) = listener.accept().await?;
        info!("server accepted connection from {}", remote_addr);

        spawn(async move {
            let mut recv_result;
            let mut send_result;
            let mut head;
            let mut buf = Buf::new_zeroed(1024, 8);

            loop {
                (recv_result, buf) = conn.recv(buf).await;
                let (nread, _) = recv_result?;

                let req: Message = bincode::deserialize(&buf.as_slice()[..nread])?;

                info!("server received: {:?}", req);

                match req {
                    Message::Ping(msg) => {
                        let res = Message::Pong(msg);

                        let mut writer = io::Cursor::new(buf.as_slice_mut());
                        bincode::serialize_into(&mut writer, &res)?;
                        let nbytes: usize = writer.position().numeric_cast();

                        (send_result, head) = conn.send(buf.head(nbytes), None).await;
                        send_result?;
                        buf = head.into_inner();
                    }
                    Message::Pong { .. } => {}
                    Message::Exit => break,
                }
            }

            anyhow::Result::<()>::Ok(())
        });
    }
}

async fn client(addr: SocketAddr) -> Result<()> {
    let conn = Arc::new(RdmaConnection::connect(addr).await?);
    let mut recv_result;
    let mut send_result;
    let mut head;
    let mut buf = Buf::new_zeroed(1024, 8);

    for i in 1..=64 {
        let req = Message::Ping(format!("iter {i}"));

        info!("client send    : {:?}", req);

        let mut writer = io::Cursor::new(buf.as_slice_mut());
        bincode::serialize_into(&mut writer, &req)?;
        let nbytes: usize = writer.position().numeric_cast();

        (send_result, head) = conn.send(buf.head(nbytes), None).await;
        send_result?;
        buf = head.into_inner();

        (recv_result, buf) = conn.recv(buf).await;
        let (nread, _) = recv_result?;

        let res: Message = bincode::deserialize(&buf.as_slice()[..nread])?;
        info!("client received: {:?}", res);
    }

    {
        let req = Message::Exit;

        let mut writer = io::Cursor::new(buf.as_slice_mut());
        bincode::serialize_into(&mut writer, &req)?;
        let nbytes: usize = writer.position().numeric_cast();

        (send_result, _) = conn.send(buf.head(nbytes), None).await;
        send_result?;
    }

    Ok(())
}
