

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
        let (tx, rx) = tokio::sync::broadcast::channel(cap);
        (Sender(tx), Receiver(rx))
    }

    fn name() -> &'static str {
        "tokio_broadcast"
    }
}

pub struct Sender<T>(tokio::sync::broadcast::Sender<T>);

impl<T> Clone for Sender<T> 
where 
    T: Clone,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> SenderOp<T> for Sender<T> 
where 
    T: Clone,
{
    fn try_send(&mut self, msg: T) -> Result<(), TrySendError<T>> {
        self.0.send(msg).map_err(|e| TrySendError(e.0))?;
        Ok(())
    }
}

pub struct Receiver<T>(tokio::sync::broadcast::Receiver<T>);

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

        self.0.try_recv().map_err(|e|{
            use tokio::sync::broadcast::error;
            match e {
                error::TryRecvError::Lagged(_n) => TryRecvError::Overflowed,
                error::TryRecvError::Empty => TryRecvError::Empty,
                error::TryRecvError::Closed => TryRecvError::Empty,
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
            let r = self.0.recv().await;
            match r {
                Ok(v) => Ok(v),
                Err(_e) => Err(RecvError),
            }
        }
    }
}




