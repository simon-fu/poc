///
/// 用 async-broadcast 实现：
/// - 每个 suber 有一个 async-broadcast rx 
/// - 每个 channel 有一个 suber 列表，列表 item 为 async-broadcast tx
/// - channel 在 publish 时遍历 suber 列表，tx.send
/// - 消息保存在 channel queue 里以外，还保存一份在 async-broadcast 里，多了一份冗余
/// - TODO:  
///   - broadcast 带上 active index
/// 


use anyhow::{Result, bail};
// use async_broadcast::{broadcast, Receiver, Sender, TryRecvError};
use parking_lot::{Mutex, RwLock};
use crate::ch_common::uid::SuberId;
use crate::ch_common::{RecvOutput, GetSeq, ChIdOp, ChDeque, VecMap};

use crate::mpsc_ch::mpsc_defs::error::TryRecvError;
use crate::mpsc_ch::mpsc_defs::{MpscOp, AsyncRecvOp, TryRecvOp, ReceiverOp};

use super::bus::Watcher;
use super::channel::{Channel, Cursor};
use super::super::event::{Event, Msg};
use super::cursors::Cursors;

// pub struct Cursors<K, R> 
// // where
// //     K: ChIdOp,
// //     T: Clone,
// //     // M: MpscOp<Event<K, T>>,
// //     R: ReadNext<Output = ReadQueOutput<T>>,
// {
//     in_syncs: VecMap<K, Cursor<R>>,
//     out_of_syncs: VecMap<K, Cursor<R>>,
//     out_of_sync_index: usize,
// }

// impl<K, T, R> Cursors<K, R> 
// where
//     K: ChIdOp,
//     T: Clone + GetSeq,
//     // M: MpscOp<Event<K, T>>,
//     R: ReadNext<Output = ReadQueOutput<T>>,
// {
//     pub fn new() -> Self {

//         Self { 
//             in_syncs: VecMap::new(),
//             out_of_syncs: VecMap::new(),
//             out_of_sync_index: 0,
//         }
//     }

//     pub fn len(&self) -> usize {
//         self.in_syncs.len() + self.out_of_syncs.len()
//     }

//     pub(super) fn insert(&mut self, ch_id: K, cursor: Cursor<R>) -> Result<()> {

//         if self.exist_channel(&ch_id) {
//             bail!("already subscribed channel [{:?}]", ch_id)
//         }

//         // let ch = ch.clone();
//         // let cursor = Cursor { seq, ch: ch.clone() };
//         // cursor.ch.insert_suber(self);
//         self.out_of_syncs.insert(ch_id, cursor);

//         Ok(())
//     }

//     pub fn remove(&mut self, ch_id: &K) -> Option<Cursor<R>> { 
//         // remove_channel(&mut self.active_cursors, ch_id, self.id());
//         let r = self.in_syncs.remove(ch_id); 
//         if r.is_some() {
//             r
//         } else {
//             self.out_of_syncs.remove(ch_id)
//             // remove_channel(&mut self.pending_cursors, ch_id, self.id())
//         }
//     }

//     pub fn channels(&self) -> usize {
//         self.in_syncs.len() + self.out_of_syncs.len()
//     }

//     pub fn exist_channel(&self, key: &K) -> bool {
//         let r = self.in_syncs.get(key);
//         if r.is_some() {
//             true
//         } else {
//             self.out_of_syncs.get(key).is_some()
//         }
//     }

//     // pub fn into_iter(self) -> (impl Iterator<Item = (K, Cursor<R>)>, impl Iterator<Item = (K, Cursor<R>)>,) {
//     //     (self.active_cursors.into_iter(), self.pending_cursors.into_iter())
//     // }

//     pub fn in_syncs_iter(&self) -> impl Iterator<Item = (&K, &Cursor<R>)> {
//         self.in_syncs.iter()
//     }

//     pub fn out_of_syncs_iter(&self) -> impl Iterator<Item = (&K, &Cursor<R>)> {
//         self.out_of_syncs.iter()
//     }

//     // fn check_active_msg(&mut self, ch_id: &K, seq: u64) -> Option<&mut Cursor<R>> {
//     //     let out_of_sync = {
//     //         let r = self.active_cursors.get_full_mut(ch_id);
//     //         let out_of_sync = if let Some((index, k, cursor)) = &r {
//     //             if seq == cursor.seq {
//     //                 return r; // Some(cursor);
//     //             } else if seq > cursor.seq { 
//     //                 true
//     //             }  else {
//     //                 // ignore
//     //                 false
//     //             }
//     //         } else {
//     //             false
//     //         };
//     //         out_of_sync
//     //     };

