
///
/// FuturesUnordered 由借用，使用起来很不方便，不大可行
/// 
use std::{sync::Arc, fmt};
use anyhow::{Result, bail};
use futures::{stream::FuturesUnordered, StreamExt};
use parking_lot::Mutex;
use tokio::sync::broadcast::{self, Receiver, Sender, error::RecvError};

use crate::ch_common::uid::ChId;


pub async fn run() -> Result<()> {
    let capacity = 10;
    let ch_id1 = ChId::new(1);
    let ch1 = Channel::with_capacity(ch_id1, capacity);
    let mut puber1 = ch1.puber();
    let mut suber = Suber::new();
    suber.subscribe(&ch1)?;

    puber1.push(1);
    
    {
        let mut recvers = suber.recvers();

        loop {
            let r = recvers.next().await;
            match r {
                Some(r) => {
                    match r {
                        Ok(_v) => {},
                        Err(e) => {
                            drop(recvers);
                            suber.unsubscribe(e.ch_id());
                            break;
                        },
                    }
                },
                None => break,
            }
        }
    }

    suber.unsubscribe(ch1.ch_id());
    println!("run done");
    Ok(())
}



#[derive(Clone)]
pub struct Channel<T> {
    shared: Arc<ChShared<T>>,
}

impl<T> Channel<T> {
    pub fn with_capacity(ch_id: ChId, capacity: usize) -> Self {
        Self {
            shared: Arc::new(ChShared { 
                data: Default::default(),
                capacity,
                ch_id,
            }
        )}
    }

    pub fn ch_id(&self) -> ChId {
        self.shared.ch_id
    }

    pub fn puber(&self) -> Puber<T> {
        Puber { shared: self.shared.clone() }
    }
}

struct ChShared<T> {
    capacity: usize,
    ch_id: ChId,
    data: Mutex<ChData<T>>,
}

struct ChData<T> {
    senders: Vec<Sender<T>>,
}

impl<T> Default for ChData<T> {
    fn default() -> Self {
        Self { senders: Vec::new() }
    }
}

pub struct Puber<T> {
    shared: Arc<ChShared<T>>,
}

impl<T> Puber<T> 
where
    T: Clone,
{
    pub fn push(&mut self, v: T) {
        let mut data = self.shared.data.lock();
        for tx in &mut data.senders {
            let _r = tx.send(v.clone());
        }
    }
}

pub struct Suber<T> {
    recvers: Vec<Recver<T>>,
}

impl<T> Suber<T> 
where
    T: Clone
{
    pub fn new() -> Self {
        Self { 
            recvers: Vec::new(),
        }
    }

    pub fn subscribe(&mut self, ch: &Channel<T>) -> Result<()> {

        let r = self.recvers.iter().position(|x| x.ch.ch_id() == ch.ch_id());
        if let Some(index) = r {
            bail!("already subscribed ch_id [{}] at [{}]", ch.shared.ch_id, index)
        }

        let mut data = ch.shared.data.lock();
        let (tx, rx) = broadcast::channel(ch.shared.capacity);
        self.recvers.push(Recver::new(rx, ch.clone()));
        data.senders.push(tx);
        Ok(())
    }

    pub fn unsubscribe(&mut self, ch_id: ChId) -> bool { 
        let r = self.recvers.iter().position(|x| x.ch.ch_id() == ch_id);
        if let Some(index) = r {
            self.recvers.swap_remove(index);
            true
        } else {
            false
        }
    }

    pub fn recvers(&mut self) -> impl StreamExt<Item = Result<T, NextError>> + '_ {
        let futs = FuturesUnordered::new();
        for recver in &mut self.recvers {
            futs.push(recver.recv());
        }
        futs
    }
}

struct Recver<T> {
    ch: Channel<T>,
    rx: Receiver<T>,
}

impl<T> Recver<T> 
where
    T: Clone
{
    pub fn new(rx: Receiver<T>, ch: Channel<T>) -> Self {
        Self { ch, rx }
    }

    pub async fn recv(&mut self) -> Result<T, NextError> {
        self.rx.recv().await.map_err(|e|NextError {
            ch_id: self.ch.ch_id(),
            error: e,
        })
    }
}


#[derive(Debug, PartialEq, Clone)]
pub struct NextError {
    pub(crate) ch_id: ChId,
    pub(crate) error: RecvError,
}

impl NextError {
    pub fn ch_id(&self) -> ChId {
        self.ch_id
    }

    pub fn error(&self) -> &RecvError {
        &self.error
    }
}

impl fmt::Display for NextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, ch_id {}", self.error, self.ch_id)
    }
}

impl std::error::Error for NextError {}

