use crate::buf::Buf;

use rdma::qp::QueuePair;
use rdma::wc::{self, WorkCompletion, WorkCompletionError};
use rdma::wr;

use std::future::Future;
use std::mem::{self, ManuallyDrop};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::{io, slice};

use anyhow::Result;
use numeric_cast::NumericCast;
use parking_lot::Mutex;

/// # Safety
/// TODO
unsafe trait Operation: Send + Sync + Unpin {
    fn submit(&mut self, qp: &QueuePair, id: u64) -> bool;
    fn complete(&mut self, wc: &WorkCompletion);
}

struct Work<T> {
    state: Arc<Mutex<State<T>>>,
}

struct State<T> {
    step: Step,
    waker: Option<Waker>,
    qp: QueuePair,
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
    pub fn new(qp: QueuePair, op: T) -> Self {
        Work {
            state: Arc::new(Mutex::new(State {
                step: Step::Pending,
                waker: None,
                qp,
                op: ManuallyDrop::new(op),
            })),
        }
    }

    pub unsafe fn complete(wc: &WorkCompletion) {
        let state_ptr = wc.wr_id() as usize as *mut Mutex<State<T>>;
        let state = Arc::from_raw(state_ptr);
        {
            let mut guard = state.lock();
            let state = &mut *guard;
            assert_eq!(state.step, Step::Running);
            state.op.complete(wc);
            state.step = Step::Completed;
            if let Some(ref waker) = state.waker {
                waker.wake_by_ref();
            }
        }
    }
}

pub unsafe fn complete(wc: &WorkCompletion) {
    match wc.opcode() {
        wc::Opcode::Send => Work::<OpSend>::complete(wc),
        wc::Opcode::Recv => Work::<OpRecv>::complete(wc),
        _ => unimplemented!(),
    }
}

impl<T: Operation> Future for Work<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut guard = self.state.lock();
        let state = &mut *guard;
        match mem::replace(&mut state.step, Step::Poisoned) {
            Step::Pending => {
                let state_ptr = Arc::into_raw(Arc::clone(&self.state));

                let id: u64 = (state_ptr as usize).numeric_cast();

                if state.op.submit(&state.qp, id) {
                    state.step = Step::Running;
                    state.waker = Some(cx.waker().clone());
                    Poll::Pending
                } else {
                    // SAFETY: state refcount
                    unsafe { Arc::decrement_strong_count(state_ptr) };

                    state.step = Step::Invalid;
                    // SAFETY: managed state machine
                    let op = unsafe { ManuallyDrop::take(&mut state.op) };
                    Poll::Ready(op)
                }
            }
            Step::Running => {
                state.step = Step::Running;
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
            Step::Completed => {
                state.step = Step::Invalid;
                // SAFETY: managed state machine
                let op = unsafe { ManuallyDrop::take(&mut state.op) };
                Poll::Ready(op)
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

struct OpSend {
    buf: Buf,
    nbytes: usize,
    res: io::Result<()>,
    status: u32,
}

/// SAFETY: operation type
unsafe impl Operation for OpSend {
    fn submit(&mut self, qp: &QueuePair, id: u64) -> bool {
        let cq = qp.send_cq().expect("the qp can not post send");

        self.res = cq.req_notify_all();
        if self.res.is_err() {
            return false;
        }

        let send_sge = wr::Sge {
            addr: self.buf.mr.addr_u64(),
            length: self.nbytes.numeric_cast(),
            lkey: self.buf.mr.lkey(),
        };

        let mut send_wr = wr::SendRequest::zeroed();
        send_wr
            .id(id)
            .sg_list(slice::from_ref(&send_sge))
            .opcode(wr::Opcode::Send)
            .send_flags(wr::SendFlags::SIGNALED);

        // SAFETY: managed state machine
        self.res = unsafe { qp.post_send(&send_wr) };
        self.res.is_ok()
    }

    fn complete(&mut self, wc: &WorkCompletion) {
        self.status = wc.status();
    }
}

struct OpRecv {
    buf: Buf,
    res: io::Result<()>,
    status: u32,
    byte_len: u32,
}

/// SAFETY: operation type
unsafe impl Operation for OpRecv {
    fn submit(&mut self, qp: &QueuePair, id: u64) -> bool {
        let cq = qp.recv_cq().expect("the qp can not post recv");

        self.res = cq.req_notify_all();
        if self.res.is_err() {
            return false;
        }

        let recv_sge = wr::Sge {
            addr: self.buf.mr.addr_u64(),
            length: self.buf.mr.length().numeric_cast(),
            lkey: self.buf.mr.lkey(),
        };

        let mut recv_wr = wr::RecvRequest::zeroed();
        recv_wr.id(id).sg_list(slice::from_ref(&recv_sge));

        // SAFETY: managed state machine
        self.res = unsafe { qp.post_recv(&recv_wr) };
        self.res.is_ok()
    }

    fn complete(&mut self, wc: &WorkCompletion) {
        self.status = wc.status();
        self.byte_len = wc.byte_len();
    }
}

pub async fn send(qp: QueuePair, buf: Buf, nbytes: usize) -> (Result<()>, Buf) {
    let nbytes = nbytes.min(buf.mr.length());

    let work = Work::new(
        qp,
        OpSend {
            buf,
            nbytes,
            res: Ok(()),
            status: u32::MAX,
        },
    );

    let op = work.await;

    if let Err(err) = op.res {
        return (Err(err.into()), op.buf);
    }
    if let Err(err) = WorkCompletionError::result(op.status) {
        return (Err(err.into()), op.buf);
    }
    (Ok(()), op.buf)
}

pub async fn recv(qp: QueuePair, buf: Buf) -> (Result<usize>, Buf) {
    let work = Work::new(
        qp,
        OpRecv {
            buf,
            res: Ok(()),
            status: u32::MAX,
            byte_len: 0,
        },
    );

    let op = work.await;

    if let Err(err) = op.res {
        return (Err(err.into()), op.buf);
    }
    if let Err(err) = WorkCompletionError::result(op.status) {
        return (Err(err.into()), op.buf);
    }
    (Ok(op.byte_len.numeric_cast()), op.buf)
}
