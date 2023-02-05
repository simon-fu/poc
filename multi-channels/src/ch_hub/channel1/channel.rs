///
/// 用 async-broadcast 实现：
/// - 每个 suber 有一个 async-broadcast rx 
/// - 每个 channel 有一个 suber 列表，列表 item 为 async-broadcast tx
/// - channel 在 publish 时遍历 suber 列表，tx.send
/// - 消息保存在 channel queue 里以外，还保存一份在 async-broadcast 里，多了一份冗余
/// - TODO:  
///   - broadcast 带上 active index
/// 

use std::sync::Arc;
use anyhow::Result;
// use async_broadcast::{broadcast, Receiver, Sender, TryRecvError};
use parking_lot::RwLock;
use crate::ch_common::uid::SuberId;
use crate::ch_common::{SeqVal, RecvOutput, GetSeq, ReadQueOutput, ChIdOp, WithSeq, ChDeque};

use crate::mpsc_ch::mpsc_defs::MpscOp;

use super::bus::Bus;
use super::super::event::Event;
use super::suber::Suber;

pub fn impl_name() -> &'static str {
    "impl02"
}

/// Seq Channel
pub type SChannel<K, T, M> = Channel<K, SeqVal<T>, M>;

///
/// Generic types:
/// - K is Channel Id
/// - T is Message
/// - M is Mpsc
pub struct Channel<K, T, M> 
where
    K: ChIdOp,
    T: Clone,
    M: MpscOp<Event<K, T>>,
{
    shared: Arc<ChShared<K, T, M>>,
}

impl<K, T, M> Clone for Channel<K, T, M> 
where
    K: ChIdOp,
    T: Clone,
    M: MpscOp<Event<K, T>>,
{
    fn clone(&self) -> Self {
        Self { shared: self.shared.clone() }
    }
}

impl<K, T, M> Channel<K, T, M> 
where
    K: ChIdOp,
    T: Clone + GetSeq,
    M: MpscOp<Event<K, T>>,
{
    pub fn with_capacity(ch_id: K, cap: usize) -> Self {
        Self {
            shared: Arc::new(ChShared { 
                // queue: RwLock::new(ChDeque::new()),
                // subers: Default::default(),
                cache: ChCache::with_capacity(cap),
                bus: Bus::new(),
                // capacity: cap,
                ch_id,
            }
        )}
    }

    pub fn ch_id(&self) -> &K {
        &self.shared.ch_id
    }

    pub fn capacity(&self) -> usize {
        self.shared.cache.queue.read().capacity()
    }

    // pub fn puber(&self) -> Puber<K, T, M> {
    //     Puber { ch_shared: self.shared.clone() }
    // }

    pub fn subers(&self) -> usize {
        // self.shared.subers.lock().len()
        self.shared.bus.subers()
    }

    pub fn tail_seq(&self) -> u64 {
        // self.shared.queue.read().next_seq()
        self.shared.cache.tail_seq()
    }

    pub(super) fn insert_suber(&self, suber: &Suber<K, T, M>) {
        self.shared.bus.watch(suber.watcher());
        // let mut subers = self.shared.subers.lock();
        // let key = *suber.id();
        // subers.insert(key, suber.tx());
    }

    pub(super) fn remove_suber(&self, key: &SuberId) {
        self.shared.bus.unwatch(key);
        // self.shared.subers.lock().remove(key);
    }

}

impl<K, T, M> Channel<K, T, M> 
where
    K: ChIdOp,
    T: Clone + GetSeq + WithSeq,
    M: MpscOp<Event<K, T>>,
{
    pub(super) fn push_raw(&self, v: T) -> Result<()> {
        self.shared.cache.push_raw(v.clone())?;
        self.broadcast_to_subers(v);
        Ok(())
    }

    fn broadcast_to_subers(&self, v: T) {
        self.shared.bus.broadcast(Event::msg(self.shared.ch_id.clone(), v.clone()));
        // let ch_id = &self.shared.ch_id;
        // let mut subers = self.shared.subers.lock();
        // for (_k, tx) in subers.deref_mut() { 
        //     let _r = tx.try_send(Event::msg(ch_id.clone(), v.clone()));
        // }
    }
}

impl<K, T, M> Channel<K, T, M> 
where
    K: ChIdOp,
    T: Clone + GetSeq + WithSeq,
    M: MpscOp<Event<K, T>>,
{
    pub(crate) fn push(&self, v: T::Value) -> Result<()> {
        // let v = {
        //     let mut queue = self.shared.queue.write();
        //     let v = T::with_seq(queue.next_seq(), v);   
        //     queue.push_raw(v.clone(), self.shared.capacity)?;
        //     v
        // };
        let v = self.shared.cache.push(v)?;

        self.broadcast_to_subers(v);

        Ok(())
    }
}

