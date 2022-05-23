#[derive(Debug, clap::Args)]
pub struct Args {
    /// listen on/connect to port
    #[clap(long, short = 'p', default_value = "18515")]
    port: u16,

    /// use IB device (default first device found)
    #[clap(long, short = 'd')]
    ib_dev: Option<String>,

    /// use port of IB device
    #[clap(long, short = 'i', default_value = "1")]
    ib_port: u16,

    /// size of message to exchange
    #[clap(long, short = 's', default_value = "4096")]
    size: usize,

    /// path MTU
    #[clap(long, short = 'm', default_value = "1024")]
    mtu: usize,

    /// number of receives to post at a time
    #[clap(long, short = 'r', default_value = "500")]
    rx_depth: usize,

    /// number of exchanges
    #[clap(long, short = 'n', default_value = "1000")]
    iters: usize,

    /// service level value
    #[clap(long, short = 'l', default_value = "0")]
    sl: usize,

    /// sleep on CQ events (default poll)
    #[clap(long, short = 'e')]
    events: Option<usize>,

    /// local port gid index
    #[clap(long, short = 'g', default_value = "0")]
    gid_idx: usize,

    /// use on demand paging
    #[clap(long, short = 'o')]
    odp: bool,

    /// use implicit on demand paging
    #[clap(long, short = 'O')]
    iodp: bool,

    /// prefetch an ODP MR
    #[clap(long, short = 'P')]
    prefetch: bool,

    /// get CQE with timestamp
    #[clap(long, short = 't')]
    ts: bool,

    /// validate received buffer
    #[clap(long, short = 'c')]
    chk: bool,

    /// use device memory
    #[clap(long, short = 'j')]
    dm: bool,

    /// use new post send WR API
    #[clap(long, short = 'N')]
    new_send: bool,

    hostname: Option<String>,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    dbg!(args);
    todo!()
}
