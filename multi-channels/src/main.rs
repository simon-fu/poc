///
/// 实现思路：
/// - 注册 waker 到 channel
/// - channel 的 wakers 是一个 Vec
/// - waker 里的 ready_channels 是一个 Vec
/// - channel 有数据时，取出所有 wakers，一个个 把自己添加到 ready_channels里
/// 
use anyhow::{Result, bail};

mod poc_futures;
mod poc_atomic_waker;
mod poc_async_broadcast;

mod impl_test;
pub mod impl01;
pub mod impl02;
pub mod impl51;
mod ch_common;

#[tokio::main]
async fn main() -> Result<()> {
    let rtype = 0;
    match rtype {
        0 => impl_test::run().await,
        51 => impl51::run().await,
        81 => poc_futures::run().await,
        82 => poc_async_broadcast::run().await,
        _ => bail!("unknown rtype [{}]", rtype)
    }
}
