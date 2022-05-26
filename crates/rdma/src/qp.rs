use crate::error::create_resource;
use crate::resource::Resource;
use crate::utils::{bool_to_c_int, c_uint_to_u32};
use crate::CompletionQueue;
use crate::Context;
use crate::ProtectionDomain;

use rdma_sys::IBV_QP_INIT_ATTR_PD;
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
pub struct QueuePair(Asc<Inner>);

impl QueuePair {
    #[inline]
    #[must_use]
    pub fn options() -> QueuePairOptions {
        QueuePairOptions::default()
    }

    #[inline]
    pub fn create(ctx: &Context, options: QueuePairOptions) -> io::Result<Self> {
        let inner = Inner::create(ctx, options)?;
        Ok(Self(Asc::new(inner)))
    }

    #[inline]
    #[must_use]
    pub fn id(&self) -> QueuePairId {
        let qp = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        QueuePairId(unsafe { (*qp).qp_num })
    }

    // FIXME: clippy false positive: https://github.com/rust-lang/rust-clippy/issues/8622
    #[allow(clippy::transmutes_expressible_as_ptr_casts)]
    #[inline]
    #[must_use]
    pub fn user_data(&self) -> usize {
        let qp = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { mem::transmute((*qp).qp_context) }
    }
}

/// SAFETY: shared resource type
unsafe impl Resource for QueuePair {
    type Ctype = ibv_qp;

    fn ffi_ptr(&self) -> *mut Self::Ctype {
        self.0.qp.as_ptr().cast()
    }

    fn strong_ref(&self) -> Self {
        Self(Asc::clone(&self.0))
    }
}

struct Inner {
    qp: NonNull<UnsafeCell<ibv_qp>>,

    _ctx: Context,
    _pd: Option<ProtectionDomain>,
    _send_cq: Option<CompletionQueue>,
    _recv_cq: Option<CompletionQueue>,
}

/// SAFETY: owned type
unsafe impl Send for Inner {}
/// SAFETY: owned type
unsafe impl Sync for Inner {}

impl Inner {
    fn create(ctx: &Context, options: QueuePairOptions) -> io::Result<Self> {
        assert!(options.pd.is_some(), "pd is required");
        assert!(options.qp_type.is_some(), "qp_type is required");
        assert!(options.sq_sig_all.is_some(), "sq_sig_all is required");
        // SAFETY: ffi
        unsafe {
            let context = ctx.ffi_ptr();

            let mut qp_attr: ibv_qp_init_attr_ex = mem::zeroed();
            qp_attr.qp_context = mem::transmute(options.user_data);
            if let Some(ref send_cq) = options.send_cq {
                qp_attr.send_cq = ibv_cq_ex_to_cq(send_cq.ffi_ptr());
            }
            if let Some(ref recv_cq) = options.recv_cq {
                qp_attr.recv_cq = ibv_cq_ex_to_cq(recv_cq.ffi_ptr());
            }
            qp_attr.qp_type = options.qp_type.unwrap_unchecked().to_c_uint();
            qp_attr.sq_sig_all = bool_to_c_int(options.sq_sig_all.unwrap_unchecked());
            qp_attr.cap = options.cap;
            qp_attr.pd = options.pd.as_ref().unwrap_unchecked().ffi_ptr();
            qp_attr.comp_mask = IBV_QP_INIT_ATTR_PD;

            let qp = create_resource(
                || ibv_create_qp_ex(context, &mut qp_attr),
                || "failed to create queue pair",
            )?;

            Ok(Self {
                qp: qp.cast(),
                _ctx: ctx.strong_ref(),
                _pd: options.pd,
                _send_cq: options.send_cq,
                _recv_cq: options.recv_cq,
            })
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let qp: *mut ibv_qp = self.qp.as_ptr().cast();
            let ret = ibv_destroy_qp(qp);
            assert_eq!(ret, 0);
        }
    }
}

pub struct QueuePairOptions {
    user_data: usize,
    send_cq: Option<CompletionQueue>,
    recv_cq: Option<CompletionQueue>,
    qp_type: Option<QueuePairType>,
    sq_sig_all: Option<bool>,
    cap: ibv_qp_cap,
    pd: Option<ProtectionDomain>,
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
        self.send_cq = Some(send_cq.strong_ref());
        self
    }
    #[inline]
    pub fn recv_cq(&mut self, recv_cq: &CompletionQueue) -> &mut Self {
        self.recv_cq = Some(recv_cq.strong_ref());
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
        self.pd = Some(pd.strong_ref());
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
