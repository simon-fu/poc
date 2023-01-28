
use std::collections::VecDeque;
use anyhow::{Result, bail};

use super::*;

pub struct ChDeque<T> { 
    queue: VecDeque<T>,
    last_seq: u64,
}

impl<T> ChDeque<T> {
    pub fn new() -> Self {
        Self { queue: VecDeque::new(), last_seq: 0 }
    }

    pub fn next_seq(&self) -> u64 {
        self.last_seq + 1
    }
}

// impl<T> ChDeque<T>
// where
//     T: Clone + GetSeq + WithSeq,
// {
//     pub fn push(&mut self, v: T::Value, capacity: usize) -> Result<()> { 
//         let v = T::with_seq(self.next_seq(), v);   
//         self.push_raw(v, capacity)
//     }
// }

impl<T> ChDeque<T> 
where
    T: GetSeq + Clone
{
    pub fn push_raw(&mut self, v: T, capacity: usize) -> Result<()> { 
        // if let Some(last) = self.queue.back() {
        //     if v.get_seq() <= last.get_seq() {
        //         bail!("push seq inconsist, expect [{}] but [{}]", last.get_seq()+1, v.get_seq())
        //     }
        // }

        if v.get_seq() <= self.last_seq {
            bail!("push seq inconsist, expect [{}] but [{}]", self.last_seq+1, v.get_seq())
        }

        while self.queue.len() >= capacity {
            self.queue.pop_front();
        }

        self.last_seq = v.get_seq();
        self.queue.push_back(v);

        Ok(())
    }

    pub fn read_next(&self, seq: u64) -> ReadQueOutput<T> {
        if let Some(first) = self.queue.front() {
            let start_seq = first.get_seq() ;
            if seq < start_seq {
                ReadQueOutput::Lagged
            } else {
                let delta = (seq - start_seq).min(usize::MAX as u64);
                self.reverse_search_from(seq, delta as usize)
            }
        } else {
            ReadQueOutput::Latest
        }
    }

    fn reverse_search_from(&self, seq: u64, index0: usize) -> ReadQueOutput<T> { 
        if let Some(last) = self.queue.back() {
            if seq > last.get_seq() {
                return ReadQueOutput::Latest;
            }
        }

        let mut index = index0.min(self.queue.len() - 1);
        while index > 0 {
            if seq == self.queue[index].get_seq() 
            || seq > self.queue[index-1].get_seq() {
                return ReadQueOutput::Value(self.queue[index].clone())
            } 
            index -= 1;
        }

        if seq == self.queue[index].get_seq() {
            return ReadQueOutput::Value(self.queue[index].clone())
        }

        ReadQueOutput::Lagged
    }
}

