use crate::work;

use rdma::cc::CompChannel;
use rdma::cq::CompletionQueue;
use rdma::ctx::Context;
use rdma::device::DeviceList;
use rdma::pd::ProtectionDomain;
use rdma::wc::WorkCompletion;

use std::mem::MaybeUninit;
use std::sync::Arc;
use std::thread;

use anyhow::Result;
use once_cell::sync::OnceCell;

pub struct RdmaDriver {
    pub(crate) ctx: Context,
    pub(crate) pd: ProtectionDomain,
    pub(crate) cc: CompChannel,
    pub(crate) cq: CompletionQueue,
}

static GLOBAL_DRIVER: OnceCell<Arc<RdmaDriver>> = OnceCell::new();

const DEFAULT_CQ_CAPACITY: usize = 1024;
const UNINIT_WC: MaybeUninit<WorkCompletion> = MaybeUninit::uninit();

impl RdmaDriver {
    pub fn global() -> Arc<RdmaDriver> {
        let driver = GLOBAL_DRIVER.get_or_init(Self::init_default);
        Arc::clone(driver)
    }

    fn init_default() -> Arc<RdmaDriver> {
        let ctx = {
            let dev_list = DeviceList::available().expect("Failed to get rdma devices");
            let dev = dev_list.first().expect("No available rdma device");
            Context::open(dev).expect("Failed to open rdma device")
        };

        let pd = ProtectionDomain::alloc(&ctx).expect("Failed to allocate protection domain");

        let cc = CompChannel::create(&ctx).expect("Failed to create completion channel");

        let cq = {
            let mut options = CompletionQueue::options();
            options.cqe(DEFAULT_CQ_CAPACITY).channel(&cc);
            CompletionQueue::create(&ctx, options).expect("Failed to create completion queue")
        };

        let driver = Arc::new(RdmaDriver { ctx, pd, cc, cq });

        {
            let driver = Arc::clone(&driver);
            thread::spawn(move || {
                driver.run().expect("rdma driver failed");
            });
        }

        driver
    }

    fn run(self: Arc<Self>) -> Result<()> {
        let mut wc_buf = [UNINIT_WC; DEFAULT_CQ_CAPACITY];

        loop {
            self.cc.wait_cq_event()?;
            self.cq.ack_cq_events(1);

            loop {
                let wcs = self.cq.poll(&mut wc_buf)?;
                if wcs.is_empty() {
                    break;
                }
                for wc in wcs {
                    unsafe { work::complete(wc) };
                }
            }
        }
    }
}
