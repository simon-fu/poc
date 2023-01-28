///
/// 用 async-broadcast 实现：
/// - 每个 suber 有一个 async-broadcast rx 
/// - 每个 channel 有一个 suber 列表，列表 item 为 async-broadcast tx
/// - channel 在 publish 时遍历 suber 列表，tx.send
/// - 消息保存在 channel queue 里以外，还保存一份在 async-broadcast 里，多了一份冗余
/// - TODO: 
///   - 把 async-broadcast 抽象出来，用 async-channel、tokio::sync::broadcast、flume、AtomicWaker 实现
///   - broadcast 带上 active index
/// 

use std::sync::Arc;
use std::ops::Deref;
use anyhow::{Result, bail};
use async_broadcast::{broadcast, Receiver, Sender, TryRecvError};
use parking_lot::Mutex;
use crate::ch_common::uid::{SuberId, next_suber_id};
use crate::ch_common::{SeqVal, RecvOutput, GetSeq, ReadQueOutput, ChIdOp, WithSeq, ChDeque, VecMap};

pub fn impl_name() -> &'static str {
    "impl02-async-broadcast"
}

/// Seq Channel
pub type SChannel<K, T> = Channel<K, SeqVal<T>>;

#[derive(Clone)]
pub struct Channel<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    shared: Arc<ChShared<K, T>>,
}

impl<K, T> Channel<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    pub fn with_capacity(ch_id: K, capacity: usize) -> Self {
        Self {
            shared: Arc::new(ChShared { 
                queue: Mutex::new(ChDeque::new()),
                subers: Default::default(),
                capacity,
                ch_id,
            }
        )}
    }

    pub fn ch_id(&self) -> &K {
        &self.shared.ch_id
    }

    pub fn puber(&self) -> Puber<K, T> {
        Puber { ch_shared: self.shared.clone() }
    }

    pub fn subers(&self) -> usize {
        self.shared.subers.lock().len()
    }

    fn insert_suber(&self, suber: &Suber<K, T>) {
        let mut subers = self.shared.subers.lock();
        let key = suber.id;
        subers.insert(key, suber.tx.clone());
    }

    fn remove_suber(&self, key: &SuberId) {
        self.shared.subers.lock().remove(key);
    }
}

pub struct Suber<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    active_cursors: VecMap<K, Cursor<K, T>>,
    pending_cursors: VecMap<K, Cursor<K, T>>,
    last_pending_index: usize,
    tx: Sender<(K, T)>,
    rx: Receiver<(K, T)>,
    id: SuberId,
}

impl<K, T> Suber<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    pub fn new() -> Self {
        let (mut tx, rx) = broadcast(64);
        tx.set_overflow(true);

        Self { 
            active_cursors: VecMap::new(),
            pending_cursors: VecMap::new(),
            last_pending_index: 0,
            tx,
            rx,
            id: next_suber_id(),
        }
    }

    pub fn subscribe(&mut self, ch: &Channel<K, T>, seq: u64) -> Result<()> {

        if self.exist_channel(ch.ch_id()) {
            bail!("already subscribed channel [{:?}]", ch.shared.ch_id)
        }

        let cursor = Cursor { seq, ch: ch.clone() };
        cursor.ch.insert_suber(self);
        self.pending_cursors.insert(cursor.ch.ch_id().clone(), cursor);

        Ok(())
    }

    pub fn unsubscribe(&mut self, ch_id: &K) -> Option<Channel<K, T>> { 
        let r = remove_channel(&mut self.active_cursors, ch_id, &self.id);
        if r.is_some() {
            r
        } else {
            remove_channel(&mut self.pending_cursors, ch_id, &self.id)
        }
    }

    pub fn channels(&self) -> usize {
        self.active_cursors.len() + self.pending_cursors.len()
    }

    fn exist_channel(&self, key: &K) -> bool {
        let r = self.active_cursors.get(key);
        if r.is_some() {
            true
        } else {
            self.pending_cursors.get(key).is_some()
        }
    }
}

fn remove_channel<K, T>(cursors: &mut VecMap<K, Cursor<K, T>>, ch_id: &K, sub_id: &SuberId) -> Option<Channel<K, T>> 
where
    K: ChIdOp,
    T: Clone,
{
    let r = cursors.remove(ch_id).map(|x|x.ch);
    if let Some(ch) = &r {
        ch.remove_suber(sub_id);
    }
    r
}


impl<K, T> Drop for Suber<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    fn drop(&mut self) {
        for (_id, cursor) in self.active_cursors.iter() {
            cursor.ch.remove_suber(&self.id);
        }

        for (_id, cursor) in self.pending_cursors.iter() {
            cursor.ch.remove_suber(&self.id);
        }
    }
}


