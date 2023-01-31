
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
    RecvOp, MpscOp,
};


pub struct Mpsc;

impl<T> MpscOp<T> for Mpsc
where
    T: Clone,
{
    type Sender = Sender<T>;

    type Receiver = Receiver<T>;

    fn channel(cap: usize) -> (Self::Sender, Self::Receiver) {
        let (tx, rx) = async_channel::bounded(cap);
        let overflowed = Arc::new(AtomicBool::new(false));
        (Sender{tx, overflowed: overflowed.clone()}, Receiver{rx, overflowed})
    }

    fn name() -> &'static str {
        "async_channel"
    }
}

pub struct Sender<T>{
    tx: async_channel::Sender<T>,
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
        let r = self.tx.try_send(msg);
        match r {
            Ok(_r) => Ok(()),
            Err(e) => { 
                self.overflowed.store(true, Ordering::Release);
                match e {
                    async_channel::TrySendError::Full(v) => Err(TrySendError(v)),
                    async_channel::TrySendError::Closed(v) => Err(TrySendError(v)),
                }
            },
        }
    }
}


pub struct Receiver<T>{
    rx: async_channel::Receiver<T>,
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

        self.rx.try_recv().map_err(|e|{
            match e {
                async_channel::TryRecvError::Empty => TryRecvError::Empty,
                async_channel::TryRecvError::Closed => TryRecvError::Empty,
            }
        })
    }
}

impl<T> RecvOp<T> for Receiver<T> 
where 
    T: Clone,
{ 
    type Fut<'a> = impl Future<Output = Result<T, RecvError> > + 'a where Self: 'a;

    fn recv(&mut self) -> Self::Fut<'_> { 
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

// impl<T> RecvOp<T> for Receiver<T> 
// where 
//     T: Clone,
// { 
//     type Fut<'a> = RecvFut<'a, T> where T: 'a;

//     fn recv(&mut self) -> Self::Fut<'_> { 
//         let fut = RecvFut(self.rx.recv());
//         fut
//     }
// }

// use std::pin::Pin;
// use std::task::Poll;

// pub struct RecvFut<'a, T>(async_channel::Recv<'a, T>);

// impl<'a, T> Future for RecvFut<'a, T> 
// where 
//     T: Clone,
// {
//     type Output = Result<T, RecvError>;

//     fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> { 
//         let r = Pin::new(&mut self.0).poll(cx);
//         match r {
//             Poll::Ready(r) => {
//                 match r {
//                     Ok(v) => Poll::Ready(Ok(v)),
//                     Err(_e) => Poll::Ready(Err(RecvError)),
//                 }
//             },
//             Poll::Pending => Poll::Pending,
//         }
//     }
// }

