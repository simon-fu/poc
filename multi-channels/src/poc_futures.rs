/// refer from https://stackoverflow.com/questions/70774671/tokioselect-but-for-a-vec-of-futures
/// 
use futures::{stream::FuturesUnordered, StreamExt};
use std::time::Duration;
use tokio::time::{sleep, Instant};
use anyhow::Result;

pub async fn run() -> Result<()>{
    let mut futures = FuturesUnordered::new();
    futures.push(wait(500));
    futures.push(wait(300));
    futures.push(wait(100));
    futures.push(wait(200));
    
    let start_time = Instant::now();

    let mut num_added = 0;
    while let Some(wait_time) = futures.next().await {
        println!("Waited {}ms", wait_time);
        if num_added < 3 {
            num_added += 1;
            futures.push(wait(200));
        }
    }
    
    println!("Completed all work in {}ms", start_time.elapsed().as_millis());
    Ok(())
}

async fn wait(millis: u64) -> u64 {
    sleep(Duration::from_millis(millis)).await;
    millis
}
