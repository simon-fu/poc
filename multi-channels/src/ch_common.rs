use std::collections::VecDeque;

use anyhow::{Result, Context, bail};


// 第一种 VecSet 实现方式
pub type VecSet<T> = indexmap::IndexSet<T>;
pub type VecMap<K, T> = indexmap::IndexMap<K, T>;


// 第二种 VecSet 实现方式
// refer from https://stackoverflow.com/questions/53755017/can-i-randomly-sample-from-a-hashset-efficiently



// use std::collections::HashSet;

// #[derive(Default)]
// pub struct VecSet<T> {
//     // refer from https://stackoverflow.com/questions/53755017/can-i-randomly-sample-from-a-hashset-efficiently
//     set: HashSet<T>,
//     vec: Vec<T>,
// }

// impl<T> VecSet<T>
// where
//     T: Clone + Eq + std::hash::Hash,
// {
//     pub fn new() -> Self {
//         Self {
//             set: HashSet::new(),
//             vec: Vec::new(),
//         }
//     }

//     pub fn is_empty(&self) -> bool {
//         assert_eq!(self.set.len(), self.vec.len());
//         self.vec.is_empty()
//     }

//     pub fn len(&self) -> usize {
//         self.vec.len()
//     }

//     pub fn insert(&mut self, v: T) -> bool {
//         assert_eq!(self.set.len(), self.vec.len());
//         let was_new = self.set.insert(v.clone());
//         if was_new {
//             self.vec.push(v);
//         }
//         was_new
//     }

//     pub fn remove(&mut self, v: &T) -> bool {
//         let removed = self.set.remove(v);
//         if removed {
//             self.vec.retain(|x| x!=v )
//         }
//         removed
//     }

//     pub fn get_index(&self, index: usize) -> Option<&T> {
//         self.vec.get(index)
//     }

//     // pub fn merge_set(&mut self, set: &mut HashSet<T>) {
//     //     for v in set.drain() {
//     //         self.insert(v);
//     //     }
//     // }

//     // fn remove_random(&mut self) -> T {
//     //     assert_eq!(self.set.len(), self.vec.len());
//     //     let index = thread_rng().gen_range(0, self.vec.len());
//     //     let elem = self.vec.swap_remove(index);
//     //     let was_present = self.set.remove(&elem);
//     //     assert!(was_present);
//     //     elem
//     // }
// }



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
                println!("aaa lagged 222");
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

        println!("aaa lagged 111, seq {}, index0 {}, index {}, qlen {}", seq, index0, index, self.queue.len());
        ReadQueOutput::Lagged
    }
}

pub enum ReadQueOutput<T> {
    Value(T),
    Latest,
    Lagged,
}

pub enum RecvOutput<K, T> {
    Value(K, T),
    Lagged(K),
    None,
}

impl<K, T> RecvOutput<K, T> {
    pub fn is_none(&self) -> bool {
        if let Self::None = self {
            true
        } else {
            false
        }
    }
}

impl <K, T> PartialEq for RecvOutput<K, T> 
where
    K: PartialEq,
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Value(l0, l1), Self::Value(r0, r1)) => l0 == r0 && l1 == r1,
            (Self::Lagged(l0), Self::Lagged(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl <K, T> Eq for RecvOutput<K, T> 
where
    K: Eq,
    T: Eq,
{ }

impl <K, T> std::fmt::Debug for RecvOutput<K, T> 
where
    K: std::fmt::Debug,
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Value(arg0, arg1) => f.debug_tuple("Value").field(arg0).field(arg1).finish(),
            Self::Lagged(arg0) => f.debug_tuple("Lagged").field(arg0).finish(),
            Self::None => write!(f, "None"),
        }
    }
}

pub trait ChIdOp: Clone + Eq + std::hash::Hash + std::fmt::Debug {}

impl<T> ChIdOp for T 
where 
    T: Clone + Eq + std::hash::Hash + std::fmt::Debug
{ }

pub trait GetSeq {
    fn get_seq(&self) -> u64;
}

pub trait WithSeq {
    type Value;
    fn with_seq(seq: u64, v: Self::Value) -> Self;
}

/// Seq Value
#[derive(Eq)]
pub struct SeqVal<V>(pub(crate)u64, pub(crate)V);

// impl <V> SeqVal<V> {
//     pub fn new(seq: u64, v: V) -> Self {
//         Self(seq, v)
//     }
// }

impl<V> WithSeq for SeqVal<V> {
    type Value = V;
    fn with_seq(seq: u64, v: Self::Value) -> Self {
        Self(seq, v)
    }
}

impl<V> GetSeq for SeqVal<V> {
    fn get_seq(&self) -> u64 {
        self.0
    }
}

impl<V> Clone for SeqVal<V> 
where
    V: Clone,
{
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<V> PartialEq for SeqVal<V> 
where
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl <V> std::fmt::Debug for SeqVal<V>  
where
    V: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SeqVal").field(&self.0).field(&self.1).finish()
    }
}

