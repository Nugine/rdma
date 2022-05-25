use crate::error::custom_error;
use crate::resource::{Resource, ResourceOwner};
use crate::utils::{bool_to_c_int, c_uint_to_u32};
use crate::{
    CompletionQueue, CompletionQueueOwner, Context, ContextOwner, ProtectionDomain,
    ProtectionDomainOwner,
};

use rdma_sys::{ibv_cq_ex_to_cq, ibv_create_qp_ex, ibv_destroy_qp};
use rdma_sys::{ibv_qp, ibv_qp_cap, ibv_qp_init_attr_ex};
use rdma_sys::{
    IBV_QPT_DRIVER,   //
    IBV_QPT_RC,       //
    IBV_QPT_UC,       //
    IBV_QPT_UD,       //
    IBV_QPT_XRC_RECV, //
    IBV_QPT_XRC_SEND, //
};

use std::cell::UnsafeCell;
use std::os::raw::c_uint;
use std::ptr::NonNull;
use std::{io, mem};

use asc::Asc;

#[derive(Clone)]
pub struct QueuePair(pub(crate) Resource<QueuePairOwner>);

impl QueuePair {
    #[inline]
    #[must_use]
    pub fn options() -> QueuePairOptions {
        QueuePairOptions::default()
    }

    #[inline]
    pub fn create(ctx: &Context, options: QueuePairOptions) -> io::Result<Self> {
        let owner = QueuePairOwner::create(ctx, options)?;
        Ok(Self(Resource::new(owner)))
    }

    #[inline]
    #[must_use]
    pub fn id(&self) -> QueuePairId {
        let qp = self.0.ctype();
        // SAFETY: reading a immutable field of a concurrent ffi type
        QueuePairId(unsafe { (*qp).qp_num })
    }

