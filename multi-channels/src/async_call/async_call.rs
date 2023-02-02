use std::future::Future;


/// Similar to tower Service
pub trait AsyncCall<'s, Input> {
    type Output;
    type Future: Future<Output = Self::Output> ;
    fn async_call(&'s mut self, input: Input) -> Self::Future;
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

    type Input<'c> = &'c i32;

    impl<'c> AsyncCall<'c, Input<'c>> for Test<'c> {
        type Output = (&'c i32, &'c i32);
        type Future = impl Future<Output = Self::Output> ; 

        fn async_call(&'c mut self, req: &'c i32) -> Self::Future {
            async move {
                (self.0, req)
            }
        }
    }
}
