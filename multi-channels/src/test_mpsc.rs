///
/// TODO:
/// - test async recv

#[cfg(test)]
mod test {
    use anyhow::Result;
    use crate::{
        impl02::{
            mpsc_defs::{MpscOp, SenderOp, TryRecvOp, error::{TryRecvError, RecvError}, RecvOp}, 
            mpsc_async_broadcast, 
            mpsc_tokio_mpsc, 
            mpsc_async_channel, 
            mpsc_tokio_broadcast,
            mpsc_crossbeam_que,
            mpsc_kanal, 
            mpsc_concurrent_que,
            mpsc_flume,
        } 
    };

    #[tokio::test]
    async fn test_mpsc_async_broadcast() -> Result<()> {
        test_num::<mpsc_async_broadcast::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_async_channel() -> Result<()> {
        test_num::<mpsc_async_channel::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_concurrent_que() -> Result<()> {
        test_num::<mpsc_concurrent_que::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_tokio_broadcast() -> Result<()> {
        test_num::<mpsc_tokio_broadcast::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_tokio_mpsc() -> Result<()> {
        test_num::<mpsc_tokio_mpsc::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_flume() -> Result<()> {
        test_num::<mpsc_flume::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_crossbeam_que() -> Result<()> {
        test_num::<mpsc_crossbeam_que::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_kanal() -> Result<()> {
        test_num::<mpsc_kanal::Mpsc>().await
    }

    async fn test_num<M>() -> Result<()>
    where
        M: MpscOp<TestVal> + 'static,
        for<'a> <<M as MpscOp<usize>>::Receiver as RecvOp<usize>>::Fut<'a>: Send,
        <M as MpscOp<usize>>::Receiver: Send + 'static,
        <M as MpscOp<usize>>::Sender: Send + 'static,
    {
        const MAX_RUNS: usize = 16;

        for n in 1..=MAX_RUNS {
            let r = test_overflow::<M>().await;
            assert!(r.is_ok(), "impl={}, n={}", n, M::name());
        }

        for n in 1..=MAX_RUNS {
            let r = test_overflow_async::<M>().await;
            assert!(r.is_ok(), "impl={}, n={}", n, M::name());
        }

        for n in 1..=MAX_RUNS {
            let r = test_multi_rx::<M>().await;
            assert!(r.is_ok(), "impl={}, n={}", n, M::name());
        }

        Ok(())
    }

    async fn test_overflow<M>() -> Result<()>
    where
        M: MpscOp<TestVal>,
    {
        let capacity = 16; // tokio broadcast 会向上取到2的整数幂，如果不是整数幂则实际容量会比设置值大

        let (mut tx, mut rx) = M::channel(capacity);
        
        let base = 0;

        for n in 0..capacity {
            let r = tx.try_send(base+n);
            assert!(r.is_ok(), "n={}", n);
        }

        let r = tx.try_send(base+capacity);
        // assert!(r.is_err(), "r={:?}", r);
        if r.is_ok() {

            // lost first value 
            let r = rx.try_recv();
            assert_eq!(r, Err(TryRecvError::Overflowed));

            // recv starting from second value 
            for n in 1..capacity {
                let r = rx.try_recv();
                assert_eq!(r, Ok(base+n), "n={}", n);
            }
            
            // recv last value
            let r = rx.try_recv();
            assert_eq!(r, Ok(base+capacity));

        } else {

            // lost last value 
            let r = rx.try_recv();
            assert_eq!(r, Err(TryRecvError::Overflowed));
    
            // recv starting from fisrt value
            for n in 0..capacity {
                let r = rx.try_recv();
                assert_eq!(r, Ok(base+n), "n={}", n);
            }
        }

        let r = tx.try_send(1);
        assert!(r.is_ok());

        let r = rx.try_recv();
        assert_eq!(r, Ok(1));
    
        Ok(())
    }

    async fn test_overflow_async<M>() -> Result<()>
    where
        M: MpscOp<TestVal>,
    {
        let capacity = 16; // tokio broadcast 会向上取到2的整数幂，如果不是整数幂则实际容量会比设置值大

        let (mut tx, mut rx) = M::channel(capacity);
        
        let base = 0;

        for n in 0..capacity {
            let r = tx.try_send(base+n);
            assert!(r.is_ok(), "n={}", n);
        }

        let r = tx.try_send(base+capacity);
        // assert!(r.is_err(), "r={:?}", r);
        if r.is_ok() {

            // lost first value 
            let r = rx.recv().await;
            assert_eq!(r, Err(RecvError));

            // recv starting from second value 
            for n in 1..capacity {
                let r = rx.recv().await;
                assert_eq!(r, Ok(base+n), "n={}", n);
            }
            
            // recv last value
            let r = rx.recv().await;
            assert_eq!(r, Ok(base+capacity));

        } else {

            // lost last value 
            let r = rx.recv().await;
            assert_eq!(r, Err(RecvError));
    
            // recv starting from fisrt value
            for n in 0..capacity {
                let r = rx.recv().await;
                assert_eq!(r, Ok(base+n), "n={}", n);
            }
        }

        let r = tx.try_send(1);
        assert!(r.is_ok());

        let r = rx.recv().await;
        assert_eq!(r, Ok(1));
    
        Ok(())
    }    

    async fn test_multi_rx<M>() -> Result<()>
    where
        M: MpscOp<TestVal> + 'static,
        for<'a> <<M as MpscOp<usize>>::Receiver as RecvOp<usize>>::Fut<'a>: Send,
        <M as MpscOp<usize>>::Receiver: Send + 'static,
        <M as MpscOp<usize>>::Sender: Send + 'static,
    {
        let capacity = 128;
        let msg_num = 2;
        let send_num = capacity / msg_num;

        let (tx, mut rx) = M::channel(capacity);
        let mut send_tasks = Vec::with_capacity(send_num);

        let recv_task = tokio::spawn(async move { 
            for n in 0..msg_num*send_num {
                let r = rx.recv().await;
                assert!(r.is_ok(), "n={}", n);
            }
        });

        for send_no in 0..send_num {
            let mut tx = tx.clone();
            let h = tokio::spawn(async move { 
                
                for n in 0..msg_num {
                    let r = tx.try_send(n);
                    assert!(r.is_ok(), "send_no={}, n={}", send_no, n);
                }

            });
            send_tasks.push(h);
        }

        for h in send_tasks {
            h.await?;
        }

        let _r = recv_task.await?;

        Ok(())
    }


    type TestVal = usize;
}
