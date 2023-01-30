use std::{time::{Duration, Instant}, sync::Arc};
use anyhow::Result;
use tokio::time::sleep;
use crate::{
    impl02::{
        mpsc_defs::{MpscOp, SenderOp, RecvOp},
        mpsc_async_broadcast, 
        mpsc_tokio_mpsc, 
        mpsc_async_channel, 
        mpsc_tokio_broadcast,
        mpsc_crossbeam_que,
        mpsc_kanal,
    } 
};

#[derive(Debug)]
struct BenchArgs {
    ch_len: usize,
    ch_num: usize,
    msg_num: usize,
}

pub async fn run() -> Result<()> { 
    // warm up
    bench_1_to_n_round(&BenchArgs{ ch_len: 128, ch_num: 1_000_000 , msg_num: 1 }).await?;

    bench_1_to_n_round(&BenchArgs{ ch_len: 128, ch_num: 1_000_000 , msg_num: 1 }).await?;
    bench_1_to_n_round(&BenchArgs{ ch_len: 128, ch_num: 1_000_000 , msg_num: 2 }).await?;
    bench_1_to_n_round(&BenchArgs{ ch_len: 128, ch_num: 1_000_000 , msg_num: 10 }).await?;
    bench_1_to_n_round(&BenchArgs{ ch_len: 128, ch_num: 1_000_000 , msg_num: 100 }).await?;

    bench_1_to_n_sendonly_round(&BenchArgs{ ch_len: 128, ch_num: 1_000_000 , msg_num: 1 }).await?;
    bench_1_to_n_sendonly_round(&BenchArgs{ ch_len: 128, ch_num: 1_000_000 , msg_num: 2 }).await?;
    bench_1_to_n_sendonly_round(&BenchArgs{ ch_len: 128, ch_num: 1_000_000 , msg_num: 10 }).await?;
    bench_1_to_n_sendonly_round(&BenchArgs{ ch_len: 128, ch_num: 1_000_000 , msg_num: 100 }).await?;

    Ok(())
}

async fn bench_1_to_n_round(args: &BenchArgs) -> Result<()> { 
    println!("1_to_n round: {:?}", args);
    bench_1_to_n::<mpsc_async_broadcast::Mpsc>(args).await?;
    bench_1_to_n::<mpsc_async_channel::Mpsc>(args).await?;
    bench_1_to_n::<mpsc_tokio_mpsc::Mpsc>(args).await?;
    bench_1_to_n::<mpsc_tokio_broadcast::Mpsc>(args).await?;
    bench_1_to_n::<mpsc_crossbeam_que::Mpsc>(args).await?;
    bench_1_to_n::<mpsc_kanal::Mpsc>(args).await?;
    println!();
    Ok(())
}

type Value = (u64, usize, Arc<u64>);

async fn bench_1_to_n<M>(args: &BenchArgs) -> Result<()> 
where
    M: MpscOp<Value>,
    <M as MpscOp<Value>>::Sender: std::marker::Send + 'static,
    <M as MpscOp<Value>>::Receiver: std::marker::Send + 'static,
    for<'a> <<M as MpscOp<Value>>::Receiver as RecvOp<Value>>::Fut<'a>: std::marker::Send,
{ 
    let mut senders = Vec::with_capacity(args.ch_num);
    let mut tasks = Vec::with_capacity(args.ch_num);
    let msg_num = args.msg_num;

    for _i in 1..args.ch_num+1 {
        let (tx, mut rx) = M::channel(args.ch_len);
        senders.push(tx);

        let h = tokio::spawn(async move { 
            for _ in 0..msg_num {
                rx.recv().await?;
            }
            Result::<()>::Ok(())
        });
        tasks.push(h);
    }
    sleep(Duration::from_millis(100)).await; // wait for all task ready

    let value = (1, 2, Arc::new(3));

    let kick_time = Instant::now();
    for tx in &mut senders { 
        for _ in 0..args.msg_num {
            let _r = tx.try_send(value.clone());
        }
    }
    let sent_elpased = kick_time.elapsed();

    for h in tasks {
        let _r = h.await??;
    }
    let recved_elpased = kick_time.elapsed();

    println!(
        "{},{},{}",
        M::name(),
        sent_elpased.as_millis(),
        recved_elpased.as_millis(),
    );

    Ok(())
}

async fn bench_1_to_n_sendonly_round(args: &BenchArgs) -> Result<()> { 
    println!("1_to_n_sendonly round: {:?}", args);
    bench_1_to_n_sendonly::<mpsc_async_broadcast::Mpsc>(args).await?;
    bench_1_to_n_sendonly::<mpsc_async_channel::Mpsc>(args).await?;
    bench_1_to_n_sendonly::<mpsc_tokio_mpsc::Mpsc>(args).await?;
    bench_1_to_n_sendonly::<mpsc_tokio_broadcast::Mpsc>(args).await?;
    bench_1_to_n_sendonly::<mpsc_crossbeam_que::Mpsc>(args).await?;
    bench_1_to_n_sendonly::<mpsc_kanal::Mpsc>(args).await?;
    println!();
    Ok(())
}

async fn bench_1_to_n_sendonly<M>(args: &BenchArgs) -> Result<()> 
where
    M: MpscOp<Value>,
    <M as MpscOp<Value>>::Sender: std::marker::Send + 'static,
    <M as MpscOp<Value>>::Receiver: std::marker::Send + 'static,
    for<'a> <<M as MpscOp<Value>>::Receiver as RecvOp<Value>>::Fut<'a>: std::marker::Send,
{ 
    let mut senders = Vec::with_capacity(args.ch_num);
    let mut recvers = Vec::with_capacity(args.ch_num);

    for _i in 1..args.ch_num+1 {
        let (tx, rx) = M::channel(args.ch_len);
        senders.push(tx);
        recvers.push(rx);
    }

    let value = (1, 2, Arc::new(3));

    let kick_time = Instant::now();
    for tx in &mut senders { 
        for _ in 0..args.msg_num {
            let _r = tx.try_send(value.clone());
        }
    }
    let sent_elpased = kick_time.elapsed();

    println!(
        "{},{}",
        M::name(),
        sent_elpased.as_millis(),
    );

    Ok(())
}

