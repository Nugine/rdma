use rdma::{CompletionQueue, DeviceList};

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
        indent.pop();

        let _pd = ctx.alloc_pd()?;

        let cc = ctx.create_cc()?;

        let _cq1 = {
            let mut options: _ = CompletionQueue::options();
            options.cqe(8).user_data(1);
            ctx.create_cq(options)?
        };

        let _cq2 = {
            let mut options: _ = CompletionQueue::options();
            options.cqe(8).user_data(2).channel(&cc);
            ctx.create_cq(options)?
        };

        let _cq3 = {
            let mut options: _ = CompletionQueue::options();
            options.cqe(8).user_data(3).channel(&cc);
            ctx.create_cq(options)?
        };

        println!()
    }

    Ok(())
}
