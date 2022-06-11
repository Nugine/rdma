use crate::sg_list::SgList;
use crate::{GatherList, ScatterList};
use crate::{RemoteReadAccess, RemoteWriteAccess};

use rdma::qp::QueuePair;
use rdma::wc::{WorkCompletion, WorkCompletionError};
use rdma::wr::{self, RecvRequest, SendRequest, Sge};

use std::future::Future;
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::{io, slice};

use anyhow::Result;
use numeric_cast::NumericCast;
use parking_lot::Mutex;
use scopeguard::ScopeGuard;

/// # Safety
/// TODO
unsafe trait Operation: Send {
    type Output;
    fn submit(&mut self, qp: &QueuePair, id: u64) -> io::Result<()>;
    fn complete(&mut self, wc: &WorkCompletion);
    fn output(self, result: io::Result<u32>) -> Self::Output;
}

struct Work<T> {
    inner: Arc<WorkInner<T>>,
}

impl<T> Unpin for Work<T> {}

#[repr(C)]
struct WorkInner<T> {
    complete: unsafe fn(wc: *const WorkCompletion),
    state: Mutex<State<T>>,
}

struct State<T> {
    step: Step,
    waker: Option<Waker>,
    qp: QueuePair,
    status: u32,
    op: ManuallyDrop<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Pending,
    Running,
    Completed,
    Invalid,
    Poisoned,
}

impl<T: Operation> Work<T> {
    fn new(qp: QueuePair, op: T) -> Self {
        Self {
            inner: Arc::new(WorkInner {
                complete: Self::complete,
                state: Mutex::new(State {
                    step: Step::Pending,
                    waker: None,
                    qp,
                    status: u32::MAX,
                    op: ManuallyDrop::new(op),
                }),
            }),
        }
    }

    unsafe fn complete(wc: *const WorkCompletion) {
        let wc = &*wc;
        let inner: Arc<WorkInner<T>> = Arc::from_raw(wc.wr_id() as usize as *mut _);
        {
            let mut guard: _ = inner.state.lock();
            let state = &mut *guard;
            assert_eq!(state.step, Step::Running);
            state.status = wc.status();
            state.op.complete(wc);
            state.step = Step::Completed;
            if let Some(ref waker) = state.waker {
                waker.wake_by_ref();
            }
        }
    }
}

pub unsafe fn complete(wc: &WorkCompletion) {
    let inner: *mut WorkInner<()> = wc.wr_id() as usize as *mut _;
    ((*inner).complete)(wc)
}

impl<T: Operation> Future for Work<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut guard = self.inner.state.lock();
        let state = &mut *guard;
        match mem::replace(&mut state.step, Step::Poisoned) {
            Step::Pending => {
                let inner_ptr: *const WorkInner<T> = Arc::into_raw(Arc::clone(&self.inner));
                let arc_guard: _ =
                    scopeguard::guard((), |()| unsafe { Arc::decrement_strong_count(inner_ptr) });

                let id: u64 = (inner_ptr as usize).numeric_cast();

                let res = state.op.submit(&state.qp, id);
                match res {
                    Ok(()) => {
                        ScopeGuard::into_inner(arc_guard);

                        state.step = Step::Running;
                        state.waker = Some(cx.waker().clone());
                        Poll::Pending
                    }
                    Err(err) => {
                        drop(arc_guard);

                        state.step = Step::Invalid;
                        let op = unsafe { ManuallyDrop::take(&mut state.op) };
                        Poll::Ready(op.output(Err(err)))
                    }
                }
            }
            Step::Running => {
                state.step = Step::Running;
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
            Step::Completed => {
                state.step = Step::Invalid;
                let op = unsafe { ManuallyDrop::take(&mut state.op) };
                Poll::Ready(op.output(Ok(state.status)))
            }
            Step::Invalid => panic!("the future is completed or failed"),
            Step::Poisoned => panic!("the future is poisoned"),
        }
    }
}

