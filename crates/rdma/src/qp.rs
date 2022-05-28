use crate::bindings as C;
use crate::cq::{self, CompletionQueue};
use crate::error::{create_resource, from_errno};
use crate::pd::{self, ProtectionDomain};
use crate::resource::Resource;
use crate::utils::{
    bool_to_c_int, c_uint_to_u32, u32_as_c_uint, usize_to_void_ptr, void_ptr_to_usize,
};
use crate::wr::{RecvRequest, SendRequest};

use std::os::raw::{c_int, c_uint};
use std::ptr::{self, NonNull};
use std::sync::Arc;
use std::{io, mem};

#[derive(Clone)]
pub struct QueuePair(Arc<Owner>);

/// SAFETY: resource type
unsafe impl Resource for QueuePair {
    type Owner = Owner;

    fn as_owner(&self) -> &Arc<Self::Owner> {
        &self.0
    }
}

impl QueuePair {
    pub(crate) fn ffi_ptr(&self) -> *mut C::ibv_qp {
        self.0.ffi_ptr()
    }

    #[inline]
    #[must_use]
    pub fn options() -> QueuePairOptions {
        QueuePairOptions::default()
    }

    /// # Panics
    /// 1. if the option `pd` is not set
    /// 2. if the option `qp_type` is not set
    /// 3. if the option `sq_sig_all` is not set
    #[inline]
    pub fn create(pd: &ProtectionDomain, options: QueuePairOptions) -> io::Result<Self> {
        assert!(options.qp_type.is_some(), "qp_type is required");
        assert!(options.sq_sig_all.is_some(), "sq_sig_all is required");
        // SAFETY: ffi
        let owner = unsafe {
            let context = (*pd.ffi_ptr()).context;

            let mut qp_attr: C::ibv_qp_init_attr_ex = mem::zeroed();

            qp_attr.pd = pd.ffi_ptr();

            qp_attr.qp_context = usize_to_void_ptr(options.user_data);

            if let Some(ref send_cq) = options.send_cq {
                qp_attr.send_cq = C::ibv_cq_ex_to_cq(send_cq.ffi_ptr());
            }

            if let Some(ref recv_cq) = options.recv_cq {
                qp_attr.recv_cq = C::ibv_cq_ex_to_cq(recv_cq.ffi_ptr());
            }

            qp_attr.qp_type = options.qp_type.unwrap_unchecked().to_c_uint();
            qp_attr.sq_sig_all = bool_to_c_int(options.sq_sig_all.unwrap_unchecked());
            qp_attr.cap = options.cap;
            qp_attr.comp_mask = C::IBV_QP_INIT_ATTR_PD;

            let qp = create_resource(
                || C::ibv_create_qp_ex(context, &mut qp_attr),
                || "failed to create queue pair",
            )?;

            Arc::new(Owner {
                qp,
                _pd: pd.strong_ref(),
                _send_cq: options.send_cq,
                _recv_cq: options.recv_cq,
            })
        };
        Ok(Self(owner))
    }

    #[inline]
    #[must_use]
    pub fn number(&self) -> QueuePairNumber {
        let qp = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        QueuePairNumber(unsafe { (*qp).qp_num })
    }

    #[inline]
    #[must_use]
    pub fn user_data(&self) -> usize {
        let qp = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { void_ptr_to_usize((*qp).qp_context) }
    }

    /// # Safety
    /// TODO
    #[inline]
    pub unsafe fn post_send(&self, send_wr: &mut SendRequest) -> io::Result<()> {
        let qp = self.ffi_ptr();
        let wr: *mut C::ibv_send_wr = <*mut SendRequest>::cast(send_wr);
        let mut bad_wr: *mut C::ibv_send_wr = ptr::null_mut();
        let ret = C::ibv_post_send(qp, wr, &mut bad_wr);
        if ret != 0 {
            return Err(from_errno(ret));
        }
        Ok(())
    }

    /// # Safety
    /// TODO
    #[inline]
    pub unsafe fn post_recv(&self, recv_wr: &mut RecvRequest) -> io::Result<()> {
        let qp = self.ffi_ptr();
        let wr: *mut C::ibv_recv_wr = <*mut RecvRequest>::cast(recv_wr);
        let mut bad_wr: *mut C::ibv_recv_wr = ptr::null_mut();
        let ret = C::ibv_post_recv(qp, wr, &mut bad_wr);
        if ret != 0 {
            return Err(from_errno(ret));
        }
        Ok(())
    }

    #[inline]
    pub fn modify(&self, options: ModifyOptions) -> io::Result<()> {
        let qp = self.ffi_ptr();
        // SAFETY: ffi
        unsafe {
            let attr_mask: c_int = mem::transmute(options.mask);
            let mut attr = options.attr;
            let ret = C::ibv_modify_qp(qp, &mut attr, attr_mask);
            if ret != 0 {
                return Err(from_errno(ret));
            }
            Ok(())
        }
    }
}

pub(crate) struct Owner {
    qp: NonNull<C::ibv_qp>,

    _pd: Arc<pd::Owner>,
    _send_cq: Option<Arc<cq::Owner>>,
    _recv_cq: Option<Arc<cq::Owner>>,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    fn ffi_ptr(&self) -> *mut C::ibv_qp {
        self.qp.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let qp: *mut C::ibv_qp = self.ffi_ptr();
            let ret = C::ibv_destroy_qp(qp);
            assert_eq!(ret, 0);
        }
    }
}

pub struct QueuePairOptions {
    user_data: usize,
    send_cq: Option<Arc<cq::Owner>>,
    recv_cq: Option<Arc<cq::Owner>>,
    qp_type: Option<QueuePairType>,
    sq_sig_all: Option<bool>,
    cap: C::ibv_qp_cap,
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
pub struct QueuePairNumber(u32);

impl QueuePairNumber {
    #[inline]
    #[must_use]
    pub fn raw_value(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum QueuePairType {
    RC = c_uint_to_u32(C::IBV_QPT_RC),
    UC = c_uint_to_u32(C::IBV_QPT_UC),
    UD = c_uint_to_u32(C::IBV_QPT_UD),
    Driver = c_uint_to_u32(C::IBV_QPT_DRIVER),
    XrcRecv = c_uint_to_u32(C::IBV_QPT_XRC_RECV),
    XrcSend = c_uint_to_u32(C::IBV_QPT_XRC_SEND),
}

impl QueuePairType {
    fn to_c_uint(self) -> c_uint {
        #[allow(clippy::as_conversions)]
        u32_as_c_uint(self as u32)
    }
}

#[repr(C)]
pub struct ModifyOptions {
    mask: C::ibv_qp_attr_mask,
    attr: C::ibv_qp_attr,
}

impl Default for ModifyOptions {
    #[inline]
    fn default() -> Self {
        // SAFETY: POD ffi type
        unsafe { mem::zeroed() }
    }
}
