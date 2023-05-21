//! libibverbs 1.14.41
//! static inline functions
#![allow(
    clippy::as_conversions,
    clippy::unneeded_field_pattern,
    clippy::undocumented_unsafe_blocks,
    clippy::arithmetic_side_effects,
    clippy::inline_always,
    clippy::shadow_same,
    clippy::missing_safety_doc,
    clippy::arithmetic_side_effects
)]

use super::*;

use std::mem;
use std::ops::Not;
use std::ptr;

macro_rules! container_of_mut {
    ($ptr: expr, $ty: path, $field: tt) => {{
        use ::core::primitive::u8;
        let ptr: *mut _ = $ptr;
        ptr.cast::<u8>().sub(offset_of!($ty, $field)).cast::<$ty>()
    }};
}

#[inline(always)]
unsafe fn set_errno(errno: i32) {
    __errno_location().write(errno);
}

mod compat {
    use super::{_compat_ibv_port_attr, ibv_context, ibv_mr, ibv_pd};
    use super::{c_int, c_uint, c_void};

    extern "C" {
        pub fn ibv_query_port(
            context: *mut ibv_context,
            port_num: u8,
            port_attr: *mut _compat_ibv_port_attr,
        ) -> c_int;

        pub fn ibv_reg_mr(
            pd: *mut ibv_pd,
            addr: *mut c_void,
            length: usize,
            access: c_uint,
        ) -> *mut ibv_mr;
    }
}

#[inline]
pub fn ibv_cq_ex_to_cq(cq: *mut ibv_cq_ex) -> *mut ibv_cq {
    cq.cast()
}

#[inline]
unsafe fn verbs_get_ctx(ctx: *mut ibv_context) -> *mut verbs_context {
    if (*ctx).abi_compat != usize::MAX as *mut c_void {
        return ptr::null_mut();
    }
    container_of_mut!(ctx, verbs_context, context)
}

macro_rules! verbs_get_ctx_op {
    ($ctx: expr, $op: tt) => {{
        let vctx: *mut verbs_context = verbs_get_ctx($ctx);
        if vctx.is_null()
            || (*vctx).sz < ::core::mem::size_of::<verbs_context>() - offset_of!(verbs_context, $op)
            || (*vctx).$op.is_none()
        {
            ptr::null_mut()
        } else {
            vctx
        }
    }};
}

#[inline]
pub unsafe fn ibv_create_cq_ex(
    context: *mut ibv_context,
    cq_attr: *mut ibv_cq_init_attr_ex,
) -> *mut ibv_cq_ex {
    let vctx = verbs_get_ctx_op!(context, create_cq_ex);
    if vctx.is_null() {
        set_errno(EOPNOTSUPP);
        return ptr::null_mut();
    }
    let op = (*vctx).create_cq_ex.unwrap_unchecked();
    (op)(context, cq_attr)
}

#[inline]
pub unsafe fn ibv_query_gid_ex(
    context: *mut ibv_context,
    port_num: u32,
    gid_index: u32,
    entry: *mut ibv_gid_entry,
    flags: u32,
) -> c_int {
    _ibv_query_gid_ex(
        context,
        port_num,
        gid_index,
        entry,
        flags,
        mem::size_of::<ibv_gid_entry>(),
    )
}

#[inline]
pub unsafe fn ibv_query_port(
    context: *mut ibv_context,
    port_num: u8,
    port_attr: *mut ibv_port_attr,
) -> c_int {
    let vctx: *mut verbs_context = verbs_get_ctx_op!(context, query_port);
    if vctx.is_null() {
        ptr::write_bytes(port_attr, 0, 1);
        return compat::ibv_query_port(context, port_num, port_attr.cast());
    }
    let op = (*vctx).query_port.unwrap_unchecked();
    (op)(
        context,
        port_num,
        port_attr,
        mem::size_of::<ibv_port_attr>(),
    )
}

