use rdma::DeviceList;

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

        println!("{indent}device guid: {:x}", dev.guid());
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
                println!("{indent} gid index: {}", gid_index);
                indent.push();

                let gid_entry = ctx.query_gid_entry(port_num, gid_index)?;

                println!("{indent}gid type: {:?}", gid_entry.gid_type());
                println!("{indent}gid: {:?}", gid_entry.gid());

                indent.pop();
            }

            indent.pop();
        }
        indent.pop();

        let _pd = ctx.alloc_pd()?;

        let cc = ctx.create_cc()?;

        let _cq1 = ctx.create_cq(8, 1)?;

        let _cq2 = ctx.create_cq_with_cc(8, 2, &cc)?;

        let _cq3 = ctx.create_cq_with_cc(8, 3, &cc)?;

        println!()
    }

    Ok(())
}