    // FIXME: clippy false positive: https://github.com/rust-lang/rust-clippy/issues/8622
    #[allow(clippy::transmutes_expressible_as_ptr_casts)]
    #[inline]
    #[must_use]
    pub fn user_data(&self) -> usize {
        let qp = self.0.ctype();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { mem::transmute((*qp).qp_context) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueuePairId(u32);

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum QueuePairType {
    RC = c_uint_to_u32(IBV_QPT_RC),
    UC = c_uint_to_u32(IBV_QPT_UC),
    UD = c_uint_to_u32(IBV_QPT_UD),
    Driver = c_uint_to_u32(IBV_QPT_DRIVER),
    XrcRecv = c_uint_to_u32(IBV_QPT_XRC_RECV),
    XrcSend = c_uint_to_u32(IBV_QPT_XRC_SEND),
}

impl QueuePairType {
    #[allow(clippy::as_conversions)]
    fn to_c_uint(self) -> c_uint {
        self as u32 as c_uint
    }
}

pub struct QueuePairOptions {
    user_data: usize,
    send_cq: Option<Asc<CompletionQueueOwner>>,
    recv_cq: Option<Asc<CompletionQueueOwner>>,
    qp_type: Option<QueuePairType>,
    sq_sig_all: Option<bool>,
    cap: ibv_qp_cap,
    pd: Option<Asc<ProtectionDomainOwner>>,
}

impl Default for QueuePairOptions {
    #[inline]
    fn default() -> Self {
        Self {
            user_data: 0,
            send_cq: None,
            recv_cq: None,
            qp_type: None,
            sq_sig_all: None,
            // SAFETY: POD ffi type
            cap: unsafe { mem::zeroed() },
            pd: None,
        }
    }
}

impl QueuePairOptions {
    #[inline]
    pub fn user_data(&mut self, user_data: usize) -> &mut Self {
        self.user_data = user_data;
        self
    }
    #[inline]
    pub fn send_cq(&mut self, send_cq: &CompletionQueue) -> &mut Self {
        self.send_cq = Some(send_cq.0.strong_ref());
        self
    }
    #[inline]
    pub fn recv_cq(&mut self, recv_cq: &CompletionQueue) -> &mut Self {
        self.recv_cq = Some(recv_cq.0.strong_ref());
        self
    }
    #[inline]
    pub fn qp_type(&mut self, qp_type: QueuePairType) -> &mut Self {
        self.qp_type = Some(qp_type);
        self
    }
    #[inline]
    pub fn sq_sig_all(&mut self, sq_sig_all: bool) -> &mut Self {
        self.sq_sig_all = Some(sq_sig_all);
        self
    }
    #[inline]
    pub fn pd(&mut self, pd: &ProtectionDomain) -> &mut Self {
        self.pd = Some(pd.0.strong_ref());
        self
    }
    #[inline]
    pub fn max_send_wr(&mut self, max_send_wr: u32) -> &mut Self {
        self.cap.max_send_wr = max_send_wr;
        self
    }
    #[inline]
    pub fn max_recv_wr(&mut self, max_recv_wr: u32) -> &mut Self {
        self.cap.max_recv_wr = max_recv_wr;
        self
    }
    #[inline]
    pub fn max_send_sge(&mut self, max_send_sge: u32) -> &mut Self {
        self.cap.max_send_sge = max_send_sge;
        self
    }
    #[inline]
    pub fn max_recv_sge(&mut self, max_recv_sge: u32) -> &mut Self {
        self.cap.max_recv_sge = max_recv_sge;
        self
    }
    #[inline]
    pub fn max_inline_data(&mut self, max_inline_data: u32) -> &mut Self {
        self.cap.max_inline_data = max_inline_data;
        self
    }
}

pub(crate) struct QueuePairOwner {
    qp: NonNull<UnsafeCell<ibv_qp>>,

    _ctx: Asc<ContextOwner>,
    _pd: Option<Asc<ProtectionDomainOwner>>,
    _send_cq: Option<Asc<CompletionQueueOwner>>,
    _recv_cq: Option<Asc<CompletionQueueOwner>>,
}

/// SAFETY: owned type
unsafe impl Send for QueuePairOwner {}
/// SAFETY: owned type
unsafe impl Sync for QueuePairOwner {}

/// SAFETY: resource owner
unsafe impl ResourceOwner for QueuePairOwner {
    type Ctype = ibv_qp;

    fn ctype(&self) -> *mut Self::Ctype {
        self.qp.as_ptr().cast()
    }
}

impl QueuePairOwner {
    fn create(ctx: &Context, options: QueuePairOptions) -> io::Result<Self> {
        assert!(options.qp_type.is_some(), "qp_type is required");
        assert!(options.sq_sig_all.is_some(), "sq_sig_all is required");
        // SAFETY: ffi
        unsafe {
            let context = ctx.0.ffi_ptr();

            let mut qp_attr: ibv_qp_init_attr_ex = mem::zeroed();
            qp_attr.qp_context = mem::transmute(options.user_data);
            if let Some(ref send_cq) = options.send_cq {
                qp_attr.send_cq = ibv_cq_ex_to_cq(send_cq.ctype());
            }
            if let Some(ref recv_cq) = options.recv_cq {
                qp_attr.recv_cq = ibv_cq_ex_to_cq(recv_cq.ctype());
            }
            qp_attr.qp_type = options.qp_type.unwrap_unchecked().to_c_uint();
            qp_attr.sq_sig_all = bool_to_c_int(options.sq_sig_all.unwrap_unchecked());
            qp_attr.cap = options.cap;
            if let Some(ref pd) = options.pd {
                qp_attr.pd = pd.ctype();
            }

            let qp = ibv_create_qp_ex(context, &mut qp_attr);
            if qp.is_null() {
                return Err(custom_error("failed to create queue pair"));
            }
            let qp = NonNull::new_unchecked(qp.cast());
            Ok(Self {
                qp,
                _ctx: ctx.0.strong_ref(),
                _pd: options.pd,
                _send_cq: options.send_cq,
                _recv_cq: options.recv_cq,
            })
        }
    }
}

impl Drop for QueuePairOwner {
    fn drop(&mut self) {
        // SAFETY: ffi
        let ret = unsafe { ibv_destroy_qp(self.ctype()) };
        assert_eq!(ret, 0);
    }
}
