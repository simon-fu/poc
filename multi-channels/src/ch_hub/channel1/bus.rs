use std::sync::Arc;

use parking_lot::Mutex;

use crate::{
    mpsc_ch::{
        mpsc_defs::{MpscOp, SenderOp}}, 
        ch_common::{ChIdOp, uid::{next_suber_id, SuberId}, 
        VecMap
    }
};

use super::super::event::Event;

#[derive(Clone)]
pub struct Bus<K, T, M: MpscOp<Event<K, T>>> {
    shared: Arc<Shared<K, T, M>>,
}

impl<K, T, M> Bus<K, T, M>
where
    K: ChIdOp,
    T: Clone, // + GetSeq,
    M: MpscOp<Event<K, T>>, 
{
    pub fn new() -> Self {
        let subers = Mutex::new(VecMap::new());
        Self { shared: Arc::new(Shared { subers }) }
    }

    pub fn subers(&self) -> usize {
        self.shared.subers.lock().len()
    }

    pub fn watch(&self, watcher: &Watcher<K, T, M>) {
        let key = watcher.id;
        self.shared.subers.lock().insert(key, watcher.tx.clone());
    }

    pub fn unwatch(&self, key: &SuberId) -> bool {
        self.shared.subers.lock().remove(key).is_some()
    }

    pub fn broadcast(&self, ev: Event<K, T>) {
        let mut subers = self.shared.subers.lock();
        for (_k, tx) in subers.iter_mut() { 
            let _r = tx.try_send(ev.clone());
        }
    }
}

pub struct Shared<K, T, M: MpscOp<Event<K, T>>> {
    subers: Mutex<VecMap<SuberId, M::Sender>>,
}

// pub type Sender<EV> = LSender<EV>;
// pub type Receiver<EV> = LReceiver<EV>;

#[derive(Clone)]
pub struct Watcher<K, T, M> 
where
    K: ChIdOp,
    T: Clone, // + GetSeq,
    M: MpscOp<Event<K, T>>, 
{
    tx: M::Sender, // LSender<Event<K, T>>, // Sender<Mail<K, T>>,
    rx: M::Receiver,
    id: SuberId,
}

impl<K, T, M> Watcher<K, T, M> 
where
    K: ChIdOp,
    T: Clone, // + GetSeq,
    M: MpscOp<Event<K, T>>, 
{
    pub fn new(cap: usize) -> Self { 
        let (tx, rx) = M::channel(cap);
        Self {
            tx,
            rx,
            id: next_suber_id(),
        }
    }

    pub fn id(&self)-> &SuberId {
        &self.id
    }

    pub(super) fn rx(&mut self) -> &mut M::Receiver {
        &mut self.rx
    }
}

// // watcher
// // listener