impl<T> Drop for State<T> {
    fn drop(&mut self) {
        match self.step {
            Step::Pending | Step::Completed | Step::Poisoned => {
                // SAFETY: managed state machine
                unsafe { ManuallyDrop::drop(&mut self.op) };
            }
            Step::Running => panic!("state should not be dropped when step is running"),
            Step::Invalid => {}
        }
    }
}

unsafe fn convert_sglist<R>(sg_list: SgList<'_>, f: impl FnOnce(&[Sge]) -> R) -> R {
    const N: usize = 4;
    let mut arr_sg_list;
    let mut vec_sg_list;
    let sg_list: &[Sge] = {
        let len = sg_list.length();
        if len <= N {
            arr_sg_list = MaybeUninit::<[Sge; N]>::uninit();
            let ptr = arr_sg_list.as_mut_ptr().cast();
            sg_list.fill(ptr);
            slice::from_raw_parts(ptr, len)
        } else {
            vec_sg_list = Vec::with_capacity(len);
            sg_list.fill(vec_sg_list.as_mut_ptr());
            vec_sg_list.set_len(len);
            vec_sg_list.as_slice()
        }
    };
    f(sg_list)
}

unsafe fn submit_single_send(
    qp: &QueuePair,
    id: u64,
    sg_list: SgList<'_>,
    f: &mut dyn FnMut(&mut SendRequest),
) -> io::Result<()> {
    let cq = qp.send_cq().expect("the qp can not post send");

    cq.req_notify_all()?;

    convert_sglist(sg_list, |sg_list| {
        let mut send_wr = SendRequest::zeroed();
        send_wr
            .id(id)
            .sg_list(sg_list)
            .send_flags(wr::SendFlags::SIGNALED);
        f(&mut send_wr);

        qp.post_send(&send_wr)
    })
}

unsafe fn submit_single_recv(qp: &QueuePair, id: u64, sg_list: SgList<'_>) -> io::Result<()> {
    let cq = qp.recv_cq().expect("the qp can not post recv");

    cq.req_notify_all()?;

    convert_sglist(sg_list, |sg_list| {
        let mut recv_wr = RecvRequest::zeroed();
        recv_wr.id(id).sg_list(sg_list);

        qp.post_recv(&recv_wr)
    })
}

fn return_value<F, G>(
    result: io::Result<u32>,
    f: impl FnOnce() -> F,
    g: impl FnOnce() -> G,
) -> (Result<F>, G) {
    match result {
        Ok(status) => match WorkCompletionError::result(status) {
            Ok(()) => (Ok(f()), g()),
            Err(err) => (Err(err.into()), g()),
        },
        Err(err) => (Err(err.into()), g()),
    }
}

struct OpSend<T> {
    slist: T,
    imm: Option<u32>,
}

/// SAFETY: operation type
unsafe impl<T> Operation for OpSend<T>
where
    T: ScatterList + Send,
{
    type Output = (Result<()>, T);

    fn submit(&mut self, qp: &QueuePair, id: u64) -> io::Result<()> {
        unsafe {
            let sg_list = SgList::from_slist(&self.slist);
            submit_single_send(qp, id, sg_list, &mut |send_wr| {
                match self.imm {
                    None => send_wr.opcode(wr::Opcode::Send),
                    Some(imm) => send_wr.opcode(wr::Opcode::SendWithImm).imm_data(imm),
                };
            })
        }
    }

    fn complete(&mut self, _: &WorkCompletion) {}

    fn output(self, result: io::Result<u32>) -> Self::Output {
        return_value(result, || (), || self.slist)
    }
}

struct OpRecv<T> {
    glist: T,
    byte_len: u32,
    imm_data: Option<u32>,
}

