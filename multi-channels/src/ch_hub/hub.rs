use std::marker::PhantomData;

use anyhow::Result;
use parking_lot::Mutex;
use crate::{ch_common::{GetSeq, ChIdOp, VecMap}, mpsc_ch::mpsc_defs::MpscOp};

use super::{store::ChStore, event::Event, channel1::{channel::Channel, suber::Suber}};


// /// T: Seq Message
// /// 
// /// S: Channel Store
// pub struct Channel<T, S> {
//     store: S,
//     bus: Bus<T>,
// }

// impl<'s, T, S> Channel<T, S> 
// where
//     T: Clone + GetSeq + WithSeq,
//     S: ChStore<'s, T>,
// {
//     pub fn new(store: S) -> Self {
//         Self {
//             bus: Bus::new(),
//             store,
//         }
//     }
// }




pub struct Hub<K, T, M> 
where
    K: ChIdOp,
    T: Clone,
    M: MpscOp<Event<K, T>>,
{
    none: PhantomData<T>,
    channels: Mutex<VecMap<K, Channel<K, T, M>>>,
}

impl<K, T, M> Hub<K, T, M> 
where
    K: ChIdOp,
    T: Clone + GetSeq,
    M: MpscOp<Event<K, T>>,
{
    pub fn new() -> Self {
        Self { 
            none: Default::default(),
            channels: Mutex::new(VecMap::new()),
        }
    }

    /// auto create channel if NOT exist
    pub async fn subscribe(&self, ch_id: &K, suber: &mut Suber<K, T, M>, seq: u64) -> Result<()> {
        let ch = self.get_or_add_ch(ch_id).await?;
        suber.subscribe(&ch, seq)
    }

    /// auto create channel if NOT exist
    pub async fn puber(&self, ch_id: &K) -> Result<Puber<K, T, M>> {
        let ch = self.get_or_add_ch(ch_id).await?;
        Ok(ch.clone())
    }

    async fn get_or_add_ch(&self, ch_id: &K) -> Result<Channel<K, T, M>> {
        let mut channels = self.channels.lock();
        let ch = channels.entry(ch_id.clone()).or_insert_with(||Channel::with_capacity(ch_id.clone(), 16));
        Ok(ch.clone())
    }
}

pub type Puber<K, T, M> = Channel<K, T, M>;

// pub struct Puber<T> {
//     none: PhantomData<T>,
// }

// impl<T> Puber<T> 
// where
//     T: Clone + GetSeq + WithSeq,
// {
//     pub fn new() -> Self {
//         Self { 
//             none: Default::default() 
//         }
//     }

//     pub async fn push(&self, v: T::Value) -> Result<()> { 
//         Ok(())
//     }

// }



#[cfg(test)]
mod test {
    use crate::{ch_common::{SeqVal, uid::ChId}, ch_hub::store::mem_store::ChMemStore, mpsc_ch::mpsc_crossbeam_que::Mpsc};

    use super::*;
    
    #[tokio::test]
    async fn test() { 
        // let ch_store = ChMemStore::<Message>::new();
        // let ch = Channel::new(ch_store);
        let hub = Hub::<ChId, Message, Mpsc>::new();

        let mut suber = Suber::with_inbox_cap(16);
        let r = hub.subscribe(&ChId::new(1), &mut suber, 1).await;
        assert!(r.is_ok(), "r={:?}", r);
        
        let puber = hub.puber(&ChId::new(1)).await.unwrap();
        puber.push(1_u64).unwrap();

        let r: Message = suber.async_recv().await.unwrap();
        assert_eq!(r, Message::new(1, 1));
    }

    type Message = SeqVal<u64>;
}




