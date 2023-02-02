use std::future::Future;
use super::AsyncCall;

/// Similar to service_fn of tower
/// 
/// # Examples
/// 
/// ```
/// 
/// use async_call::{async_call_fn, AsyncCall};
///  
/// pub async fn call_example1() {
///     async fn handle(t: &mut i32) -> i32 {
///         *t
///     }
///     example1(async_call_fn(handle));
/// }
/// 
/// pub async fn example1<C>(mut func: C) 
/// where
///     for<'a> C: AsyncCall<&'a mut i32, Response = i32>,
/// {
///     let mut t1 = 1;
///     let t2 = func.async_call(&mut t1).await;
///     assert_eq!(t1, t2);
/// }
/// ```
pub fn async_call_fn<F>(f: F) -> AsyncCallFn<F> {
    AsyncCallFn{f}
}

#[derive(Copy, Clone)]
pub struct AsyncCallFn<F> {
    f: F,
}

impl<F> std::fmt::Debug for AsyncCallFn<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncCallFn")
        .field("f", &format_args!("{}", std::any::type_name::<F>()))
        .finish()
    }
}

// impl<F, Fut, Request, Response> AsyncCall<Request> for AsyncCallFn<F>
// where
//     F: FnMut(Request) -> Fut,
//     Fut: Future<Output = Response>,
// {
//     type Response = Response;
//     type Future = Fut;

//     fn async_call(&mut self, req: Request) -> Self::Future {
//         (self.f)(req)
//     }
// }

impl<'s, F, Fut, Request, Response> AsyncCall<'s, Request> for AsyncCallFn<F>
where
    F: FnMut(Request) -> Fut + 's,
    Fut: Future<Output = Response>,
{
    type Response = Response;
    type Future = Fut;

    fn async_call(&mut self, req: Request) -> Self::Future {
        (self.f)(req)
    }
}

// impl<'c> AsyncCall<&'c i32> for Test<'c> {
//     type Response = (&'c i32, &'c i32);

//     type Future<'a> = impl Future<Output = Self::Response> + 'a where Self: 'a;

//     fn async_call(&mut self, req: &'c i32) -> Self::Future<'_> {
//         async move {
//             (self.0, req)
//         }
//     }
// }