#[inline]
pub unsafe fn ibv_query_device_ex(
    context: *mut ibv_context,
    input: *const ibv_query_device_ex_input,
    attr: *mut ibv_device_attr_ex,
) -> c_int {
    if input.is_null().not() && (*input).comp_mask != 0 {
        return EINVAL;
    }

    let legacy = || {
        ptr::write_bytes(attr, 0, 1);
        ibv_query_device(context, ptr::addr_of_mut!((*attr).orig_attr))
    };

    let vctx: *mut verbs_context = verbs_get_ctx_op!(context, query_device_ex);
    if vctx.is_null() {
        return legacy();
    }

    let op = (*vctx).query_device_ex.unwrap_unchecked();
    let ret = (op)(context, input, attr, mem::size_of::<ibv_device_attr_ex>());
    if ret == EOPNOTSUPP || ret == ENOSYS {
        return legacy();
    }
    ret
}

#[inline]
pub unsafe fn ibv_create_qp_ex(
    context: *mut ibv_context,
    qp_attr: *mut ibv_qp_init_attr_ex,
) -> *mut ibv_qp {
    let mask = (*qp_attr).comp_mask;
    if mask == IBV_QP_INIT_ATTR_PD {
        let pd = (*qp_attr).pd;
        return ibv_create_qp(pd, qp_attr.cast());
    }
    let vctx = verbs_get_ctx_op!(context, create_qp_ex);
    if vctx.is_null() {
        set_errno(EOPNOTSUPP);
        return ptr::null_mut();
    }
    let op = (*vctx).create_qp_ex.unwrap_unchecked();
    (op)(context, qp_attr)
}

#[inline]
pub unsafe fn ibv_req_notify_cq(cq: *mut ibv_cq, solicited_only: c_int) -> c_int {
    let ctx: *mut ibv_context = (*cq).context;
    let op = (*ctx).ops.req_notify_cq.unwrap_unchecked();
    (op)(cq, solicited_only)
}

#[inline]
pub unsafe fn ibv_poll_cq(cq: *mut ibv_cq, num_entries: c_int, wc: *mut ibv_wc) -> c_int {
    let ctx: *mut ibv_context = (*cq).context;
    let op = (*ctx).ops.poll_cq.unwrap_unchecked();
    (op)(cq, num_entries, wc)
}

#[inline]
pub unsafe fn ibv_alloc_mw(pd: *mut ibv_pd, mw_type: ibv_mw_type) -> *mut ibv_mw {
    let ctx: *mut ibv_context = (*pd).context;
    let op = (*ctx).ops.alloc_mw;
    if op.is_none() {
        set_errno(EOPNOTSUPP);
        return ptr::null_mut();
    }
    let op = op.unwrap_unchecked();
    (op)(pd, mw_type)
}

#[inline]
pub unsafe fn ibv_dealloc_mw(mw: *mut ibv_mw) -> c_int {
    let ctx: *mut ibv_context = (*mw).context;
    let op = (*ctx).ops.dealloc_mw.unwrap_unchecked();
    (op)(mw)
}

#[inline]
pub unsafe fn ibv_bind_mw(qp: *mut ibv_qp, mw: *mut ibv_mw, mw_bind: *mut ibv_mw_bind) -> c_int {
    {
        let mw = &*mw;
        if mw.type_ != IBV_MW_TYPE_1 {
            return EINVAL;
        }

        let bind_info = &((*mw_bind).bind_info);
        if bind_info.mr.is_null() && (bind_info.addr != 0 || bind_info.length != 0) {
            return EINVAL;
        }
        if bind_info.mr.is_null().not() && (mw.pd != (*bind_info.mr).pd) {
            return EPERM;
        }
    }

    {
        let ctx: *mut ibv_context = (*mw).context;
        let op = (*ctx).ops.bind_mw.unwrap_unchecked();
        (op)(qp, mw, mw_bind)
    }
}

#[inline]
pub unsafe fn ibv_alloc_dm(context: *mut ibv_context, attr: *mut ibv_alloc_dm_attr) -> *mut ibv_dm {
    let vctx: *mut verbs_context = verbs_get_ctx_op!(context, alloc_dm);

    if vctx.is_null() {
        set_errno(EOPNOTSUPP);
        return ptr::null_mut();
    }

    let op = (*vctx).alloc_dm.unwrap_unchecked();
    (op)(context, attr)
}

