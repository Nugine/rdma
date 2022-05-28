use crate::bindings::ibv_wc;

#[repr(transparent)]
pub struct WorkCompletion(ibv_wc);
