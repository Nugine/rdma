use crate::bindings as C;

#[repr(transparent)]
pub struct WorkCompletion(C::ibv_wc);