#[inline]
pub unsafe fn ibv_free_dm(dm: *mut ibv_dm) -> c_int {
    let vctx: *mut verbs_context = verbs_get_ctx_op!((*dm).context, free_dm);

    if vctx.is_null() {
        return EOPNOTSUPP;
    }

    let op = (*vctx).free_dm.unwrap_unchecked();
    (op)(dm)
}

#[inline]
pub unsafe fn ibv_post_send(
    qp: *mut ibv_qp,
    wr: *mut ibv_send_wr,
    bad_wr: *mut *mut ibv_send_wr,
) -> c_int {
    let ctx: *mut ibv_context = (*qp).context;
    let op = (*ctx).ops.post_send.unwrap_unchecked();
    (op)(qp, wr, bad_wr)
}

#[inline]
pub unsafe fn ibv_post_recv(
    qp: *mut ibv_qp,
    wr: *mut ibv_recv_wr,
    bad_wr: *mut *mut ibv_recv_wr,
) -> c_int {
    let ctx: *mut ibv_context = (*qp).context;
    let op = (*ctx).ops.post_recv.unwrap_unchecked();
    (op)(qp, wr, bad_wr)
}

#[inline]
pub unsafe fn ibv_reg_mr(
    pd: *mut ibv_pd,
    addr: *mut c_void,
    length: usize,
    access: c_uint,
) -> *mut ibv_mr {
    if access & _RS_IBV_ACCESS_OPTIONAL_RANGE == 0 {
        return compat::ibv_reg_mr(pd, addr, length, access);
    }
    ibv_reg_mr_iova2(pd, addr, length, addr as usize as u64, access)
}

#[inline]
pub unsafe fn ibv_wr_atomic_cmp_swp(
    qp: *mut ibv_qp_ex,
    rkey: u32,
    remote_addr: u64,
    compare: u64,
    swap: u64,
) {
    let op = (*qp).wr_atomic_cmp_swp.unwrap_unchecked();
    (op)(qp, rkey, remote_addr, compare, swap);
}

#[inline]
pub unsafe fn ibv_wr_atomic_fetch_add(qp: *mut ibv_qp_ex, rkey: u32, remote_addr: u64, add: u64) {
    let op = (*qp).wr_atomic_fetch_add.unwrap_unchecked();
    (op)(qp, rkey, remote_addr, add);
}

#[inline]
pub unsafe fn ibv_wr_bind_mw(
    qp: *mut ibv_qp_ex,
    mw: *mut ibv_mw,
    rkey: u32,
    bind_info: *const ibv_mw_bind_info,
) {
    let op = (*qp).wr_bind_mw.unwrap_unchecked();
    (op)(qp, mw, rkey, bind_info);
}

#[inline]
pub unsafe fn ibv_wr_local_inv(qp: *mut ibv_qp_ex, invalidate_rkey: u32) {
    let op = (*qp).wr_local_inv.unwrap_unchecked();
    (op)(qp, invalidate_rkey);
}

#[inline]
pub unsafe fn ibv_wr_rdma_read(qp: *mut ibv_qp_ex, rkey: u32, remote_addr: u64) {
    let op = (*qp).wr_rdma_read.unwrap_unchecked();
    (op)(qp, rkey, remote_addr);
}

#[inline]
pub unsafe fn ibv_wr_rdma_write(qp: *mut ibv_qp_ex, rkey: u32, remote_addr: u64) {
    let op = (*qp).wr_rdma_write.unwrap_unchecked();
    (op)(qp, rkey, remote_addr);
}

#[inline]
pub unsafe fn ibv_wr_rdma_write_imm(
    qp: *mut ibv_qp_ex,
    rkey: u32,
    remote_addr: u64,
    imm_data: __be32,
) {
    let op = (*qp).wr_rdma_write_imm.unwrap_unchecked();
    (op)(qp, rkey, remote_addr, imm_data);
}

#[inline]
pub unsafe fn ibv_wr_send(qp: *mut ibv_qp_ex) {
    let op = (*qp).wr_send.unwrap_unchecked();
    (op)(qp);
}

#[inline]
pub unsafe fn ibv_wr_send_imm(qp: *mut ibv_qp_ex, imm_data: __be32) {
    let op = (*qp).wr_send_imm.unwrap_unchecked();
    (op)(qp, imm_data);
}

