
use std::{task::Poll, pin::Pin};

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
        let (mut tx, rx) = async_broadcast::broadcast(cap);
        tx.set_overflow(true);
        (Sender(tx), Receiver(rx))
    }

    fn name() -> &'static str {
        "async_broadcast"
    }
}

pub struct Sender<T>(async_broadcast::Sender<T>);

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
        self.0.try_broadcast(msg).map_err(|e| TrySendError(e.into_inner()))?;
        Ok(())
    }
}

pub struct Receiver<T>(async_broadcast::Receiver<T>);

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
            match e {
                async_broadcast::TryRecvError::Overflowed(_n) => TryRecvError::Overflowed,
                async_broadcast::TryRecvError::Empty => TryRecvError::Empty,
                async_broadcast::TryRecvError::Closed => TryRecvError::Empty,
            }
        })
    }
}

impl<T> AsyncRecvOp<T> for Receiver<T> 
where 
    T: Clone,
{ 
    type Fut<'a> = RecvFut<'a, T> where T: 'a;

    fn async_recv(&mut self) -> Self::Fut<'_> { 
        let fut = RecvFut(self.0.recv());
        fut
    }
}

pub struct RecvFut<'a, T>(async_broadcast::Recv<'a, T>);

impl<'a, T> Future for RecvFut<'a, T> 
where 
    T: Clone,
{
    type Output = Result<T, RecvError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> { 
        let r = Pin::new(&mut self.0).poll(cx);
        match r {
            Poll::Ready(r) => {
                match r {
                    Ok(v) => Poll::Ready(Ok(v)),
                    Err(_e) => Poll::Ready(Err(RecvError)),
                }
            },
            Poll::Pending => Poll::Pending,
        }
    }
}


// impl<T> RecvOp<T> for Receiver<T> 
// where 
//     T: Clone,
// { 
//     // type Fut<'a> where Self: 'a,
//     // = impl Future<Output = Result<T, RecvError> >;

//     type Fut<'a>
//     = impl Future<Output = Result<T, RecvError> >
//     where Self: 'a;


//     // fn recv(&mut self) -> Self::Fut<'_> {
//     fn recv<'a>(&'a mut self) -> Self::Fut<'a> { 
//         let fut = async move {
//             self.0.recv().await.map_err(|e|{
//                 match e {
//                     async_broadcast::RecvError::Overflowed(_n) => RecvError,
//                     async_broadcast::RecvError::Closed => RecvError,
//                 }
//             })
//         };
//         fut
//         // // let fut = RecvFut(self.0.recv());
//         // let fut = self.0.recv().map_err(|e|{
//         //     match e {
//         //         async_broadcast::RecvError::Overflowed(_n) => RecvError,
//         //         async_broadcast::RecvError::Closed => RecvError,
//         //     }
//         // });
//         // RecvFut(fut)
//     }
// }




// pub trait KvIterator {
//     type NextFuture<'a>: Future<Output = Option<(&'a [u8], &'a [u8])>>
//     where
//         Self: 'a;

//     /// Get the next item from the iterator.
//     fn next(&mut self) -> Self::NextFuture<'_>;
// }

// pub struct TestIterator {
//     idx: usize,
//     to_idx: usize,
//     key: Vec<u8>,
//     value: Vec<u8>,
// }

// impl TestIterator {
//     pub fn new(from_idx: usize, to_idx: usize) -> Self {
//         Self {
//             idx: from_idx,
//             to_idx,
//             key: Vec::new(),
//             value: Vec::new(),
//         }
//     }
// }
// impl KvIterator for TestIterator {
//     type NextFuture<'a>
//     where
//         Self: 'a,
//     = impl Future<Output = Option<(&'a [u8], &'a [u8])>>;

//     fn next(&mut self) -> Self::NextFuture<'_> {
//         use std::io::Write;
//         async move {
//             if self.idx >= self.to_idx {
//                 return None;
//             }

//             // Zero-allocation key value manipulation

//             self.key.clear();
//             write!(&mut self.key, "key_{:05}", self.idx).unwrap();

//             self.value.clear();
//             write!(&mut self.value, "value_{:05}", self.idx).unwrap();

//             self.idx += 1;
//             Some((&self.key[..], &self.value[..]))
//         }
//     }
// }


// mod test_impl2 { 
//     use std::future::Future;

//     pub trait Trait1 {
//         type Fut<'a>: Future<Output = Option<&'a usize>> where Self: 'a;
//         fn func1(&mut self) -> Self::Fut<'_>;
//     }
    
//     pub struct Wrap1(usize);
    
//     impl Trait1 for Wrap1 {
//         type Fut<'a>
//         = impl Future<Output = Option<&'a usize>> where Self: 'a;
    
//         fn func1(&mut self) -> Self::Fut<'_> {
//             async move {
//                 Some(&self.0)
//             }
//         }
//     }

//     pub trait Trait2 {
//         type Fut<'a>: Future<Output = Option<usize>> where Self: 'a;
//         fn func1(&mut self) -> Self::Fut<'_>;
//     }
    
//     pub struct Wrap2(usize);
    
//     impl Trait2 for Wrap2 {
//         type Fut<'a>
//         = impl Future<Output = Option<usize>> + 'a where Self: 'a;
    
//         fn func1(&mut self) -> Self::Fut<'_> {
//             async move {
//                 Some(self.0)
//             }
//         }
//     }
// }