impl<K, T, M> ReadNext for Channel<K, T, M> 
where
    K: ChIdOp,
    T: Clone + GetSeq,
    M: MpscOp<Event<K, T>>,
{
    type Output = ReadQueOutput<T>;
    fn read_next(&self, seq: u64) -> Self::Output {
        self.shared.cache.queue.read().read_next(seq)
    }
}



struct ChShared<K, T, M> 
where
    K: ChIdOp,
    T: Clone,
    M: MpscOp<Event<K, T>>,
{
    // capacity: usize,
    ch_id: K,
    // subers: Mutex<VecMap<SuberId, M::Sender>>,
    // queue: RwLock<ChDeque<T>>,
    
    bus: Bus<K, T, M>,
    cache: ChCache<T>,
}

// pub struct Cursor<K, T, M> 
// where
//     K: ChIdOp,
//     T: Clone,
//     M: MpscOp<Event<K, T>>,
// {
//     pub(super) seq: u64,
//     pub(super) ch: Channel<K, T, M>, // Arc<ChShared<K, T>>,
// }

// impl<K, T, M> Cursor<K, T, M>  
// where
//     K: ChIdOp,
//     T: Clone + GetSeq,
//     M: MpscOp<Event<K, T>>,
// {
//     pub fn read_next(&mut self) -> RecvOutput<K, T> {
        
//         let r = {
//             let queue = self.ch.shared.queue.read();
//             queue.read_next(self.seq)
//         };

//         match r {
//             ReadQueOutput::Latest => RecvOutput::None,
//             ReadQueOutput::Value(v) => {
//                 return self.output_value(v);
//             }
//             ReadQueOutput::Lagged => {
//                 return RecvOutput::Lagged(self.ch.ch_id().clone());
//             },
//         }
//     }

//     pub fn output_value(&mut self, v: T) -> RecvOutput<K, T> {
//         self.seq = v.get_seq() + 1;
//         RecvOutput::Value(self.ch.ch_id().clone(), v)
//     }
// }

pub struct Cursor<R> 
// where
    // K: ChIdOp,
    // T: Clone,
    // M: MpscOp<Event<K, T>>,
    // R: ReadNext,
{
    pub(super) seq: u64,
    // pub(super) ch: Channel<K, T, M>, // Arc<ChShared<K, T>>,
    pub(super) ch: R, 
}

impl<R, T> Cursor<R>  
where
    // K: ChIdOp,
    T: Clone + GetSeq,
    // M: MpscOp<Event<K, T>>,
    R: ReadNext<Output = ReadQueOutput<T>>,
{
    pub fn read_next<K>(&mut self, ch_id: &K) -> RecvOutput<K, T> 
    where
        K: ChIdOp,
    {
        
        let r = self.ch.read_next(self.seq);
        // {
        //     let queue = self.ch.shared.queue.read();
        //     queue.read_next(self.seq)
        // };

        match r {
            ReadQueOutput::Latest => RecvOutput::None,
            ReadQueOutput::Value(v) => {
                self.on_output_value(v.get_seq());
                return RecvOutput::Value(ch_id.clone(), v);
                // return self.output_value(ch_id.clone(), v);
            }
            ReadQueOutput::Lagged => {
                return RecvOutput::Lagged(ch_id.clone());
            },
        }
    }

    // pub fn output_value<K>(&mut self, ch_id: K, v: T) -> RecvOutput<K, T> 
    // where 
    //     K: ChIdOp,
    // {
    //     self.seq = v.get_seq() + 1;
    //     RecvOutput::Value(ch_id, v)
    // }

    pub fn on_output_value(&mut self, seq: u64) {
        self.seq = seq + 1;
    }
}


pub struct ChCache<T> {
    queue: RwLock<ChDeque<T>>,
}

impl<T> ChCache<T> {
    // pub fn new() -> Self {
    //     Self{
    //         queue: RwLock::new(ChDeque::new()),
    //     }
    // }

    pub fn with_capacity(cap: usize) -> Self {
        Self{
            queue: RwLock::new(ChDeque::with_capacity(cap)),
        }
    }

    pub fn tail_seq(&self) -> u64 {
        self.queue.read().next_seq()
    }
}

impl<T> ChCache<T> 
where
    T: Clone + GetSeq,
{
    pub(super) fn push_raw(&self, v: T) -> Result<()> { 
        self.queue.write().push_raw(v)
    }
}

impl<T> ChCache<T> 
where
    T: Clone + GetSeq + WithSeq,
{
    pub(super) fn push(&self, v: T::Value) -> Result<T> {
        let mut queue = self.queue.write();
        let v = T::with_seq(queue.next_seq(), v);   
        queue.push_raw(v.clone())?;
        Ok(v)
    }
}



pub trait ReadNext {
    type Output;
    fn read_next(&self, seq: u64) -> Self::Output;
}