#[inline]
pub unsafe fn ibv_wr_send_inv(qp: *mut ibv_qp_ex, invalidate_rkey: u32) {
    let op = (*qp).wr_send_inv.unwrap_unchecked();
    (op)(qp, invalidate_rkey);
}

#[inline]
pub unsafe fn ibv_wr_send_tso(qp: *mut ibv_qp_ex, hdr: *mut c_void, hdr_sz: u16, mss: u16) {
    let op = (*qp).wr_send_tso.unwrap_unchecked();
    (op)(qp, hdr, hdr_sz, mss);
}

#[inline]
pub unsafe fn ibv_wr_set_ud_addr(
    qp: *mut ibv_qp_ex,
    ah: *mut ibv_ah,
    remote_qpn: u32,
    remote_qkey: u32,
) {
    let op = (*qp).wr_set_ud_addr.unwrap_unchecked();
    (op)(qp, ah, remote_qpn, remote_qkey);
}

#[inline]
pub unsafe fn ibv_wr_set_xrc_srqn(qp: *mut ibv_qp_ex, remote_srqn: u32) {
    let op = (*qp).wr_set_xrc_srqn.unwrap_unchecked();
    (op)(qp, remote_srqn);
}

#[inline]
pub unsafe fn ibv_wr_set_inline_data(qp: *mut ibv_qp_ex, addr: *mut c_void, length: usize) {
    let op = (*qp).wr_set_inline_data.unwrap_unchecked();
    (op)(qp, addr, length);
}

#[inline]
pub unsafe fn ibv_wr_set_inline_data_list(
    qp: *mut ibv_qp_ex,
    num_buf: usize,
    buf_list: *const ibv_data_buf,
) {
    let op = (*qp).wr_set_inline_data_list.unwrap_unchecked();
    (op)(qp, num_buf, buf_list);
}

#[inline]
pub unsafe fn ibv_wr_set_sge(qp: *mut ibv_qp_ex, lkey: u32, addr: u64, length: u32) {
    let op = (*qp).wr_set_sge.unwrap_unchecked();
    (op)(qp, lkey, addr, length);
}

#[inline]
pub unsafe fn ibv_wr_set_sge_list(qp: *mut ibv_qp_ex, num_sge: usize, sg_list: *const ibv_sge) {
    let op = (*qp).wr_set_sge_list.unwrap_unchecked();
    (op)(qp, num_sge, sg_list);
}

#[inline]
pub unsafe fn ibv_wr_start(qp: *mut ibv_qp_ex) {
    let op = (*qp).wr_start.unwrap_unchecked();
    (op)(qp);
}

#[inline]
pub unsafe fn ibv_wr_complete(qp: *mut ibv_qp_ex) -> c_int {
    let op = (*qp).wr_complete.unwrap_unchecked();
    (op)(qp)
}

#[inline]
pub unsafe fn ibv_wr_abort(qp: *mut ibv_qp_ex) {
    let op = (*qp).wr_abort.unwrap_unchecked();
    (op)(qp);
}

#[inline]
pub unsafe fn ibv_create_srq_ex(
    context: *mut ibv_context,
    srq_init_attr_ex: *mut ibv_srq_init_attr_ex,
) -> *mut ibv_srq {
    let legacy = {
        let mask: u32 = (*srq_init_attr_ex).comp_mask;

        let has = |m: u32| (mask & m) != 0;
        let contained_by = |m: u32| (mask & !m) == 0;

        contained_by(IBV_SRQ_INIT_ATTR_PD | IBV_SRQ_INIT_ATTR_TYPE)
            && has(IBV_SRQ_INIT_ATTR_PD)
            && (has(IBV_SRQ_INIT_ATTR_TYPE).not()
                || ((*srq_init_attr_ex).srq_type == IBV_SRQT_BASIC))
    };

    if legacy {
        return ibv_create_srq((*srq_init_attr_ex).pd, srq_init_attr_ex.cast());
    }

    let vctx = verbs_get_ctx_op!(context, create_srq_ex);
    if vctx.is_null() {
        set_errno(EOPNOTSUPP);
        return ptr::null_mut();
    }

    let op = (*vctx).create_srq_ex.unwrap_unchecked();
    (op)(context, srq_init_attr_ex)
}
