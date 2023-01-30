use std::{time::{Duration, Instant}, sync::Arc};
use anyhow::Result;
use console::Term;
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
    }, cli_graph::bars::{BarRow, self} 
};

#[derive(Debug)]
struct BenchArgs {
    ch_len: usize,
    ch_num: usize,
    msg_num: usize,
}

pub async fn run() -> Result<()> { 
    let mut bench = Bench {
        args: BenchArgs{ ch_len: 256, ch_num: 300_000 , msg_num: 1 },
        term: Term::stderr(),
        bars: Vec::with_capacity(16),
        is_plot: false,
    };

    // warm up
    bench.term.write_line("warming up...")?;
    bench_1_to_n_round(&mut bench.msg_num(1)).await?;
    bench.term.write_line("warming up done")?;
    bench.term.write_line("")?;

    bench.is_plot = true;

    bench_1_to_n_round(&mut bench.msg_num(1)).await?;
    bench_1_to_n_round(&mut bench.msg_num(2)).await?;
    bench_1_to_n_round(&mut bench.msg_num(10)).await?;
    bench_1_to_n_round(&mut bench.msg_num(100)).await?;
    bench_1_to_n_round(&mut bench.msg_num(200)).await?;

    bench_1_to_n_sendonly_round(&mut bench.msg_num(1)).await?;
    bench_1_to_n_sendonly_round(&mut bench.msg_num(2)).await?;
    bench_1_to_n_sendonly_round(&mut bench.msg_num(10)).await?;
    bench_1_to_n_sendonly_round(&mut bench.msg_num(100)).await?;
    bench_1_to_n_sendonly_round(&mut bench.msg_num(200)).await?;

    Ok(())
}

async fn bench_1_to_n_round(bench: &mut Bench) -> Result<()> { 
    
    bench.term.write_line(&format!("1_to_n round: {:?}", bench.args))?;

    bench_1_to_n::<mpsc_crossbeam_que::Mpsc>(bench).await?;
    bench_1_to_n::<mpsc_kanal::Mpsc>(bench).await?;
    bench_1_to_n::<mpsc_async_broadcast::Mpsc>(bench).await?;
    bench_1_to_n::<mpsc_async_channel::Mpsc>(bench).await?;
    bench_1_to_n::<mpsc_tokio_mpsc::Mpsc>(bench).await?;
    bench_1_to_n::<mpsc_tokio_broadcast::Mpsc>(bench).await?;

    bench.plot()?;

    Ok(())
}


async fn bench_1_to_n<M>(bench: &mut Bench) -> Result<()> 
where
    M: MpscOp<Message>,
    <M as MpscOp<Message>>::Sender: Send + 'static,
    <M as MpscOp<Message>>::Receiver: Send + 'static,
    for<'a> <<M as MpscOp<Message>>::Receiver as RecvOp<Message>>::Fut<'a>: Send,
{ 
    let args = &bench.args;

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

    bench.term.clear_line()?;
    bench.term.write_str(&format!(
        "{},{},{}",
        M::name(),
        sent_elpased.as_millis(),
        recved_elpased.as_millis(),
    ))?;

    bench.bars.push(BarRow { 
        label: M::name().into(), 
        count: recved_elpased.as_millis() as u64 
    });

    Ok(())
}

async fn bench_1_to_n_sendonly_round(bench: &mut Bench) -> Result<()> { 


    bench.term.write_line(&format!("1_to_n_sendonly round: {:?}", bench.args))?;

    bench_1_to_n_sendonly::<mpsc_crossbeam_que::Mpsc>(bench).await?;
    bench_1_to_n_sendonly::<mpsc_kanal::Mpsc>(bench).await?;
    bench_1_to_n_sendonly::<mpsc_async_broadcast::Mpsc>(bench).await?;
    bench_1_to_n_sendonly::<mpsc_async_channel::Mpsc>(bench).await?;
    bench_1_to_n_sendonly::<mpsc_tokio_mpsc::Mpsc>(bench).await?;
    bench_1_to_n_sendonly::<mpsc_tokio_broadcast::Mpsc>(bench).await?;
    
    bench.plot()?;

    Ok(())
}

async fn bench_1_to_n_sendonly<M>(bench: &mut Bench) -> Result<()> 
where
    M: MpscOp<Message>,
    <M as MpscOp<Message>>::Sender: Send + 'static,
    <M as MpscOp<Message>>::Receiver: Send + 'static,
    for<'a> <<M as MpscOp<Message>>::Receiver as RecvOp<Message>>::Fut<'a>: Send,
{ 
    let args = &bench.args;
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

    bench.term.clear_line()?;
    bench.term.write_str(&format!(
        "{},{}",
        M::name(),
        sent_elpased.as_millis(),
    ))?;

    bench.bars.push(BarRow { 
        label: M::name().into(), 
        count: sent_elpased.as_millis() as u64 
    });


    Ok(())
}

struct Bench {
    term: Term,
    bars: Vec<BarRow>,
    args: BenchArgs,
    is_plot: bool,
}

impl Bench {
    pub fn msg_num(&mut self, n: usize) -> &mut Self {
        self.args.msg_num = n;
        self
    }

    pub fn plot(&mut self) -> Result<()>{
        self.term.clear_line()?;
        if self.is_plot { 
            let display = bars::display_with_width(&self.bars, self.term.size().1 as usize);
            self.term.write_line(&format!("{}", display))?;
            self.term.write_line("")?;
        }
        self.bars.clear();
        Ok(())
    }
}

type Message = (u64, usize, Arc<u64>);


