use tabled::Table;
use tabled::Tabled;

#[derive(Tabled)]
struct DeviceInfo {
    name: String,
    guid: String,
}

pub fn run() -> anyhow::Result<()> {
    let dev_list = rdma::device::DeviceList::available()?;

    if dev_list.is_empty() {
        println!("No available rdma devices");
        return Ok(());
    }

    let iter = dev_list.iter().map(|dev| DeviceInfo {
        name: dev.name().to_owned(),
        guid: format!("{:x}", dev.guid()),
    });

    let table = Table::new(iter);
    print!("{}", table);

    Ok(())
}
