///
/// mpsc channel defines
/// 

use futures::Future;
use self::error::*;


pub trait MpscOp<T> { 
    type Sender: SenderOp<T>;
    
    type Receiver: ReceiverOp<T>;

    fn channel(cap: usize) -> (Self::Sender, Self::Receiver);
    
    fn name() -> &'static str; 
}

pub trait SenderOp<T> : Clone {
    fn try_send(&mut self, msg: T) -> Result<(), TrySendError<T>>;
}

pub trait ReceiverOp<T>: TryRecvOp<T> + AsyncRecvOp<T> { 
    fn clear(&mut self) {
        loop {
            let r = self.try_recv();
            if let Err(e) = r {
                match e {
                    TryRecvError::Overflowed => {},
                    TryRecvError::Empty => return,
                }
            }
        }
    }
}

pub trait TryRecvOp<T> {
    fn try_recv(&mut self) -> Result<T, TryRecvError>;
}

pub trait AsyncRecvOp<T> 
{ 
    type Fut<'a>: Future<Output = Result<T, RecvError> >
    where Self: 'a ;

    fn async_recv(&mut self) -> Self::Fut<'_>;
}



pub mod error { 
    /// refer from async_broadcast error

    use std::error;
    use std::fmt;


    #[derive(PartialEq, Eq, Clone, Copy)]
    pub struct TrySendError<T>(pub T);

    impl<T> TrySendError<T> {
        /// Unwraps the message that couldn't be sent.
        pub fn into_inner(self) -> T {
            self.0
        }
    }

    impl<T> error::Error for TrySendError<T> {}

    impl<T> fmt::Debug for TrySendError<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "SendError(..)")
        }
    }

    impl<T> fmt::Display for TrySendError<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "sending into a full channel")
        }
    }

    /// The channel has overflowed since the last element was seen
    #[derive(PartialEq, Eq, Clone, Copy, Debug)]
    pub struct RecvError;

    impl error::Error for RecvError {}

    impl fmt::Display for RecvError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { 
            write!(f, "receiving but got overflowed", )
        }
    }

    /// An error returned from [`Receiver::try_recv()`].
    #[derive(PartialEq, Eq, Clone, Copy, Debug)]
    pub enum TryRecvError {
        /// The channel has overflowed since the last element was seen. 
        Overflowed,

        /// The channel is empty but not closed.
        Empty,
    }

    impl TryRecvError {
        /// Returns `true` if the channel is empty but not closed.
        pub fn is_empty(&self) -> bool {
            match self {
                TryRecvError::Empty => true,
                TryRecvError::Overflowed => false,
            }
        }

        /// Returns `true` if this error indicates the receiver missed messages.
        pub fn is_overflowed(&self) -> bool {
            match self {
                TryRecvError::Empty => false,
                TryRecvError::Overflowed => true,
            }
        }
    }

    impl error::Error for TryRecvError {}

    impl fmt::Display for TryRecvError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match *self {
                TryRecvError::Empty => write!(f, "receiving from an empty channel"),
                TryRecvError::Overflowed => {
                    write!(f, "receiving operation but see overflowed")
                }
            }
        }
    }
}
