
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use std::future::Future;

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
        let (tx, rx) = kanal::bounded_async(cap);
        let overflowed = Arc::new(AtomicBool::new(false));
        (Sender{tx, overflowed: overflowed.clone()}, Receiver{rx, overflowed})
    }

    fn name() -> &'static str {
        "kanal"
    }
}

pub struct Sender<T>{
    tx: kanal::AsyncSender<T>,
    overflowed: Arc<AtomicBool>,
}

impl<T> Clone for Sender<T> 
where 
    T: Clone,
{
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone(), overflowed: self.overflowed.clone() }
    }
}

impl<T> SenderOp<T> for Sender<T> 
where 
    T: Clone,
{
    fn try_send(&mut self, msg: T) -> Result<(), TrySendError<T>> {
        let r = self.tx.try_send(msg.clone());
        match r {
            Ok(r) => {
                if r {
                    return Ok(())
                } 
            },
            Err(_e) => { },
        }
        self.overflowed.store(true, Ordering::Release);
        Err(TrySendError(msg))
    }
}


pub struct Receiver<T>{
    rx: kanal::AsyncReceiver<T>,
    overflowed: Arc<AtomicBool>,
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
        let overflowed = self.overflowed.fetch_and(false, Ordering::Acquire);
        if overflowed {
            return Err(TryRecvError::Overflowed)
        }

        let r = self.rx.try_recv();
        match r {
            Ok(r) => {
                match r {
                    Some(v) => Ok(v),
                    None => Err(TryRecvError::Empty),
                }
            },
            Err(_e) => Err(TryRecvError::Empty),
        }
    }
}

impl<T> AsyncRecvOp<T> for Receiver<T> 
where 
    T: Clone,
{ 
    type Fut<'a> = impl Future<Output = Result<T, RecvError> > + 'a where Self: 'a;

    fn async_recv(&mut self) -> Self::Fut<'_> { 
        async move {
            let overflowed = self.overflowed.fetch_and(false, Ordering::Acquire);
            if overflowed {
                return Err(RecvError)
            }

            let r = self.rx.recv().await;
            match r {
                Ok(v) => Ok(v),
                Err(_e) => Err(RecvError),
            }
        }
    }
}

