use rdma::cq::CompletionQueue;
use rdma::device::DeviceList;
use rdma::qp::{QueuePair, QueuePairType};

use std::fmt;

struct Indent {
    width: usize,
    level: usize,
}

impl Indent {
    fn push(&mut self) {
        self.level = self.level.saturating_add(self.width)
    }
    fn pop(&mut self) {
        self.level = self.level.saturating_sub(self.width)
    }
}

impl fmt::Display for Indent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indent = self.level;
        write!(f, "{:indent$}", "")
    }
}

pub fn run() -> anyhow::Result<()> {
    let dev_list = DeviceList::available()?;

    if dev_list.is_empty() {
        println!("No available rdma devices");
        return Ok(());
    }

    let mut indent = Indent { width: 4, level: 0 };

    for dev in dev_list.iter() {
        let name = dev.name();
        println!("{indent}device name: {}", name);
        indent.push();

        println!("{indent}guid: {:x}", dev.guid());
        let ctx = dev.open()?;

        let device_attr = ctx.query_device()?;
        println!(
            "{indent}physical port count: {}",
            device_attr.physical_port_count()
        );

        for port_num in 1..=device_attr.physical_port_count() {
            let port_attr = ctx.query_port(port_num)?;
            println!("{indent}port {}:", port_num);

            indent.push();

            println!(
                "{indent}logical port state ({}): {:?}",
                port_num,
                port_attr.state()
            );
            println!("{indent}link layer: {:?}", port_attr.link_layer());

            for gid_index in 0..port_attr.gid_table_len() {
                match ctx.query_gid_entry(port_num, gid_index) {
                    Ok(gid_entry) => {
                        println!("{indent}gid index: {}", gid_index);
                        indent.push();
                        println!("{indent}type: {:?}", gid_entry.gid_type());
                        println!("{indent}gid: {:?}", gid_entry.gid());
                        indent.pop();
                        continue;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }

            indent.pop();
        }

        let pd = ctx.alloc_pd()?;

        let cc = ctx.create_cc()?;

        let cq1 = {
            let mut options: _ = CompletionQueue::options();
            options.cqe(8).user_data(1);
            ctx.create_cq(options)?
        };

        let cq2 = {
            let mut options: _ = CompletionQueue::options();
            options.cqe(8).user_data(2).channel(&cc);
            ctx.create_cq(options)?
        };

        let cq3 = {
            let mut options: _ = CompletionQueue::options();
            options.cqe(8).user_data(3).channel(&cc);
            ctx.create_cq(options)?
        };

        let qp1 = {
            let mut options: _ = QueuePair::options();
            options
                .user_data(1)
                .send_cq(&cq1)
                .recv_cq(&cq1)
                .pd(&pd)
                .qp_type(QueuePairType::RC)
                .sq_sig_all(true)
                .max_send_wr(8)
                .max_recv_wr(8)
                .max_send_sge(8)
                .max_recv_sge(8)
                .max_inline_data(8);
            ctx.create_qp(options)?
        };
        println!("{indent}qp: {:?}", qp1.id());

        let qp2 = {
            let mut options: _ = QueuePair::options();
            options
                .user_data(2)
                .send_cq(&cq2)
                .recv_cq(&cq3)
                .pd(&pd)
                .qp_type(QueuePairType::UD)
                .sq_sig_all(true)
                .max_send_wr(8)
                .max_recv_wr(8)
                .max_send_sge(8)
                .max_recv_sge(8)
                .max_inline_data(8);
            ctx.create_qp(options)?
        };
        println!("{indent}qp: {:?}", qp2.id());

        indent.pop();
        println!()
    }

    Ok(())
}