/// SAFETY: operation type
unsafe impl<T> Operation for OpRecv<T>
where
    T: GatherList + Send,
{
    type Output = (Result<(usize, Option<u32>)>, T);

    fn submit(&mut self, qp: &QueuePair, id: u64) -> io::Result<()> {
        unsafe {
            let sg_list = SgList::from_glist(&self.glist);
            submit_single_recv(qp, id, sg_list)
        }
    }

    fn complete(&mut self, wc: &WorkCompletion) {
        self.byte_len = wc.byte_len();
        self.imm_data = wc.imm_data();
    }

    fn output(self, result: io::Result<u32>) -> Self::Output {
        return_value(
            result,
            || (self.byte_len.numeric_cast(), self.imm_data),
            || self.glist,
        )
    }
}

pub fn send<T>(qp: QueuePair, slist: T, imm: Option<u32>) -> impl Future<Output = (Result<()>, T)>
where
    T: ScatterList + Send,
{
    Work::new(qp, OpSend { slist, imm })
}

pub fn recv<T>(qp: QueuePair, glist: T) -> impl Future<Output = (Result<(usize, Option<u32>)>, T)>
where
    T: GatherList + Send,
{
    Work::new(
        qp,
        OpRecv {
            glist,
            byte_len: 0,
            imm_data: None,
        },
    )
}

pub struct OpWrite<T, U> {
    slist: T,
    remote: U,
}

/// SAFETY: operation type
unsafe impl<T, U> Operation for OpWrite<T, U>
where
    T: ScatterList + Send,
    U: RemoteWriteAccess + Send,
{
    type Output = (Result<()>, (T, U));

    fn submit(&mut self, qp: &QueuePair, id: u64) -> io::Result<()> {
        unsafe {
            let sg_list = SgList::from_slist(&self.slist);
            submit_single_send(qp, id, sg_list, &mut |send_wr| {
                send_wr
                    .opcode(wr::Opcode::Write)
                    .rdma_remote_addr(self.remote.addr_u64())
                    .rdma_rkey(self.remote.rkey());
            })
        }
    }

    fn complete(&mut self, _: &WorkCompletion) {}

    fn output(self, result: io::Result<u32>) -> Self::Output {
        return_value(result, || (), || (self.slist, self.remote))
    }
}

pub struct OpRead<T, U> {
    glist: T,
    remote: U,
    byte_len: u32,
}

/// SAFETY: operation type
unsafe impl<T, U> Operation for OpRead<T, U>
where
    T: GatherList + Send,
    U: RemoteReadAccess + Send,
{
    type Output = (Result<usize>, (T, U));

    fn submit(&mut self, qp: &QueuePair, id: u64) -> io::Result<()> {
        unsafe {
            let sg_list = SgList::from_glist(&self.glist);
            submit_single_send(qp, id, sg_list, &mut |send_wr| {
                send_wr
                    .opcode(wr::Opcode::Read)
                    .rdma_remote_addr(self.remote.addr_u64())
                    .rdma_rkey(self.remote.rkey());
            })
        }
    }

    fn complete(&mut self, wc: &WorkCompletion) {
        self.byte_len = wc.byte_len();
    }

    fn output(self, result: io::Result<u32>) -> Self::Output {
        return_value(
            result,
            || self.byte_len.numeric_cast(),
            || (self.glist, self.remote),
        )
    }
}

pub fn write<T, U>(qp: QueuePair, slist: T, remote: U) -> impl Future<Output = (Result<()>, (T, U))>
where
    T: ScatterList + Send,
    U: RemoteWriteAccess + Send,
{
    Work::new(qp, OpWrite { slist, remote })
}

pub fn read<T, U>(
    qp: QueuePair,
    glist: T,
    remote: U,
) -> impl Future<Output = (Result<usize>, (T, U))>
where
    T: GatherList + Send,
    U: RemoteReadAccess + Send,
{
    Work::new(
        qp,
        OpRead {
            glist,
            remote,
            byte_len: 0,
        },
    )
}
