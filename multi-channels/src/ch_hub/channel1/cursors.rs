
use anyhow::{Result, bail};
use crate::ch_common::{RecvOutput, GetSeq, ReadQueOutput, ChIdOp, VecMap};
use super::channel::{Cursor, ReadNext};


pub struct Cursors<K, R> 
{
    in_syncs: VecMap<K, Cursor<R>>,
    out_of_syncs: VecMap<K, Cursor<R>>,
    out_of_sync_index: usize,
}

impl<K, R> Cursors<K, R> 
{
    pub fn new() -> Self {

        Self { 
            in_syncs: VecMap::new(),
            out_of_syncs: VecMap::new(),
            out_of_sync_index: 0,
        }
    }
}

impl<K, T, R> Cursors<K, R> 
where
    K: ChIdOp,
    T: Clone + GetSeq,
    R: ReadNext<Output = ReadQueOutput<T>>,
{
    pub fn len(&self) -> usize {
        self.in_syncs.len() + self.out_of_syncs.len()
    }

    pub(super) fn insert(&mut self, ch_id: K, cursor: Cursor<R>) -> Result<()> {

        if self.exist_channel(&ch_id) {
            bail!("already subscribed channel [{:?}]", ch_id)
        }

        // let ch = ch.clone();
        // let cursor = Cursor { seq, ch: ch.clone() };
        // cursor.ch.insert_suber(self);
        self.out_of_syncs.insert(ch_id, cursor);

        Ok(())
    }

    pub fn remove(&mut self, ch_id: &K) -> Option<Cursor<R>> { 
        // remove_channel(&mut self.active_cursors, ch_id, self.id());
        let r = self.in_syncs.remove(ch_id); 
        if r.is_some() {
            r
        } else {
            self.out_of_syncs.remove(ch_id)
            // remove_channel(&mut self.pending_cursors, ch_id, self.id())
        }
    }

    pub fn channels(&self) -> usize {
        self.in_syncs.len() + self.out_of_syncs.len()
    }

    pub fn exist_channel(&self, key: &K) -> bool {
        let r = self.in_syncs.get(key);
        if r.is_some() {
            true
        } else {
            self.out_of_syncs.get(key).is_some()
        }
    }

    // pub fn into_iter(self) -> (impl Iterator<Item = (K, Cursor<R>)>, impl Iterator<Item = (K, Cursor<R>)>,) {
    //     (self.active_cursors.into_iter(), self.pending_cursors.into_iter())
    // }

    pub fn in_syncs_iter(&self) -> impl Iterator<Item = (&K, &Cursor<R>)> {
        self.in_syncs.iter()
    }

    pub fn out_of_syncs_iter(&self) -> impl Iterator<Item = (&K, &Cursor<R>)> {
        self.out_of_syncs.iter()
    }

    // fn check_active_msg(&mut self, ch_id: &K, seq: u64) -> Option<&mut Cursor<R>> {
    //     let out_of_sync = {
    //         let r = self.active_cursors.get_full_mut(ch_id);
    //         let out_of_sync = if let Some((index, k, cursor)) = &r {
    //             if seq == cursor.seq {
    //                 return r; // Some(cursor);
    //             } else if seq > cursor.seq { 
    //                 true
    //             }  else {
    //                 // ignore
    //                 false
    //             }
    //         } else {
    //             false
    //         };
    //         out_of_sync
    //     };

    //     if out_of_sync {
    //         let r = self.active_cursors.remove(ch_id);
    //         if let Some(cursor) = r {
    //             self.pending_cursors.insert(ch_id.clone(), cursor);
    //         }
    //     }

    //     None
    // }

    pub fn check_in_sync_msg(&mut self, ch_id: &K, seq: u64) -> bool {
        let r = self.in_syncs.get_full_mut(ch_id);
        if let Some((index, k, cursor)) = r {
            if seq == cursor.seq { 
                cursor.on_output_value(seq);
                return true
            } else if seq > cursor.seq { 
                // out of sync
                let r = self.in_syncs.swap_remove_index(index) ;
                if let Some((ch_id, cursor)) = r {
                    self.out_of_syncs.insert(ch_id, cursor);
                }
            } else {
                // ignore
            }
        } 
        false
    }

    pub fn is_in_sync_empty(&self) -> bool {
        self.out_of_syncs.len() == 0 
    }


    pub fn read_in_sync(&mut self) -> (RecvOutput<K, T>, bool){ 
        let mut is_first_sync = false;

        while self.out_of_syncs.len() > 0 { 
            let r = self.read_in_sync_at(self.out_of_sync_index, &mut is_first_sync);
            
            self.inc_out_of_sync_index();

            if !r.is_none() {
                return (r, is_first_sync);
            } 
        }
        (RecvOutput::None, is_first_sync)
    }

    fn read_in_sync_at(&mut self, index: usize, is_first_sync: &mut bool) -> RecvOutput<K, T> {
        let r = self.out_of_syncs.get_index_mut(index);
        if let Some((ch_id, cursor)) = r {
            let r = cursor.read_next(ch_id);
            if r.is_none() {
                let is_first = self.move_outofsync_to_insync(index);
                if is_first {
                    *is_first_sync = true;
                }
            }
            return r;
        } 
        RecvOutput::None
    }

    fn inc_out_of_sync_index(&mut self) {
        self.out_of_sync_index += 1;
        if self.out_of_sync_index >= self.out_of_syncs.len() {
            self.out_of_sync_index = 0;
        }
    }

    pub fn move_all_insyncs_to_outofsyncs(&mut self) {
        while let Some((ch_id, cursor)) = self.in_syncs.pop() {
            self.out_of_syncs.insert(ch_id, cursor);
        }
    }

    fn move_outofsync_to_insync(&mut self, index: usize) -> bool {
        let r = self.out_of_syncs.swap_remove_index(index);
        if let Some((ch_id, cursor)) = r {
            // if self.active_cursors.len() == 0 { 
            //     self.rx.clear(); // 避免反复 overflowed
            // }
            self.in_syncs.insert(ch_id, cursor);
            self.in_syncs.len() == 1
        } else {
            false
        }
    }
}
