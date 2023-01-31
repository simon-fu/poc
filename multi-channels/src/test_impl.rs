///
/// 修改 use implxx 测试各种实现
/// 


#[cfg(test)]
mod test {
    use anyhow::{Result, Context};
    use crate::{
        ch_common::{uid::ChId, SeqVal, RecvOutput}, 
        impl02::{ SChannel, Suber, impl_name, Mail},
        mpsc_ch:: {
            mpsc_defs::MpscOp, 
            mpsc_async_broadcast, 
            mpsc_async_channel, 
            mpsc_tokio_mpsc, 
            mpsc_tokio_broadcast,
            mpsc_crossbeam_que,
            mpsc_kanal,
        },
    };


    #[tokio::test]
    async fn test_mpsc_async_broadcast1() -> Result<()> {
        test_num::<mpsc_async_broadcast::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_async_channel2() -> Result<()> {
        test_num::<mpsc_async_channel::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_tokio_mpsc3() -> Result<()> {
        test_num::<mpsc_tokio_mpsc::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_tokio_broadcast4() -> Result<()> {
        test_num::<mpsc_tokio_broadcast::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_crossbeam_que5() -> Result<()> {
        test_num::<mpsc_crossbeam_que::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_kanal6() -> Result<()> {
        test_num::<mpsc_kanal::Mpsc>().await
    }


    async fn test_num<M>() -> Result<()>
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>>,
    {
        const MAX_RUNS: usize = 16;

        for n in 1..=MAX_RUNS {
            test_subscribe::<M>().await.with_context(||format!("fail at NO.{} when testing [{}]", n, impl_name()))?;
        }
        // println!("test1 Ok times [{}] for [{}]-[{}]", MAX_RUNS, impl_name(), M::name());

        for n in 1..=MAX_RUNS {
            test_try_recv_and_lagged::<M>().await.with_context(||format!("fail at NO.{} when testing [{}]", n, impl_name()))?;
        }

        for n in 1..=MAX_RUNS {
            test_recv_and_lagged::<M>().await.with_context(||format!("fail at NO.{} when testing [{}]", n, impl_name()))?;
        }
        
        Ok(())
    }

    async fn test_subscribe<M>() -> Result<()>
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>>,
    {
        let ch_cap = 16;        
        let inbox_cap = 8;
        
        let ch_id1: ChId = 1.into();
        let ch_id2: ChId = 2.into();
    
    
        let ch1 = SChannel::<ChId, TestVal, M>::with_capacity(ch_id1, ch_cap);
        let mut puber1 = ch1.puber();
    
        let ch2 = SChannel::<ChId, TestVal, M>::with_capacity(ch_id2, ch_cap);
        let mut puber2 = ch2.puber();
    
        let mut suber = Suber::with_inbox_cap(inbox_cap);
    
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

    async fn test_try_recv_and_lagged<M>() -> Result<()>
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>>,
    {
        let ch_cap = 16;        
        let ch_id1: ChId = 1.into();
        let inbox_cap = 8;

        let ch1 = SChannel::<ChId, TestVal, M>::with_capacity(ch_id1, ch_cap);
        let mut puber1 = ch1.puber();
    
        let mut suber = Suber::with_inbox_cap(inbox_cap);
    
        suber.subscribe(&ch1, 1)?;
        assert_eq!(suber.channels(), 1);
        assert_eq!(ch1.subers(), 1);
    
        let mut seq = 0;

        // test normal try_recv
        for _ in 0..10 {
            for n in 0..ch_cap {
                let r = puber1.push(n);
                assert!(r.is_ok(), "n={}", n);
            }
    
            for n in 0..ch_cap { 
                seq += 1;
                let r = suber.try_recv();
                assert_eq!(r, RecvOutput::Value(ch_id1, SeqVal(seq, n)), "n={}", n);
            }
    
            let r = suber.try_recv();
            assert_eq!(r, RecvOutput::None);
        }

        // test lagged
        {
            for n in 0..ch_cap+1 {
                let r = puber1.push(n);
                assert!(r.is_ok(), "n={}", n);
            }    

            let r = suber.try_recv();
            assert_eq!(r, RecvOutput::Lagged(ch_id1));
        }
    
        Ok(())
    }

    async fn test_recv_and_lagged<M>() -> Result<()>
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>>,
    {
        let ch_cap = 16;        
        let ch_id1: ChId = 1.into();
        let inbox_cap = 8;

        let ch1 = SChannel::<ChId, TestVal, M>::with_capacity(ch_id1, ch_cap);
        let mut puber1 = ch1.puber();
    
        let mut suber = Suber::with_inbox_cap(inbox_cap);
    
        suber.subscribe(&ch1, 1)?;
        assert_eq!(suber.channels(), 1);
        assert_eq!(ch1.subers(), 1);
    
        let mut seq = 0;

        // test normal try_recv
        for _ in 0..10 {
            for n in 0..ch_cap {
                let r = puber1.push(n);
                assert!(r.is_ok(), "n={}", n);
            }
    
            for n in 0..ch_cap { 
                seq += 1;
                let r = suber.recv_next().await;
                assert_eq!(r, RecvOutput::Value(ch_id1, SeqVal(seq, n)), "n={}", n);
            }
    
            let r = suber.try_recv();
            assert_eq!(r, RecvOutput::None);
        }

        // test lagged
        {
            for n in 0..ch_cap+1 {
                let r = puber1.push(n);
                assert!(r.is_ok(), "n={}", n);
            }    

            let r = suber.recv_next().await;
            assert_eq!(r, RecvOutput::Lagged(ch_id1));
        }
    
        Ok(())
    }

    fn vec_remove<T>(vec: &mut Vec<T>, val: &T) -> Result<T> 
    where
        T: Eq+std::fmt::Debug,
    {
        let index = vec.iter().position(|x|*x == *val)
        .with_context(||format!("Not found [{:?}]", val))?;

        Ok(vec.swap_remove(index))
    }

    type TestVal = usize;
}
