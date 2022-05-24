use rdma::DeviceList;

pub fn run() -> anyhow::Result<()> {
    let dev_list = DeviceList::available()?;

    if dev_list.is_empty() {
        println!("No available rdma devices");
        return Ok(());
    }

    for dev in dev_list.iter() {
        let name = dev.name();
        println!("device name: {}", name);
        println!("device guid: {:x}", dev.guid());

        let ctx = dev.open()?;

        let device_attr = ctx.query_device()?;
        println!("physical port count: {}", device_attr.physical_port_count());

        let _pd = ctx.alloc_pd()?;

        let cc = ctx.create_cc()?;

        let _cq1 = ctx.create_cq(8, 1)?;

        let _cq2 = ctx.create_cq_with_cc(8, 2, &cc)?;

        let _cq3 = ctx.create_cq_with_cc(8, 3, &cc)?;

        println!()
    }

    Ok(())
}
