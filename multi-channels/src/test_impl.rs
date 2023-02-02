///
/// 修改 use implxx 测试各种实现
/// 


#[cfg(test)]
mod test {
    use anyhow::{Result, Context, bail};

    use crate::{
        ch_common::{uid::ChId, SeqVal, RecvOutput}, 
        async_call::{async_call_fn, AsyncCall},
        impl02::{ SChannel, Suber, impl_name, Mail, Puber},
        mpsc_ch:: {
            mpsc_defs::MpscOp, 
            mpsc_async_broadcast, 
            mpsc_async_channel, 
            mpsc_tokio_mpsc, 
            mpsc_tokio_broadcast,
            mpsc_crossbeam_que,
            mpsc_concurrent_que,
            mpsc_kanal, mpsc_flume, 
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

    #[tokio::test]
    async fn test_mpsc_concurrent_que7() -> Result<()> {
        test_num::<mpsc_concurrent_que::Mpsc>().await
    }

    #[tokio::test]
    async fn test_mpsc_flume8() -> Result<()> {
        test_num::<mpsc_flume::Mpsc>().await
    }

    async fn test_num<M>() -> Result<()>
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>> + 'static,
    {
        const MAX_RUNS: usize = 16;

        for n in 1..=MAX_RUNS {
            test_subscribe::<M>().await.with_context(||format!("fail at NO.{} when test_subscribe  [{}]", n, impl_name()))?;
        }
        // println!("test1 Ok times [{}] for [{}]-[{}]", MAX_RUNS, impl_name(), M::name());

        for n in 1..=MAX_RUNS {
            test_sync_recv_and_lagged::<M>().await.with_context(||format!("fail at NO.{} when test_sync_recv_and_lagged [{}]", n, impl_name()))?;
        }

        for n in 1..=MAX_RUNS {
            test_async_recv_and_lagged::<M>().await.with_context(||format!("fail at NO.{} when test_async_recv_and_lagged [{}]", n, impl_name()))?;
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

    async fn test_sync_recv_and_lagged<M>() -> Result<()>
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>>+ 'static,
    {
        async fn handle<M>(request: (&mut Tester<M>, usize)) -> OutVec
        where
            M: MpscOp<Mail<ChId, SeqVal<TestVal>>> + 'static,
        {
            request.0.try_recv_num(request.1)
        }

        let caller = async_call_fn(handle::<M>);
        test_recv_and_lagged(caller).await?;
        Ok(())

    }

    async fn test_async_recv_and_lagged<M>() -> Result<()>
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>>+ 'static,
    {
        async fn handle<M>(request: (&mut Tester<M>, usize)) -> OutVec
        where
            M: MpscOp<Mail<ChId, SeqVal<TestVal>>> + 'static,
        {
            request.0.recv_num(request.1).await
        }

        let caller = async_call_fn(handle::<M>);
        test_recv_and_lagged(caller).await?;
        Ok(())

    }

    async fn test_recv_and_lagged<M, S>(mut caller: S) -> Result<()>
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>> + 'static,
        for<'s, 'a> S: AsyncCall<'s, (&'a mut Tester<M>, usize), Output = OutVec>,
        // for<'a> Fut: std::future::Future + 'a,
    {
        let mut tester = make_tester::<M>()?;

        let r = tester.subscribe_all_and_check();
        assert!(r.is_ok(), "r={:?}", r);
        
        // test normal recv
        for _ in 0..10 { 
            tester.clear_values();
            
            let r = tester.push_all(1..tester.ch_cap + 1);
            assert!(r.is_ok(), "r={:?}", r);
    
            let num = tester.ch_cap * tester.ch_ids.len();
            // let outputs = tester.recv_num(num).await;
            let outputs = caller.async_call((&mut tester, num)).await;

            let r = tester.check_outputs(&mut Vec::new(), &outputs);
            assert!(r.is_ok(), "r={:?}", r);
    
            let r = tester.recv_none();
            assert!(r.is_ok(), "r={:?}", r);
        }

        // test all lagged
        {
            let r = tester.push_all(1..tester.ch_cap + 2);
            assert!(r.is_ok(), "r={:?}", r);  

            let num = tester.channels.len();
            // let outputs = tester.recv_num(num).await;
            let outputs = caller.async_call((&mut tester, num)).await;
            
            let r = tester.check_outputs(&mut tester.all_index(), &outputs);
            assert!(r.is_ok(), "r={:?}", r);
        }

        let r = tester.re_subscribe_all();
        assert!(r.is_ok(), "r={:?}", r);

        // test partial lagged
        {
            tester.clear_values();

            // index 0: lagged 
            let r = tester.push_ch(0, 1..tester.ch_cap + 2);
            assert!(r.is_ok(), "r={:?}", r);  

            // index 1: value
            let r = tester.push_ch(1, 1..tester.ch_cap + 1);
            assert!(r.is_ok(), "r={:?}", r);  

            let mut laggeds = vec![0];

            for _ in 0..tester.channels.len() {
                let r = tester.suber.try_recv(); 
                let r = tester.check_output(&mut laggeds, &r);
                assert!(r.is_ok(), "r={:?}", r);
            }
        }
    
        Ok(())
    }



    struct Tester<M> 
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>>,
    {
        _inbox_cap: usize, 
        ch_cap: usize,
        ch_ids: Vec<ChId>,
        channels: Vec<SChannel<ChId, usize, M>>,
        pubers: Vec<Puber<ChId, SeqVal<usize>, M>>,
        suber: Suber<ChId, SeqVal<TestVal>, M>,
        seqs: Vec<u64>,
        values: Vec<TestVal>,
    }

    impl<M> Tester<M> 
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>>,
    {
        fn all_index(&self) -> Vec<usize> {
            (0..self.ch_ids.len()).enumerate().map(|x|x.0).collect()
        }

        fn subscribe_all_and_check(&mut self) -> Result<()> {
            
            self.subscribe_all()?;

            if self.suber.channels() != self.channels.len() {
                bail!("subscribe_all_and_check: check fail {} != {}", self.suber.channels(), self.channels.len() )
            }

            self.check_ch_subers(1)?;

            Ok(())
        }

        fn re_subscribe_all(&mut self) -> Result<()> { 
            for ch_id in self.ch_ids.iter() {
                self.suber.unsubscribe(ch_id);
            }
            self.subscribe_all()
        }

        fn subscribe_all(&mut self) -> Result<()> {
            for (index, ch) in self.channels.iter().enumerate() {
                self.suber.subscribe(ch, ch.tail_seq())
                .with_context(||format!("subscribe_all: fail to subscribe {:?}", ch.ch_id()))?;
                self.seqs[index] = ch.tail_seq()-1;
            }
            Ok(())
        }

        fn try_recv_num(&mut self, num: usize) -> Vec<RecvOutput<ChId, SeqVal<TestVal>>> {
            let mut outputs = Vec::with_capacity(num);
            for _ in 0..num {
                let r = self.suber.try_recv();
                outputs.push(r);
            }
            outputs
        }

        async fn recv_num(&mut self, num: usize) -> Vec<RecvOutput<ChId, SeqVal<TestVal>>> {
            let mut outputs = Vec::with_capacity(num);
            for _ in 0..num {
                let r = self.suber.recv_next().await;
                outputs.push(r);
            }
            outputs
        }

        fn recv_none(&mut self) -> Result<()> {
            let r = self.suber.try_recv();
            if !r.is_none() {
                bail!("expect recv none but {:?}", r)
            } else {
                Ok(())
            }
        }

        fn check_ch_subers(&self, num: usize) -> Result<()> {
            for ch in &self.channels {
                if ch.subers() != num {
                    bail!("check_ch_subers: wrong subers {:?}", (ch.subers(), num))
                }
            }
            Ok(())
        }

        fn clear_values(&mut self) {
            for v in self.values.iter_mut() {
                *v = 0;
            }
        }

        fn push_ch<I>(&mut self, index: usize, iter: I) -> Result<()> 
        where
            I: Iterator<Item = TestVal>,
        {
            for n in iter { 
                self.pubers[index].push(n).with_context(||format!("push fail, n={}, index={}", n, index))?;
            }
            Ok(())
        }

        fn push_all<I>(&mut self, iter: I) -> Result<()> 
        where
            I: Iterator<Item = TestVal>,
        {
            // for n in 0..self.ch_cap ;
            for n in iter { 
                for puber in &mut self.pubers {
                    puber.push(n)
                    .with_context(||format!("push fail, n={}", n))?;
                }
            }
            Ok(())
        }

        fn check_outputs(
            &mut self, 
            laggeds: &mut Vec<usize>, 
            outputs: &Vec<RecvOutput<ChId, SeqVal<TestVal>>>,
        ) -> Result<()> {
            for output in outputs.iter() {
                self.check_output(laggeds, output)?;
            }
            Ok(())
        }

        fn check_output(
            &mut self, 
            laggeds: &mut Vec<usize>, 
            output: &RecvOutput<ChId, SeqVal<TestVal>>,
        ) -> Result<()> {
            match output {
                RecvOutput::Value(_ch_id, _v) => self.check_value(&output),
                RecvOutput::Lagged(_) => self.check_lagged_and_unsub(laggeds, &output),
                RecvOutput::None => {
                    bail!("check_output but none")
                },
            }
        }

        fn check_value(&mut self, r: &RecvOutput<ChId, SeqVal<TestVal>>) -> Result<()> {
            if let RecvOutput::Value(ch_id, _val) = &r { 
                let index = ch_id.to_index();
                self.seqs[index] += 1;
                self.values[index] += 1;
                // println!("n={}, r={:?}", n, r);
                // assert_eq!(r, RecvOutput::Value(self.ch_ids[index], SeqVal(self.seqs[index], self.values[index])), "n={}, r={:?}", n, r);
                let expect = RecvOutput::Value(self.ch_ids[index], SeqVal(self.seqs[index], self.values[index]));
                if *r != expect {
                    bail!("Not equal, r={:?}, expect {:?}", r, expect)    
                } 
                Ok(())
            } else {
                // assert!(false, "n={}, r={:?}", n, r);
                bail!("Not value, r={:?}", r)
            }
        }

        fn check_lagged_and_unsub(&mut self, laggeds: &mut Vec<usize>, output: &RecvOutput<ChId, SeqVal<TestVal>>) -> Result<()> {

            if let RecvOutput::Lagged(ch_id) = output {
                let index = ch_id.to_index();
                let r = laggeds.iter().position(|x| *x == index);
                if !r.is_some() {
                    bail!("Not found in laggeds, output={:?}", output);
                }
                
                let _r = laggeds.swap_remove(r.unwrap());
                let r = self.suber.unsubscribe(ch_id);
                if !r.is_some() {
                    bail!("Not found in lagged ids, output={:?}", output);
                }
                Ok(())
            } else {
                bail!("Not lagged, output={:?}", output);
            }
        }
        
    }

    fn make_tester<M>() -> Result<Tester<M>>
    where
        M: MpscOp<Mail<ChId, SeqVal<TestVal>>>,
    {
        let inbox_cap = 8;
        let ch_cap = 16;        

        let ch_ids: Vec<ChId> = vec![
            ChId::new(0),
            ChId::new(1),
            ChId::new(2),
            ChId::new(3),
        ];

        let channels: Vec<_> = ch_ids.iter()
        .map(|x|SChannel::<ChId, TestVal, M>::with_capacity(*x, ch_cap))
        .collect();


        let pubers: Vec<_> = channels.iter()
        .map(|x|x.puber())
        .collect();

    
        let suber = Suber::with_inbox_cap(inbox_cap);
        

        let seqs: Vec<_> = channels.iter()
        .map(|_x| 0)
        .collect();

        let values: Vec<TestVal> = channels.iter()
        .map(|_x|0)
        .collect();

        Ok(Tester { 
            _inbox_cap: inbox_cap,
            ch_cap,
            ch_ids,
            channels,
            pubers,
            suber,
            seqs,
            values,
        })
    }


    trait ToIndex {
        fn to_index(&self) -> usize;
    }

    impl ToIndex for ChId {
        fn to_index(&self) -> usize {
            self.to() as usize
        }
    }

    fn vec_remove<T>(vec: &mut Vec<T>, val: &T) -> Result<T> 
    where
        T: Eq+std::fmt::Debug,
    {
        let index = vec.iter().position(|x|*x == *val)
        .with_context(||format!("Not found [{:?}]", val))?;

        Ok(vec.swap_remove(index))
    }

    type OutVec = Vec<RecvOutput<ChId, SeqVal<TestVal>>>;
    type TestVal = usize;
}