pub fn vec_remove<T>(vec: &mut Vec<T>, val: &T) -> Result<T> 
where
    T: Eq+std::fmt::Debug,
{
    let index = vec.iter().position(|x|*x == *val)
    .with_context(||format!("Not found [{:?}]", val))?;

    Ok(vec.swap_remove(index))
}


#[macro_export]
macro_rules! define_arc_hash {
    ( $id1:ty, $id2:ty) => {
        // struct $id2(Arc<$id1>);

        impl<K> std::hash::Hash for $id1 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                let ptr = self.0.deref() as *const $id2;
                ptr.hash(state);
            }
        }
        
        impl<K> PartialEq for $id1 {
            fn eq(&self, other: &Self) -> bool {
                let ptr1 = self.0.deref() as *const $id2;
                let ptr2 = other.0.deref() as *const $id2;
                ptr1 == ptr2
            }
        }
        
        impl<K> Eq for $id1 {}

        impl<K> std::hash::Hash for $id2 {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                let ptr = self as *const $id2;
                ptr.hash(state);
            }
        }
        
        impl<K> indexmap::Equivalent<$id1> for $id2 {
            fn equivalent(&self, other: &$id1) -> bool {
                let ptr1 = self as *const $id2;
                let ptr2 = other.0.deref() as *const $id2;
                ptr1 == ptr2
            }
        }
    }
}

pub(super) mod uid {
    use std::ops::Deref;

    #[macro_export]
    macro_rules! define_immutable_id {
        ($id1:ident) => {
            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy, Hash, Default)]
            // #[derive(Serialize, Deserialize)]
            pub struct $id1(u64);
            
            impl $id1 {
                #[inline]
                pub fn new(id: u64) -> Self {
                    Self(id)
                }
    
                #[inline]
                pub fn to(&self) -> u64 {
                    self.0
                }
            }
    
            impl Deref for $id1 {
                type Target = u64;
                #[inline]
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }
    
            impl From<$id1> for u64 {
                #[inline]
                fn from(id: $id1) -> Self {
                    id.to()
                    // *id.deref()
                }
            }
            
            impl From<u64> for $id1 {
                #[inline]
                fn from(id: u64) -> Self {
                    $id1(id)
                }
            }
    
            impl std::ops::Add<u64> for $id1 {
                type Output = Self;
            
                #[inline]
                fn add(self, rhs: u64) -> Self::Output {
                    Self(self.0 + rhs)
                }
            }
            
            impl std::ops::Sub for $id1 {
                type Output = u64;
            
                #[inline]
                fn sub(self, rhs: Self) -> Self::Output {
                    self.0 - rhs.0
                }
            }
    
            impl std::fmt::Display for $id1 {
                #[inline]
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    self.0.fmt(f)
                }
            }
        }
    }
    
    #[macro_export]
    macro_rules! define_mutable_id {
        ($id1:ident) => {
            
            define_immutable_id!($id1);
            
            impl $id1 {        
                #[inline]
                pub fn next(&mut self) {
                    self.0 += 1;
                }
            
                #[inline]
                pub fn add(&mut self, n: u64) {
                    self.0 += n;
                }
            
                #[inline]
                pub fn sub(&mut self, n: u64) {
                    self.0 -= n;
                }
            }
            
            impl DerefMut for $id1 {
                #[inline]
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.0
                }
            }
    
        }
    }
    
    // pub(crate) trait To<T: Sized> {
    //     fn to(self) -> T;
    // }
    
    // #[macro_export]
    // macro_rules! define_immuable_to_id {
    //     ($id1:ident) => {
    //         define_immutable_id!($id1);
    
    //         impl To<u64> for $id1 {
    //             #[inline]
    //             fn to(self) -> u64 {
    //                 self.0
    //             }
    //         }
    
    //         impl To<Option<u64>> for Option<$id1> {
    //             #[inline]
    //             fn to(self) -> Option<u64> {
    //                 self.map(|n| n.0)
    //                 // match self {
    //                 //     Some(id) => Some(id.0),
    //                 //     None => None,
    //                 // }
    //             }
    //         }
    
    //         impl To<Option<$id1>> for Option<u64> {
    //             #[inline]
    //             fn to(self) -> Option<$id1> {
    //                 self.map(|n| $id1(n))
    //             }
    //         }
    //     }
    // }
    
    
    
    define_immutable_id!(ChId);
    
    pub(crate) fn next_ch_id() -> ChId {
        ChId(next_instance_id())
    }

    define_immutable_id!(SuberId);

    pub(crate) fn next_suber_id() -> SuberId {
        SuberId(next_instance_id())
    }
    
    /// 生成本地内存里唯一
    pub fn next_instance_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
    
        lazy_static::lazy_static! {
            static ref INST_ID: AtomicU64 = AtomicU64::new(1);
        };
        let inst_id = INST_ID.fetch_add(1, Ordering::Relaxed);
        inst_id
    }
}


