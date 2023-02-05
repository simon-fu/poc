
use anyhow::Result;
use std::future::Future;
use crate::ch_common::{ReadQueOutput, WithSeq};


pub trait ChStore<'s, K, T> 
where
    T: WithSeq,
{
    fn read_hot(&self, seq: u64) -> ReadOutput<T>;

    /// return none if reach tail
    fn load_cold(&self, ch_id: &K, seq: u64) -> Self::ReadFut;
    type ReadFut: Future<Output = Result<Option<Vec<T>>>>;
    
    fn async_write(&'s self, v: T::Value) -> Self::WriteFut;
    type WriteFut: Future<Output = Result<T>>;
}

type ReadOutput<T> = ReadQueOutput<T>;


pub mod mem_store {

    use anyhow::Result;
    use parking_lot::RwLock;
    use std::{future::Future, marker::PhantomData};
    use crate::{ch_common::{ChDeque, WithSeq, GetSeq}};

    use super::{ChStore, ReadOutput};

    pub struct ChMemStore<K, T> { 
        hot: RwLock<ChDeque<T>>,
        cold: RwLock<ChDeque<T>>,
        none: PhantomData<(K, T)>,
    }
    
    impl<K, T> ChMemStore<K, T> {
        pub fn with_capacity(cap: usize) -> Self {
            Self {
                hot: RwLock::new(ChDeque::with_capacity(cap)),
                cold: RwLock::new(ChDeque::with_capacity(cap)),
                none: PhantomData,
            }
        }
    }
    
    impl<'s, K: 's, T> ChStore<'s, K, T> for ChMemStore<K, T> 
    where
        T: Clone + GetSeq + WithSeq + 's,
    {
        fn read_hot(&self, seq: u64) -> ReadOutput<T> {
            self.hot.read().read_next(seq)
        }
    
        /// return none if reach tail
        fn load_cold(&self, ch_id: &K, seq: u64) -> Self::ReadFut {
            async move {
                Ok(None)
            }
        }
    
        type ReadFut = impl Future<Output = Result<Option<Vec<T>>>>;
    
        fn async_write(&'s self, v: <T as WithSeq>::Value) -> Self::WriteFut { 
            async move { 
                let mut que = self.hot.write();
                let msg = T::with_seq(que.next_seq(), v);
                que.push_raw(msg.clone())?;
    
                self.cold.write().push_raw(msg.clone())?;
                Ok(msg)
            }
        }
    
        type WriteFut = impl Future<Output = Result<T>> + 's;
    }
    
}
