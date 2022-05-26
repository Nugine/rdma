use rdma_sys::ibv_wc;

#[repr(transparent)]
pub struct WorkCompletion(ibv_wc);
