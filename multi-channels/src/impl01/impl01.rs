///
/// 用 AtomicWaker 实现自定义的 channel：
/// - 每个 suber 有一个 ready-ch-id 列表
/// - 每个 channel 有一个 suber 列表
/// - channel 在 publish 时遍历 suber 列表
///   - 添加 ch_id 到 ready-ch-id 列表
///   - 唤醒 suber
/// 

use std::{sync::Arc, pin::Pin, task::Poll};
use std::ops::Deref;
use anyhow::{Result, bail};
use futures::{task::AtomicWaker, Future};
use parking_lot::{Mutex, RwLock};
use crate::{ch_common::{SeqVal, RecvOutput, GetSeq, ReadQueOutput, ChIdOp, WithSeq, ChDeque, VecSet, VecMap}, define_arc_hash};

pub fn impl_name() -> &'static str {
    "impl01-AtomicWaker-Custom"
}

/// Seq Channel
pub type SChannel<K, T> = Channel<K, SeqVal<T>>;

#[derive(Clone)]
pub struct Channel<K, T> {
    shared: Arc<ChShared<K, T>>,
}

impl<K, T> Channel<K, T> {
    pub fn with_capacity(ch_id: K, capacity: usize) -> Self {
        Self {
            shared: Arc::new(ChShared { 
                queue: RwLock::new(ChDeque::new()),
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

    fn remove_suber(&self, k: &SuberShared<K>) {
        self.shared.subers.lock().remove(k);
    }
}

pub struct Suber<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    cursors: VecMap<K, Cursor<K, T>>,
    ready_ch_ids: VecSet<K>,
    suber_shared: Arc<SuberShared<K>>,
    last_ready_index: usize,
}

impl<K, T> Suber<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    pub fn new() -> Self {
        Self { 
            cursors: VecMap::new(),
            ready_ch_ids: VecSet::new(),
            suber_shared: Default::default(),
            last_ready_index: 0,
        }
    }

    pub fn subscribe(&mut self, ch: &Channel<K, T>, seq: u64) -> Result<()> {

        let r = self.cursors.get(ch.ch_id());
        if let Some(_index) = r {
            bail!("already subscribed channel [{:?}]", ch.shared.ch_id)
        }

        let cursor = Cursor { seq, ch: ch.clone() };
        self.cursors.insert(ch.ch_id().clone(), cursor);

        let mut subers = ch.shared.subers.lock();
        subers.insert(HashSuberShared(self.suber_shared.clone()));

        Ok(())
    }

    pub fn unsubscribe(&mut self, ch_id: &K) -> Option<Channel<K, T>> { 
        let r = self.cursors.remove(ch_id).map(|x|x.ch);
        if let Some(ch) = &r {
            ch.remove_suber(self.suber_shared.deref());
        }
        r
    }

    pub fn channels(&self) -> usize {
        self.cursors.len()
    }

}

impl<K, T> Drop for Suber<K, T> 
where
    K: ChIdOp,
    T: Clone,
{
    fn drop(&mut self) {
        for (_id, cursor) in self.cursors.iter() {
            cursor.ch.remove_suber(self.suber_shared.deref());
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
            while self.ready_ch_ids.len() > 0 {
                let r = self.read_ready(self.last_ready_index);
                if let RecvOutput::None = r {
                    self.inc_last_index();
                } else {
                    self.inc_last_index();
                    return r;
                }
            }
    
            {
                let mut set = self.suber_shared.ready_ch_ids.lock();

                while let Some(v) = set.pop() {
                    self.ready_ch_ids.insert(v);
                }
            }

            if self.ready_ch_ids.is_empty() {
                return RecvOutput::None;
            }
        }
    }

    pub async fn recv_next(&mut self) -> RecvOutput<K, T> { 
        loop {
            let r = self.try_recv();
            if let RecvOutput::None = r {
                self.suber_shared.as_ref().await;
            } else {
                return r
            }
        }
    }

    fn inc_last_index(&mut self) {
        self.last_ready_index += 1;
        if self.last_ready_index >= self.ready_ch_ids.len() {
            self.last_ready_index = 0;
        }
    }

    fn read_ready(&mut self, index: usize) -> RecvOutput<K, T> {
        let r = self.ready_ch_ids.get_index(index);
        if let Some(ch_id) = r {
            let ch_id = ch_id.clone();
            let r = self.cursors.get_mut(&ch_id);
            match r {
                Some(cursor) => {
                    let r = cursor.read_next();
                    if !r.is_none() {
                        return r;
                    }
                },
                None => {},
            }
            self.ready_ch_ids.remove(&ch_id);
        } 
        RecvOutput::None
    }

}

pub struct Puber<K, T> {
    ch_shared: Arc<ChShared<K, T>>,
}

impl<K, T> Puber<K, T> 
where
    K: ChIdOp,
    T: Clone + GetSeq,
{
    pub fn push_raw(&mut self, v: T) -> Result<()> {
        {
            let mut queue = self.ch_shared.queue.write();
            queue.push_raw(v, self.ch_shared.capacity)?;
        }

        self.wakeup_all();

        Ok(())
    }

    fn wakeup_all(&mut self) {
        let ch_id = &self.ch_shared.ch_id;
        let subers = self.ch_shared.subers.lock();
        for suber in subers.deref() { 
            suber.0.ready_ch_ids.lock().insert(ch_id.clone());
            suber.0.waker.wake();
        }
    }
}

impl<K, T> Puber<K, T> 
where
    K: ChIdOp,
    T: Clone + GetSeq + WithSeq,
{
    pub fn push(&mut self, v: T::Value) -> Result<()> {
        {
            let mut queue = self.ch_shared.queue.write();
            let v = T::with_seq(queue.next_seq(), v);   
            queue.push_raw(v, self.ch_shared.capacity)?;
        }

        self.wakeup_all();

        Ok(())
    }
}

struct SuberShared<K> {
    waker: AtomicWaker,
    ready_ch_ids: Mutex<VecSet<K>>,
}

impl<K> Default for SuberShared<K> {
    fn default() -> Self {
        Self { waker: AtomicWaker::new(), ready_ch_ids: Mutex::new(VecSet::new()) }
    }
}

impl<K> Future for &SuberShared<K> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let readys = self.ready_ch_ids.lock();
        if !readys.is_empty() {
            return Poll::Ready(())
        }

        self.waker.register(cx.waker());

        Poll::Pending
    }
}

struct ChShared<K, T> {
    capacity: usize,
    ch_id: K,
    subers: Mutex<VecSet< HashSuberShared<K> >>,
    queue: RwLock<ChDeque<T>>,
}


struct Cursor<K, T> {
    seq: u64,
    ch: Channel<K, T>, // Arc<ChShared<K, T>>,
}

impl<K, T> Cursor<K, T>  
where
    K: ChIdOp,
    T: Clone + GetSeq,
{
    pub fn read_next(&mut self) -> RecvOutput<K, T> {
        let r =  {
            let queue = self.ch.shared.queue.read();
            queue.read_next(self.seq)
        };

        match r {
            ReadQueOutput::Latest => RecvOutput::None,
            ReadQueOutput::Value(v) => {
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

struct HashSuberShared<K>(Arc<SuberShared<K>>);
define_arc_hash!(HashSuberShared<K>, SuberShared<K>);