impl<K, T> Suber<K, T> 
where
    K: ChIdOp,
    T: Clone + GetSeq,
{
    pub fn try_recv(&mut self) -> RecvOutput<K, T> { 

        loop { 
            while self.pending_cursors.len() > 0 { 
                let r = self.read_pending_at(self.last_pending_index);
                
                self.inc_last_pending_index();

                if !r.is_none() {
                    return r;
                } 
            }

            let r = self.try_recv_active();
                
            if !r.is_none() {
                return r;
            }

            if self.pending_cursors.len() == 0 {
                return RecvOutput::None;
            }            
        }
    }

    pub async fn recv_next(&mut self) -> RecvOutput<K, T> { 
        loop {
            let r = self.try_recv();
            if !r.is_none() {
                return r;
            }

            let r = self.rx.recv().await;
            match r {
                Ok((ch_id, v)) => {
                    let r = self.process_recved(ch_id, v);
                    if !r.is_none() {
                        return r;
                    }
                },
                Err(_e) => {
                    self.move_all_actives_to_pendings();
                },
            }
        }
    }

    fn try_recv_active(&mut self) -> RecvOutput<K, T> { 
        loop {
            let r = self.rx.try_recv();
            match r {
                Ok((ch_id, v)) => {
                    let r = self.process_recved(ch_id, v);
                    if !r.is_none() {
                        return r;
                    }
                },
                Err(e) => {
                    match e {
                        TryRecvError::Overflowed(_n) => {
                            self.move_all_actives_to_pendings();
                        },
                        TryRecvError::Empty 
                        | TryRecvError::Closed // never recv Closed
                        => {}, 
                    }
                    return RecvOutput::None;
                },
            }
        }
    }

    fn process_recved(&mut self, ch_id: K, v: T) -> RecvOutput<K, T>{
        let r = self.active_cursors.get_mut(&ch_id);
        if let Some(cursor) = r {
            if v.get_seq() == cursor.seq {
                return cursor.output_value(v);

            } else if v.get_seq() > cursor.seq { 
                let r = self.active_cursors.remove(&ch_id);
                if let Some(cursor) = r {
                    self.pending_cursors.insert(cursor.ch.ch_id().clone(), cursor);
                }
            } 
            // ignore
        }
        RecvOutput::None
    }

    fn read_pending_at(&mut self, index: usize) -> RecvOutput<K, T> {
        let r = self.pending_cursors.get_index_mut(index);
        if let Some((_ch_id, cursor)) = r {
            let r = cursor.read_next();
            if r.is_none() {
                self.move_pending_to_active(index);
            }
            return r;
        } 
        RecvOutput::None
    }

    fn inc_last_pending_index(&mut self) {
        self.last_pending_index += 1;
        if self.last_pending_index >= self.pending_cursors.len() {
            self.last_pending_index = 0;
        }
    }

    fn move_all_actives_to_pendings(&mut self) {
        while let Some((_ch_id, cursor)) = self.active_cursors.pop() {
            self.pending_cursors.insert(cursor.ch.ch_id().clone(), cursor);
        }
    }

    fn move_pending_to_active(&mut self, index: usize) {
        let r = self.pending_cursors.swap_remove_index(index);
        if let Some((_ch_id, cursor)) = r {
            self.active_cursors.insert(cursor.ch.ch_id().clone(), cursor);
        }
    }

}

pub struct Puber<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    ch_shared: Arc<ChShared<K, T>>,
}

impl<K, T> Puber<K, T> 
where
    K: ChIdOp,
    T: Clone + GetSeq,
{
    pub fn push_raw(&mut self, v: T) -> Result<()> {
        {
            let mut queue = self.ch_shared.queue.lock();
            queue.push_raw(v.clone(), self.ch_shared.capacity)?;
        }

        self.broadcast_to_subers(v);

        Ok(())
    }

    fn broadcast_to_subers(&mut self, v: T) {
        let ch_id = &self.ch_shared.ch_id;
        let subers = self.ch_shared.subers.lock();
        for (_k, tx) in subers.deref() { 
            let _r = tx.try_broadcast((ch_id.clone(), v.clone()));
            debug_assert!(_r.is_ok());
        }
    }
}

impl<K, T> Puber<K, T> 
where
    K: ChIdOp,
    T: Clone + GetSeq + WithSeq,
{
    pub fn push(&mut self, v: T::Value) -> Result<()> {
        let v = {
            let mut queue = self.ch_shared.queue.lock();
            let v = T::with_seq(queue.next_seq(), v);   
            queue.push_raw(v.clone(), self.ch_shared.capacity)?;
            v
        };

        self.broadcast_to_subers(v);

        Ok(())
    }
}


struct ChShared<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    capacity: usize,
    ch_id: K,
    subers: Mutex<VecMap<SuberId,  Sender<(K, T)>>>,
    queue: Mutex<ChDeque<T>>,
}


struct Cursor<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    seq: u64,
    ch: Channel<K, T>, // Arc<ChShared<K, T>>,
}

impl<K, T> Cursor<K, T>  
where
    K: ChIdOp,
    T: Clone + GetSeq,
{
    pub fn read_next(&mut self) -> RecvOutput<K, T> {
        let queue = self.ch.shared.queue.lock();
        let r = queue.read_next(self.seq);
        match r {
            ReadQueOutput::Latest => RecvOutput::None,
            ReadQueOutput::Value(v) => {
                drop(queue);
                return self.output_value(v);
            }
            ReadQueOutput::Lagged => {
                return RecvOutput::Lagged(self.ch.ch_id().clone());
            },
        }
    }

    pub fn output_value(&mut self, v: T) -> RecvOutput<K, T> {
        self.seq = v.get_seq() + 1;
        RecvOutput::Value(self.ch.ch_id().clone(), v)
    }
}



