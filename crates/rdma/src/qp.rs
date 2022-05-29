use crate::bindings as C;
use crate::cq::{self, CompletionQueue};
use crate::error::{create_resource, from_errno};
use crate::mr::AccessFlags;
use crate::pd::{self, ProtectionDomain};
use crate::resource::Resource;
use crate::utils::{bool_to_c_int, c_uint_to_u32, u32_as_c_uint};
use crate::utils::{usize_to_void_ptr, void_ptr_to_usize};
use crate::wr::{RecvRequest, SendRequest};

use std::mem::MaybeUninit;
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
    /// 1. if the option `qp_type` is not set
    /// 2. if the option `sq_sig_all` is not set
    #[inline]
    pub fn create(pd: &ProtectionDomain, mut options: QueuePairOptions) -> io::Result<Self> {
        assert!(options.has_qp_type, "qp_type is required");
        assert!(options.has_sq_sig_all, "sq_sig_all is required");
        // SAFETY: ffi
        let owner = unsafe {
            let context = (*pd.ffi_ptr()).context;

            let qp_attr = &mut options.attr;

            qp_attr.pd = pd.ffi_ptr();
            qp_attr.comp_mask = C::IBV_QP_INIT_ATTR_PD;

            let qp = create_resource(
                || C::ibv_create_qp_ex(context, qp_attr),
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
    pub fn modify(&self, mut options: ModifyOptions) -> io::Result<()> {
        let qp = self.ffi_ptr();
        // SAFETY: ffi
        unsafe {
            let attr_mask: c_int = mem::transmute(options.mask);
            let attr = options.attr.as_mut_ptr();
            let ret = C::ibv_modify_qp(qp, attr, attr_mask);
            if ret != 0 {
                return Err(from_errno(ret));
            }
            Ok(())
        }
    }

    #[inline]
    pub fn query(&self, options: QueryOptions) -> io::Result<QueuePairAttr> {
        let qp = self.ffi_ptr();
        // SAFETY: ffi
        unsafe {
            let attr_mask: c_int = mem::transmute(options.mask);
            let mut attr: QueuePairAttr = mem::zeroed();
            let mut init_attr: C::ibv_qp_init_attr = mem::zeroed();
            let ret = C::ibv_query_qp(qp, &mut attr.attr, attr_mask, &mut init_attr);
            if ret != 0 {
                return Err(from_errno(ret));
            }
            attr.mask = options.mask;
            Ok(attr)
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

#[derive(Clone)]
#[repr(C)]
pub struct QueuePairCapacity {
    pub max_send_wr: u32,
    pub max_recv_wr: u32,
    pub max_send_sge: u32,
    pub max_recv_sge: u32,
    pub max_inline_data: u32,
}

// layout test
const _: () = {
    assert!(mem::size_of::<QueuePairCapacity>() == mem::size_of::<C::ibv_qp_cap>());
    assert!(mem::align_of::<QueuePairCapacity>() == mem::align_of::<C::ibv_qp_cap>());
    let cap = QueuePairCapacity {
        max_send_wr: 0,
        max_recv_wr: 0,
        max_send_sge: 0,
        max_recv_sge: 0,
        max_inline_data: 0,
    };
    let _ = C::ibv_qp_cap {
        max_send_wr: cap.max_send_wr,
        max_recv_wr: cap.max_recv_wr,
        max_send_sge: cap.max_send_sge,
        max_recv_sge: cap.max_recv_sge,
        max_inline_data: cap.max_inline_data,
    };
};

impl Default for QueuePairCapacity {
    #[inline]
    fn default() -> Self {
        // SAFETY: POD ffi type
        unsafe { mem::zeroed() }
    }
}

impl QueuePairCapacity {
    fn into_ctype(self) -> C::ibv_qp_cap {
        // SAFETY: same repr
        unsafe { mem::transmute(self) }
    }
    fn from_ctype_ref(cap: &C::ibv_qp_cap) -> &Self {
        // SAFETY: same repr
        unsafe { mem::transmute(cap) }
    }
}

pub struct QueuePairOptions {
    attr: C::ibv_qp_init_attr_ex,

    has_qp_type: bool,
    has_sq_sig_all: bool,

    send_cq: Option<Arc<cq::Owner>>,
    recv_cq: Option<Arc<cq::Owner>>,
}

// SAFETY: owned type
unsafe impl Send for QueuePairOptions {}
// SAFETY: owned type
unsafe impl Sync for QueuePairOptions {}

impl Default for QueuePairOptions {
    #[inline]
    fn default() -> Self {
        Self {
            // SAFETY: POD ffi type
            attr: unsafe { mem::zeroed() },
            has_qp_type: false,
            has_sq_sig_all: false,
            send_cq: None,
            recv_cq: None,
        }
    }
}

impl QueuePairOptions {
    #[inline]
    pub fn user_data(&mut self, user_data: usize) -> &mut Self {
        self.attr.qp_context = usize_to_void_ptr(user_data);
        self
    }
    #[inline]
    pub fn send_cq(&mut self, send_cq: &CompletionQueue) -> &mut Self {
        self.attr.send_cq = C::ibv_cq_ex_to_cq(send_cq.ffi_ptr());
        self.send_cq = Some(send_cq.strong_ref());
        self
    }
    #[inline]
    pub fn recv_cq(&mut self, recv_cq: &CompletionQueue) -> &mut Self {
        self.attr.recv_cq = C::ibv_cq_ex_to_cq(recv_cq.ffi_ptr());
        self.recv_cq = Some(recv_cq.strong_ref());
        self
    }
    #[inline]
    pub fn qp_type(&mut self, qp_type: QueuePairType) -> &mut Self {
        self.attr.qp_type = qp_type.to_c_uint();
        self.has_qp_type = true;
        self
    }
    #[inline]
    pub fn sq_sig_all(&mut self, sq_sig_all: bool) -> &mut Self {
        self.attr.sq_sig_all = bool_to_c_int(sq_sig_all);
        self.has_sq_sig_all = true;
        self
    }
    #[inline]
    pub fn cap(&mut self, cap: QueuePairCapacity) -> &mut Self {
        self.attr.cap = cap.into_ctype();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueuePairNumber(u32);

impl QueuePairNumber {
    #[inline]
    #[must_use]
    pub fn new(raw_value: u32) -> QueuePairNumber {
        Self(raw_value)
    }

    #[inline]
    #[must_use]
    pub fn raw_value(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    attr: MaybeUninit<C::ibv_qp_attr>,
}

// SAFETY: owned type
unsafe impl Send for ModifyOptions {}
// SAFETY: owned type
unsafe impl Sync for ModifyOptions {}

impl Default for ModifyOptions {
    #[inline]
    fn default() -> Self {
        // SAFETY: POD ffi type
        Self {
            mask: 0,
            attr: MaybeUninit::uninit(),
        }
    }
}

impl ModifyOptions {
    #[inline]
    pub fn qp_state(&mut self, qp_state: QueuePairState) -> &mut Self {
        // SAFETY: write uninit field
        unsafe {
            let p = ptr::addr_of_mut!((*self.attr.as_mut_ptr()).qp_state);
            p.write(qp_state.to_c_uint());
        }
        self.mask |= C::IBV_QP_STATE;
        self
    }

    #[inline]
    pub fn pkey_index(&mut self, pkey_index: u16) -> &mut Self {
        // SAFETY: write uninit field
        unsafe {
            let p = ptr::addr_of_mut!((*self.attr.as_mut_ptr()).pkey_index);
            p.write(pkey_index);
        }
        self.mask |= C::IBV_QP_PKEY_INDEX;
        self
    }

    #[inline]
    pub fn port_num(&mut self, port_num: u8) -> &mut Self {
        // SAFETY: write uninit field
        unsafe {
            let p = ptr::addr_of_mut!((*self.attr.as_mut_ptr()).port_num);
            p.write(port_num);
        }
        self.mask |= C::IBV_QP_PORT;
        self
    }

    #[inline]
    pub fn qp_access_flags(&mut self, access_flags: AccessFlags) -> &mut Self {
        // SAFETY: write uninit field
        unsafe {
            let p = ptr::addr_of_mut!((*self.attr.as_mut_ptr()).qp_access_flags);
            p.write(access_flags.to_c_uint());
        }
        self.mask |= C::IBV_QP_ACCESS_FLAGS;
        self
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct QueryOptions {
    mask: C::ibv_qp_attr_mask,
}

impl Default for QueryOptions {
    #[inline]
    fn default() -> Self {
        // SAFETY: POD ffi type
        unsafe { mem::zeroed() }
    }
}

impl QueryOptions {
    #[inline]
    pub fn cap(&mut self) -> &mut Self {
        self.mask |= C::IBV_QP_CAP;
        self
    }

    #[inline]
    pub fn qp_state(&mut self) -> &mut Self {
        self.mask |= C::IBV_QP_STATE;
        self
    }
}

#[repr(C)]
pub struct QueuePairAttr {
    mask: C::ibv_qp_attr_mask,
    attr: C::ibv_qp_attr,
}

// SAFETY: owned type
unsafe impl Send for QueuePairAttr {}
// SAFETY: owned type
unsafe impl Sync for QueuePairAttr {}

impl QueuePairAttr {
    #[inline]
    #[must_use]
    pub fn cap(&self) -> Option<&QueuePairCapacity> {
        (self.mask & C::IBV_QP_CAP != 0).then(|| QueuePairCapacity::from_ctype_ref(&self.attr.cap))
    }

    #[inline]
    #[must_use]
    pub fn qp_state(&self) -> Option<QueuePairState> {
        (self.mask & C::IBV_QP_STATE != 0).then(|| QueuePairState::from_c_uint(self.attr.qp_state))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QueuePairState {
    Reset = c_uint_to_u32(C::IBV_QPS_RESET),
    Initialize = c_uint_to_u32(C::IBV_QPS_INIT),
    ReadyToReceive = c_uint_to_u32(C::IBV_QPS_RTR),
    ReadyToSend = c_uint_to_u32(C::IBV_QPS_RTS),
    SendQueueDrained = c_uint_to_u32(C::IBV_QPS_SQD),
    SendQueueError = c_uint_to_u32(C::IBV_QPS_SQE),
    Error = c_uint_to_u32(C::IBV_QPS_ERR),
    Unknown = c_uint_to_u32(C::IBV_QPS_UNKNOWN), // ASK: what is this
}

impl QueuePairState {
    fn from_c_uint(val: c_uint) -> Self {
        match val {
            C::IBV_QPS_RESET => Self::Reset,
            C::IBV_QPS_INIT => Self::Initialize,
            C::IBV_QPS_RTR => Self::ReadyToReceive,
            C::IBV_QPS_RTS => Self::ReadyToSend,
            C::IBV_QPS_SQD => Self::SendQueueDrained,
            C::IBV_QPS_SQE => Self::SendQueueError,
            C::IBV_QPS_ERR => Self::Error,
            _ => panic!("unexpected queue pair state"),
        }
    }

    fn to_c_uint(self) -> c_uint {
        #[allow(clippy::as_conversions)]
        u32_as_c_uint(self as u32)
    }
}
