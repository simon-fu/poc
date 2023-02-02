


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

    pub fn is_lagged(&self) -> bool {
        if let Self::Lagged(_v) = self {
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