//     //     if out_of_sync {
//     //         let r = self.active_cursors.remove(ch_id);
//     //         if let Some(cursor) = r {
//     //             self.pending_cursors.insert(ch_id.clone(), cursor);
//     //         }
//     //     }

//     //     None
//     // }

//     fn check_in_sync_msg(&mut self, ch_id: &K, seq: u64) -> bool {
//         let r = self.in_syncs.get_full_mut(ch_id);
//         if let Some((index, k, cursor)) = r {
//             if seq == cursor.seq { 
//                 cursor.on_output_value(seq);
//                 return true
//             } else if seq > cursor.seq { 
//                 // out of sync
//                 let r = self.in_syncs.swap_remove_index(index) ;
//                 if let Some((ch_id, cursor)) = r {
//                     self.out_of_syncs.insert(ch_id, cursor);
//                 }
//             } else {
//                 // ignore
//             }
//         } 
//         false
//     }

//     fn is_in_sync_empty(&self) -> bool {
//         self.out_of_syncs.len() == 0 
//     }


//     fn read_in_sync(&mut self) -> (RecvOutput<K, T>, bool){ 
//         let mut is_first_sync = false;

//         while self.out_of_syncs.len() > 0 { 
//             let r = self.read_in_sync_at(self.out_of_sync_index, &mut is_first_sync);
            
//             self.inc_out_of_sync_index();

//             if !r.is_none() {
//                 return (r, is_first_sync);
//             } 
//         }
//         (RecvOutput::None, is_first_sync)
//     }

//     fn read_in_sync_at(&mut self, index: usize, is_first_sync: &mut bool) -> RecvOutput<K, T> {
//         let r = self.out_of_syncs.get_index_mut(index);
//         if let Some((ch_id, cursor)) = r {
//             let r = cursor.read_next(ch_id);
//             if r.is_none() {
//                 let is_first = self.move_outofsync_to_insync(index);
//                 if is_first {
//                     *is_first_sync = true;
//                 }
//             }
//             return r;
//         } 
//         RecvOutput::None
//     }

//     fn inc_out_of_sync_index(&mut self) {
//         self.out_of_sync_index += 1;
//         if self.out_of_sync_index >= self.out_of_syncs.len() {
//             self.out_of_sync_index = 0;
//         }
//     }

//     fn move_all_insyncs_to_outofsyncs(&mut self) {
//         while let Some((ch_id, cursor)) = self.in_syncs.pop() {
//             self.out_of_syncs.insert(ch_id, cursor);
//         }
//     }

//     fn move_outofsync_to_insync(&mut self, index: usize) -> bool {
//         let r = self.out_of_syncs.swap_remove_index(index);
//         if let Some((ch_id, cursor)) = r {
//             // if self.active_cursors.len() == 0 { 
//             //     self.rx.clear(); // 避免反复 overflowed
//             // }
//             self.in_syncs.insert(ch_id, cursor);
//             self.in_syncs.len() == 1
//         } else {
//             false
//         }
//     }
// }



pub struct Suber<K, T, M> 
where
    K: ChIdOp,
    T: Clone+GetSeq,
    M: MpscOp<Event<K, T>>,
{
    cursors: Cursors<K, Channel<K, T, M>>,
    watcher: Watcher<K, T, M>,
}

impl<K, T, M> Suber<K, T, M> 
where
    K: ChIdOp,
    T: Clone+GetSeq,
    M: MpscOp<Event<K, T>>,
{
    pub fn with_inbox_cap(cap: usize) -> Self {
        // let (tx, rx) = M::channel(cap);

        Self { 
            cursors: Cursors::new(),
            watcher: Watcher::new(cap),
        }
    }

    pub fn id(&self) -> &SuberId {
        &self.watcher.id()
    }

    // pub(super) fn tx(&self) -> M::Sender {
    //     self.tx.clone()
    // }

    pub(crate) fn subscribe(&mut self, ch: &Channel<K, T, M>, seq: u64) -> Result<()> {

        if self.exist_channel(ch.ch_id()) {
            bail!("already subscribed channel [{:?}]", ch.ch_id())
        }

        
        ch.insert_suber(self);
        let cursor = Cursor { seq, ch: ch.clone() };
        // cursor.ch.insert_suber(self);
        // self.pending_cursors.insert(cursor.ch.ch_id().clone(), cursor);
        self.cursors.insert(ch.ch_id().clone(), cursor)?;

        Ok(())
    }

    pub fn unsubscribe(&mut self, ch_id: &K) -> Option<Channel<K, T, M>> { 
        let r = self.cursors.remove(ch_id);
        if let Some(cursor) = r {
            cursor.ch.remove_suber(self.id());
            Some(cursor.ch)
        } else {
            None
        }
        // let r = remove_channel(&mut self.active_cursors, ch_id, self.id());
        // if r.is_some() {
        //     r
        // } else {
        //     remove_channel(&mut self.pending_cursors, ch_id, self.id())
        // }
    }

    pub fn channels(&self) -> usize {
        self.cursors.len()
    }

    pub(super) fn watcher(&self) -> &Watcher<K, T, M>{
        &self.watcher
    }

    fn exist_channel(&self, key: &K) -> bool {
        self.cursors.exist_channel(key)
        // let r = self.active_cursors.get(key);
        // if r.is_some() {
        //     true
        // } else {
        //     self.pending_cursors.get(key).is_some()
        // }
    }
}

