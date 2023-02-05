
#[derive(Clone)]
pub enum Event<K, T> {
    Msg(Msg<K, T>),
}

impl<K, T> Event<K, T> {
    pub(super) fn msg(ch_id: K, msg: T) -> Self {
        Self::Msg(Msg{ch_id, msg})
    }
}

pub struct Msg<K, T> {
    pub(super) ch_id: K,
    pub(super) msg: T,
}

impl<K, T> Msg<K, T> {
    pub(super) fn new(ch_id: K, msg: T) -> Self {
        Self { ch_id, msg }
    }
}

impl<K, T> Clone for Msg<K, T> 
where
    K: Clone,
    T: Clone,
{
    fn clone(&self) -> Self {
        Self { ch_id: self.ch_id.clone(), msg: self.msg.clone() }
    }
}


