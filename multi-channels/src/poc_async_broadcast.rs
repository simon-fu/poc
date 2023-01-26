use async_broadcast::{broadcast, TryRecvError};
use anyhow::Result;
use futures::stream::StreamExt;


pub async fn run() -> Result<()> {
    let (s1, mut r1) = broadcast(2);
    let s2 = s1.clone();
    let mut r2 = r1.clone();

    // Send 2 messages from two different senders.
    s1.broadcast(7).await.unwrap();
    s2.broadcast(8).await.unwrap();

    // Channel is now at capacity so sending more messages will result in an error.
    assert!(s2.try_broadcast(9).unwrap_err().is_full());
    assert!(s1.try_broadcast(10).unwrap_err().is_full());

    // We can use `recv` method of the `Stream` implementation to receive messages.
    // assert_eq!(r1.next().await.unwrap(), 7);
    // assert_eq!(r1.recv().await.unwrap(), 8);
    assert_eq!(r2.next().await.unwrap(), 7);
    assert_eq!(r2.recv().await.unwrap(), 8);

    // All receiver got all messages so channel is now empty.
    assert_eq!(r1.try_recv(), Err(TryRecvError::Empty));
    assert_eq!(r2.try_recv(), Err(TryRecvError::Empty));

    // Drop both senders, which closes the channel.
    drop(s1);
    drop(s2);

    assert_eq!(r1.try_recv(), Err(TryRecvError::Closed));
    assert_eq!(r2.try_recv(), Err(TryRecvError::Closed));
    Ok(())
}
