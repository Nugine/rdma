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

        println!("open device");
        let ctx = dev.open()?;

        println!("close device");
        drop(ctx);

        println!()
    }

    Ok(())
}
