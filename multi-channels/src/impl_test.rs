///
/// 修改 use implxx 测试各种实现
/// 

use anyhow::{Result, Context};
use crate::{
    ch_common::{uid::ChId, SeqVal, RecvOutput, vec_remove}, 
    impl02::{SChannel, Suber, impl_name} 
};

pub async fn run() -> Result<()> { 
    const MAX_RUNS: usize = 16;
    for n in 1..=MAX_RUNS {
        do_run().await.with_context(||format!("fail at NO.{} when testing [{}]", n, impl_name()))?;
    }
    println!("test Ok times [{}] for [{}]", MAX_RUNS, impl_name());
    Ok(())
}

async fn do_run() -> Result<()> {
    let capacity = 10;
    
    let ch_id1: ChId = 1.into();
    let ch_id2: ChId = 2.into();

    let ch1 = SChannel::with_capacity(ch_id1, capacity);
    let mut puber1 = ch1.puber();

    let ch2 = SChannel::with_capacity(ch_id2, capacity);
    let mut puber2 = ch2.puber();

    let mut suber = Suber::new();

    suber.subscribe(&ch1, 1)?;
    assert_eq!(suber.channels(), 1);
    assert_eq!(ch1.subers(), 1);
    assert_eq!(ch2.subers(), 0);

    let r = suber.subscribe(&ch1, 1);
    assert!(r.is_err());

    suber.subscribe(&ch2, 1)?;
    assert_eq!(suber.channels(), 2);
    assert_eq!(ch1.subers(), 1);
    assert_eq!(ch2.subers(), 1);
    
    puber1.push(1)?;
    puber2.push(2)?;

    let mut expect = vec![
        RecvOutput::Value(ch_id1, SeqVal(1, 1)),
        RecvOutput::Value(ch_id2, SeqVal(1, 2)),
    ];


    let r = suber.try_recv();
    vec_remove(&mut expect, &r).unwrap();

    let r = suber.try_recv();
    vec_remove(&mut expect, &r).unwrap();

    assert_eq!(expect.len(), 0);
    assert_eq!(suber.try_recv(), RecvOutput::None);

    suber.unsubscribe(ch1.ch_id());
    assert_eq!(suber.channels(), 1);
    assert_eq!(ch1.subers(), 0);
    assert_eq!(ch2.subers(), 1);

    puber1.push(3)?;
    puber2.push(4)?;

    assert_eq!(suber.try_recv(), RecvOutput::Value(ch_id2, SeqVal(2, 4)),);
    assert_eq!(suber.try_recv(), RecvOutput::None);

    drop(suber);
    assert_eq!(ch1.subers(), 0);
    assert_eq!(ch2.subers(), 0);

    Ok(())
}