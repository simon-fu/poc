use std::future::Future;

// /// Similar to tower Service
// pub trait AsyncCall<Request> {
//     type Response;
//     type Future: Future<Output = Self::Response>;
//     fn async_call(&mut self, req: Request) -> Self::Future;
// }

/// Similar to tower Service
pub trait AsyncCall<'s, Request> {
    type Response;

    type Future: Future<Output = Self::Response>;
    fn async_call(&'s mut self, req: Request) -> Self::Future;
}


#[cfg(test)]
mod test {
    use std::future::Future;
    use super::AsyncCall;

    #[tokio::test]
    async fn test() {
        let val1 = 1;
        let val2 = 2;
        let mut test = Test(&val1);
        let r = test.async_call(&val2).await;
        assert_eq!(r, (&1, &2));
    }

    struct Test<'c>(&'c i32);

    impl<'c, 's: 'c> AsyncCall<'s, &'c i32> for Test<'s> 
    {
        type Response = (&'c i32, &'c i32);
        type Future = impl Future<Output = Self::Response> + 'c; //  'c is optional;
        // type Future<'a> = impl Future<Output = Self::Response> + 'a where Self: 'a;

        fn async_call(&'s mut self, req: &'c i32) -> Self::Future {
            async move {
                (self.0, req)
            }
        }
    }

    // impl<'c, 's: 'c> AsyncCall1<&'c i32> for Test<'s> 
    // where Self: 's
    // {
    //     type Response = (&'c i32, &'c i32);
    //     type Future = impl Future<Output = Self::Response> + 'c; //  + 'c where Self: 'c;
    //     // type Future<'a> = impl Future<Output = Self::Response> + 'a where Self: 'a;

    //     fn async_call(&mut self, req: &'c i32) -> Self::Future {
    //         async move {
    //             (self.0, req)
    //         }
    //     }
    // }

    // pub trait AsyncCall3<'s, Request>: 's {
    //     type Response;
    
    //     type Future: Future<Output = Self::Response>;
    //     fn async_call(&'s mut self, req: Request) -> Self::Future;
    // }

    // pub trait AsyncCall2<Request> {
    //     type Response;
    //     type Future<'a>: Future<Output = Self::Response> + 'a where Self: 'a;
    //     fn async_call(&mut self, req: Request) -> Self::Future<'_>;
    // }

    /// Similar to tower Service
    pub trait AsyncCall1<Request> {
        type Response;
        type Future: Future<Output = Self::Response>;
        fn async_call(&mut self, req: Request) -> Self::Future;
    }

    // trait RecvNumFut<M> 
    // where
    //     M: MpscOp<Mail<ChId, SeqVal<TestVal>>> + 'static,
    // {
    //     type Fut<'a>: Future<Output = OutVec > ;

    //     fn recv_num_fut<'a, 'b>(tester: &'a mut Tester<M>, num: usize) -> Self::Fut<'b> where 'a: 'b;
    // }

    // struct AsyncRecvNum;

    // impl<M> RecvNumFut<M> for AsyncRecvNum 
    // where
    //     M: MpscOp<Mail<ChId, SeqVal<TestVal>>> + 'static,
    // {
    //     type Fut<'a> = impl Future< Output = OutVec > + 'a ;

    //     fn recv_num_fut<'a, 'b>(tester: &'a mut Tester<M>, num: usize) -> Self::Fut<'b> where 'a: 'b
    //     {
    //         tester.recv_num(num)
    //     }
    // }

    // struct SyncRecvNum;

    // impl<M> RecvNumFut<M> for SyncRecvNum 
    // where
    //     M: MpscOp<Mail<ChId, SeqVal<TestVal>>> + 'static,
    // {
    //     type Fut<'a> = impl Future< Output = OutVec > + 'a ;

    //     fn recv_num_fut<'a, 'b>(tester: &'a mut Tester<M>, num: usize) -> Self::Fut<'b> where 'a: 'b
    //     {
    //         std::future::ready(tester.try_recv_num(num)) 
    //     }
    // }
}