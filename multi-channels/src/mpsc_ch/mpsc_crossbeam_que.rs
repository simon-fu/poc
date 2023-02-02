
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{task::Poll, pin::Pin};

use std::future::Future;

use crossbeam::queue::ArrayQueue;
use futures::task::AtomicWaker;

use super::mpsc_defs::{
    error::{
        TrySendError,
        TryRecvError,
        RecvError,
    }, 
    SenderOp, 
    ReceiverOp,
    TryRecvOp,
    AsyncRecvOp, MpscOp,
};


pub struct Mpsc;

impl<T> MpscOp<T> for Mpsc
where
    T: Clone,
{
    type Sender = Sender<T>;

    type Receiver = Receiver<T>;

    fn channel(cap: usize) -> (Self::Sender, Self::Receiver) { 
        let shared = Arc::new(Shared {
            que: ArrayQueue::new(cap),
            overflowed: AtomicBool::new(false),
            waker: AtomicWaker::new(),
        });

        let tx = Sender {
            shared: shared.clone(),
        };

        let rx = Receiver {
            shared,
        };

        (tx, rx)
    }

    fn name() -> &'static str {
        "crossbeam_que_and_awaker"
    }
}

// type Que<T> = Arc<ArrayQueue<T>>;

struct Shared<T> {
    que: ArrayQueue<T>,
    overflowed: AtomicBool,
    waker: AtomicWaker,
}


pub struct Sender<T>{
    shared: Arc<Shared<T>>,
}

impl<T> Clone for Sender<T> 
where 
    T: Clone,
{
    fn clone(&self) -> Self {
        Self { shared: self.shared.clone() }
    }
}

impl<T> SenderOp<T> for Sender<T> 
where 
    T: Clone,
{
    fn try_send(&mut self, msg: T) -> Result<(), TrySendError<T>> { 
        let r = self.shared.que.push(msg);
        self.shared.waker.wake();
        match r {
            Ok(_r) => Ok(()),
            Err(v) => { 
                self.shared.overflowed.store(true, Ordering::Release);
                Err(TrySendError(v))
            },
        }
    }
}


pub struct Receiver<T> {
    shared: Arc<Shared<T>>,
}

impl<T> Receiver<T> 
where 
    T: Clone,
{
    fn check_recv(&mut self) -> Option<Result<T, RecvError>> {
        let r = self.try_recv();
        match r {
            Ok(v) => Some(Ok(v)),
            Err(e) => {
                match e {
                    TryRecvError::Overflowed => Some(Err(RecvError)),
                    TryRecvError::Empty => None,
                }
            },
        }
    }
}

impl<T> ReceiverOp<T> for Receiver<T> 
where 
    T: Clone,
{
}


impl<T> TryRecvOp<T> for Receiver<T> 
where 
    T: Clone,
{
    fn try_recv(&mut self) -> Result<T, TryRecvError> { 
        let overflowed = self.shared.overflowed.fetch_and(false, Ordering::Acquire);
        if overflowed {
            return Err(TryRecvError::Overflowed)
        }
        
        let r = self.shared.que.pop();
        match r {
            Some(v) => Ok(v),
            None => Err(TryRecvError::Empty),
        }
    }
}

impl<T> AsyncRecvOp<T> for Receiver<T> 
where 
    T: Clone,
{ 
    type Fut<'a> = RecvFut<'a, T> where T: 'a;

    fn async_recv(&mut self) -> Self::Fut<'_> { 
        let fut = RecvFut(self);
        fut
    }
}

pub struct RecvFut<'a, T>(&'a mut Receiver<T>);

impl<'a, T> Future for RecvFut<'a, T> 
where 
    T: Clone,
{
    type Output = Result<T, RecvError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> { 
        
        let r = self.0.check_recv();
        if let Some(r) = r {
            return Poll::Ready(r)
        }

        self.0.shared.waker.register(cx.waker());

        let r = self.0.check_recv();
        if let Some(r) = r {
            return Poll::Ready(r)
        }

        Poll::Pending
    }
}