// fn remove_channel<K, T, M, R>(cursors: &mut VecMap<K, Cursor<R>>, ch_id: &K, sub_id: &SuberId) -> Option<Cursor<R>> 
// where
//     K: ChIdOp,
//     T: Clone,
//     M: MpscOp<Event<K, T>>,
// {
//     let r = cursors.remove(ch_id).map(|x|x.ch);
//     if let Some(ch) = &r {
//         ch.remove_suber(sub_id);
//     }
//     r
// }


impl<K, T, M> Drop for Suber<K, T, M> 
where
    K: ChIdOp,
    T: Clone+GetSeq,
    M: MpscOp<Event<K, T>>,
{
    fn drop(&mut self) { 
        for (_id, cursor) in self.cursors.in_syncs_iter() {
            cursor.ch.remove_suber(self.id());
        }

        for (_id, cursor) in self.cursors.out_of_syncs_iter() {
            cursor.ch.remove_suber(self.id());
        }
    }
}


impl<K, T, M> Suber<K, T, M> 
where
    K: ChIdOp,
    T: Clone + GetSeq,
    M: MpscOp<Event<K, T>>,
{
    pub async fn async_recv(&mut self) -> Result<T> {
        todo!()
    }

    pub fn try_recv(&mut self) -> RecvOutput<K, T> { 

        loop { 
            let r = self.read_in_sync();
            if !r.is_none() {
                return r;
            } 

            let r = self.try_recv_in_sync();
                
            if !r.is_none() {
                return r;
            }

            if self.cursors.is_in_sync_empty() {
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

            let r = self.watcher.rx().async_recv().await;
            match r {
                Ok(mail) => {
                    let r = self.process_recved(mail);
                    if !r.is_none() {
                        return r;
                    }
                },
                Err(_e) => {
                    // lagged
                    self.cursors.move_all_insyncs_to_outofsyncs();
                },
            }
        }
    }

    fn try_recv_in_sync(&mut self) -> RecvOutput<K, T> { 
        loop {
            let r = self.watcher.rx().try_recv();
            match r {
                Ok(mail) => {
                    let r = self.process_recved(mail);
                    if !r.is_none() {
                        return r;
                    }
                },
                Err(e) => {
                    match e {
                        TryRecvError::Overflowed => {
                            self.cursors.move_all_insyncs_to_outofsyncs();
                        },
                        TryRecvError::Empty 
                        // | TryRecvError::Closed // never recv Closed
                        => {}, 
                    }
                    return RecvOutput::None;
                },
            }
        }
    }

    fn process_recved(&mut self, event: Event<K, T>) -> RecvOutput<K, T>{ 
        match event {
            Event::Msg(msg) => self.process_msg(msg),
        }
    }

    fn process_msg(&mut self, mail: Msg<K, T>) -> RecvOutput<K, T>{ 
        let r = self.cursors.check_in_sync_msg(&mail.ch_id, mail.msg.get_seq());
        if r {
            RecvOutput::Value(mail.ch_id, mail.msg)
        } else {
            RecvOutput::None
        }
        // match r {
        //     Some(cursor) => cursor.output_value(mail.ch_id.clone(), mail.msg),
        //     None => RecvOutput::None,
        // }

        // let r = self.active_cursors.get_mut(&mail.ch_id);
        // if let Some(cursor) = r {
        //     if mail.msg.get_seq() == cursor.seq {
        //         return cursor.output_value(mail.msg);

        //     } else if mail.msg.get_seq() > cursor.seq { 
        //         let r = self.active_cursors.remove(&mail.ch_id);
        //         if let Some(cursor) = r {
        //             self.pending_cursors.insert(cursor.ch.ch_id().clone(), cursor);
        //         }
        //     } 
        //     // ignore
        // }
        // RecvOutput::None
    }

    fn read_in_sync(&mut self) -> RecvOutput<K, T> {
        let (r, is_first_sync) = self.cursors.read_in_sync();
        if is_first_sync { 
            // 第一个 sync cursor，清理掉收件箱，避免反复 overflowed
            self.watcher.rx().clear();
        }
        r
    }

    // fn is_pending_empty(&self) -> bool {
    //     self.pending_cursors.len() == 0 
    // }

    // fn read_pending(&mut self) -> RecvOutput<K, T> {
    //     while self.pending_cursors.len() > 0 { 
    //         let r = self.read_pending_at(self.last_pending_index);
            
    //         self.inc_last_pending_index();

    //         if !r.is_none() {
    //             return r;
    //         } 
    //     }
    //     RecvOutput::None
    // }

    // fn read_pending_at(&mut self, index: usize) -> RecvOutput<K, T> {
    //     let r = self.pending_cursors.get_index_mut(index);
    //     if let Some((_ch_id, cursor)) = r {
    //         let r = cursor.read_next();
    //         if r.is_none() {
    //             self.move_pending_to_active(index);
    //         }
    //         return r;
    //     } 
    //     RecvOutput::None
    // }

    // fn inc_last_pending_index(&mut self) {
    //     self.last_pending_index += 1;
    //     if self.last_pending_index >= self.pending_cursors.len() {
    //         self.last_pending_index = 0;
    //     }
    // }

    // fn move_all_actives_to_pendings(&mut self) {
    //     while let Some((_ch_id, cursor)) = self.active_cursors.pop() {
    //         self.pending_cursors.insert(cursor.ch.ch_id().clone(), cursor);
    //     }
    // }

    // fn move_pending_to_active(&mut self, index: usize) {
    //     let r = self.pending_cursors.swap_remove_index(index);
    //     if let Some((_ch_id, cursor)) = r {
    //         if self.active_cursors.len() == 0 { 
    //             self.rx.clear(); // 避免反复 overflowed
    //         }
    //         self.active_cursors.insert(cursor.ch.ch_id().clone(), cursor);
    //     }
    // }

}

// pub struct Puber<K, T, M> 
// where
//     K: ChIdOp,
//     T: Clone,
//     M: MpscOp<Event<K, T>>,
// {
//     ch_shared: Arc<ChShared<K, T, M>>,
// }

// impl<K, T, M> Puber<K, T, M> 
// where
//     K: ChIdOp,
//     T: Clone + GetSeq,
//     M: MpscOp<Event<K, T>>,
// {
//     pub fn push_raw(&self, v: T) -> Result<()> {
//         {
//             let mut queue = self.ch_shared.queue.write();
//             queue.push_raw(v.clone(), self.ch_shared.capacity)?;
//         }

//         self.broadcast_to_subers(v);

//         Ok(())
//     }

//     fn broadcast_to_subers(&self, v: T) {
//         let ch_id = &self.ch_shared.ch_id;
//         let mut subers = self.ch_shared.subers.lock();
//         for (_k, tx) in subers.deref_mut() { 
//             let _r = tx.try_send(Event::msg(ch_id.clone(), v.clone()));
//         }
//     }
// }

// impl<K, T, M> Puber<K, T, M> 
// where
//     K: ChIdOp,
//     T: Clone + GetSeq + WithSeq,
//     M: MpscOp<Event<K, T>>,
// {
//     pub fn push(&self, v: T::Value) -> Result<()> {
//         let v = {
//             let mut queue = self.ch_shared.queue.write();
//             let v = T::with_seq(queue.next_seq(), v);   
//             queue.push_raw(v.clone(), self.ch_shared.capacity)?;
//             v
//         };

//         self.broadcast_to_subers(v);

//         Ok(())
//     }
// }

// pub struct Mail<K, T> {
//     ch_id: K,
//     msg: T,
// }

// impl<K, T> Mail<K, T> {
//     fn new(ch_id: K, msg: T) -> Self {
//         Self { ch_id, msg }
//     }
// }

// impl<K, T> Clone for Mail<K, T> 
// where
//     K: Clone,
//     T: Clone,
// {
//     fn clone(&self) -> Self {
//         Self { ch_id: self.ch_id.clone(), msg: self.msg.clone() }
//     }
// }


struct ChShared<K, T, M> 
where
    K: ChIdOp,
    T: Clone,
    M: MpscOp<Event<K, T>>,
{
    capacity: usize,
    ch_id: K,
    subers: Mutex<VecMap<SuberId,  M::Sender>>,
    queue: RwLock<ChDeque<T>>,
}


// struct Cursor<K, T, M> 
// where
//     K: ChIdOp,
//     T: Clone,
//     M: MpscOp<Mail<K, T>>,
// {
//     seq: u64,
//     ch: Channel<K, T, M>, // Arc<ChShared<K, T>>,
// }

// impl<K, T, M> Cursor<K, T, M>  
// where
//     K: ChIdOp,
//     T: Clone + GetSeq,
//     M: MpscOp<Mail<K, T>>,
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



