


use std::marker::PhantomData;

use futures::Future;
use tracing::{info, warn};
use anyhow::{Result, anyhow, Context as AnyhowContext};
use tokio::{ task::JoinHandle, sync::{mpsc, oneshot}};


use crate::spawn_with_name;



pub fn start_actor<E, F1, F2, O1>(
    entity: E, 
    wait_next: F1,      // async fn wait_next(&mut E) -> O1
    handle_next: F2,    // async fn handle_next(&mut E, O1) -> Result<()>
    // handle_invoke: F3,  // async fn handle_invoke(&mut UdpEntity, Req) -> Rsp 
) -> (Invoker<E>, Wait4Completed<E>)
where
    E: ActorEntity ,

    for<'a> F1: XFn1<'a, &'a mut E, O1> + Send + Sync + 'static + Copy,
    for<'a> <F1 as XFn1<'a, &'a mut E, O1>>::Output: 'a,

    for<'a> F2: XFn2<'a, &'a mut E, O1, Result<()>> + Send + Sync + 'static + Copy,
    for<'a> <F2 as XFn2<'a, &'a mut E, O1, Result<()>>>::Output: 'a,

    // for<'a> F3: XFn2<'a, &'a mut E, Req, Rsp> + Send + Sync + 'static + Copy,
    // for<'a> <F3 as XFn2<'a, &'a mut E, Req, Rsp>>::Output: 'a,

    O1: Send + 'static,
    // Req: Send + 'static,
    // Rsp: Send + 'static,
{
    let (op_tx, op_rx) = mpsc::channel(128);

    let mut task = ActorTask {
        op_rx,
        actor: Actor {
            entity,
            wait_next,
            handle_next,
            // handle_invoke,
            none: PhantomData::default(),
            // none2: PhantomData::default(),
            // none3: PhantomData::default(),
        },
    };
    
    let name = task.actor.entity.name();
    let task_handle = spawn_with_name(name, async move {
        let r = run_actor(&mut task).await;
        if let Err(e) = r {
            warn!("finish with err [{:?}]", e)
        }
        task.actor.entity
    });
    
    (Invoker { op_tx}, Wait4Completed{ task_handle })
}




pub trait ActorEntity: Send + 'static{
    fn name(&self) -> String;
}

pub struct Wait4Completed<E> {
    task_handle: JoinHandle<E>,
}

impl<E> Wait4Completed<E> {
    pub async fn wait_for_completed(self) -> Result<E> {
        let entity = self.task_handle.await?;
        Ok(entity)
    }
}

#[derive(Clone)]
pub struct Invoker<E> {
    op_tx: mpsc::Sender<Op<E>>,
    // none: PhantomData<A>,
}

impl<E> Invoker<E> {

    // pub async fn invoke(&self, req: Req) -> Result<Rsp> 
    // {
    //     let (tx, rx) = oneshot::channel();
    //     self.op_tx.send(Op::Invoke(req, tx)).await
    //     .map_err(|_x|anyhow!("send request error"))?;
    //     let rsp = rx.await.with_context(||"recv response but error")?;
    //     Ok(rsp)
    // }

    pub async fn invoke<Request, Response>(&self, req: Request) -> Result<Response> 
    where
        Request: Send + 'static,
        Response: Send + 'static,
        E: AsyncHandler<Request, Response = Response> + Send,
    {
        let (tx, rx) = oneshot::channel();
        self.op_tx.send(Op::Envelope(AsyncEnvelope::new(req, tx))).await
        .map_err(|_x|anyhow!("send request error"))?;
        let rsp = rx.await.with_context(||"recv response but error")?;
        Ok(rsp)
    }

    pub async fn shutdown(&self) {
        let _r = self.op_tx.send(Op::Shutdown).await
        .map_err(|_x|anyhow!("send request error"));
    }
}


async fn run_actor<E, F1, O1, F2>(task: &mut ActorTask<E, F1, F2, O1>) -> Result<()>
where
    E: ActorEntity ,

    for<'a> F1: XFn1<'a, &'a mut E, O1> + Send + Sync + 'static + Copy,
    for<'a> <F1 as XFn1<'a, &'a mut E, O1>>::Output: 'a,

    for<'a> F2: XFn2<'a, &'a mut E, O1, Result<()>> + Send + Sync + 'static + Copy,
    for<'a> <F2 as XFn2<'a, &'a mut E, O1, Result<()>>>::Output: 'a,

    // for<'a> F3: XFn2<'a, &'a mut E, Req, Rsp> + Send + Sync + 'static + Copy,
    // for<'a> <F3 as XFn2<'a, &'a mut E, Req, Rsp>>::Output: 'a,

    O1: Send + 'static,
    // Req: Send + 'static,
    // Rsp: Send + 'static,
{

    loop {
        tokio::select! {
            r = task.actor.wait_next.call_me(&mut task.actor.entity) => {
                task.actor.handle_next.call_me(&mut task.actor.entity, r).await?;
            }
            r = task.op_rx.recv() => {
                match r {
                    Some(op) => {
                        let shutdown = handle_op(&mut task.actor.entity, op).await?;
                        if shutdown {
                            info!("got shutdown");
                            break;
                        }
                    },
                    None => {
                        info!("no one care, done");
                        break;
                    }
                }
            },
        }
    }
    Ok(())
}

async fn handle_op<E>(entity: &mut E, op: Op<E>) -> Result<bool>
where
    E: ActorEntity ,

    // for<'a> F3: XFn2<'a, &'a mut E, Req, Rsp> + Send + Sync + 'static + Copy,
    // for<'a> <F3 as XFn2<'a, &'a mut E, Req, Rsp>>::Output: 'a,

    // Req: Send + 'static,
    // Rsp: Send + 'static,
{
    match op {
        Op::Shutdown => return Ok(true),
        // Op::Invoke(req, tx) => {
        //     let rsp = func.call_me(entity, req).await;
        //     let _r = tx.send(rsp);
        // },
        Op::Envelope(mut envelope) => {
            envelope.handle(entity).await;
        }
    }
    
    Ok(false)
}


struct Actor<E, F1, F2, O1> {
    entity: E,
    wait_next: F1,
    handle_next: F2,
    // handle_invoke: F3,
    none: PhantomData<O1>,
    // none2: PhantomData<Req>,
    // none3: PhantomData<Rsp>,
}

struct ActorTask<E, F1, F2, O1> {
    op_rx: mpsc::Receiver<Op<E>>,
    actor: Actor<E, F1, F2, O1>,
}


// enum Op<Req, Rsp> {
//     Shutdown,
//     Invoke(Req, oneshot::Sender<Rsp>),
// }

enum Op<A> {
    Shutdown,
    Envelope(AsyncEnvelope<A>),
}


#[async_trait::async_trait]
pub trait AsyncHandler<M>
// where
    // Self: Actor,
    // M: Message,
{
    type Response: Send; //: MessageResponse<Self, M>;

    async fn handle(&mut self, msg: M) -> Self::Response;
}


#[async_trait::async_trait]
pub trait AsyncEnvelopeProxy<A> {
    /// handle message within new actor and context
    async fn handle(&mut self, act: &mut A);
}


pub struct AsyncEnvelope<A>(Box<dyn AsyncEnvelopeProxy<A> + Send>);

impl<A> AsyncEnvelope<A> {
    pub fn new<M>(msg: M, tx: oneshot::Sender<A::Response>) -> Self
    where
        A: AsyncHandler<M> + Send,
        A::Response: 'static,
        // A::Context: AsyncContext<A>,
        M: Send + 'static, // + Message ,
        // M::Result: Send,
    {
        AsyncEnvelope(Box::new(AsyncEnvelopeReal { msg: Some((msg, tx)) }))
    }

    pub fn with_proxy(proxy: Box<dyn AsyncEnvelopeProxy<A> + Send>) -> Self {
        AsyncEnvelope(proxy)
    }
}

#[async_trait::async_trait]
impl<A> AsyncEnvelopeProxy<A> for AsyncEnvelope<A> 
where
    A: Send,
{
    async fn handle(&mut self, act: &mut A) {
        self.0.handle(act).await
    }
}



pub struct AsyncEnvelopeReal<M, Rsp>
where
    M: Send, // + Message
    // M::Result: Send,
{
    // msg: Option<M>,
    // tx: Option<oneshot::Sender<Rsp>>,
    msg: Option<(M, oneshot::Sender<Rsp>)>
}

#[async_trait::async_trait]
impl<A, M> AsyncEnvelopeProxy<A> for AsyncEnvelopeReal<M, A::Response>
where
    M: Send + 'static, // + Message,
    // M::Result: Send,
    A: AsyncHandler<M> + Send,
    // A::Context: AsyncContext<A>,
{
    async fn handle(&mut self, act: &mut A) {

        if let Some((msg, tx)) = self.msg.take() {
            if tx.is_closed() {
                return;
            }
            
            let rsp = <A as AsyncHandler<M>>::handle(act, msg).await;
            let _r = tx.send(rsp);

            // let fut = <A as Handler<M>>::handle(act, msg);
            // fut.handle(ctx, tx)
        }
    }
}




// refer from https://stackoverflow.com/questions/70746671/how-to-bind-lifetimes-of-futures-to-fn-arguments-in-rust
pub trait XFn1<'a, I: 'a, O> {
    type Output: Future<Output = O> + 'a + Send;
    fn call_me(&self, session: I) -> Self::Output;
  }
  
impl<'a, I: 'a, O, F, Fut> XFn1<'a, I, O> for F
    where
    F: Fn(I) -> Fut,
    Fut: Future<Output = O> + 'a + Send,
{
    type Output = Fut;
    fn call_me(&self, x: I) -> Fut {
        self(x)
    }
}

pub trait XFn2<'a, I1: 'a, I2: 'a, O> {
    type Output: Future<Output = O> + 'a + Send;
    fn call_me(&self, x1: I1, x2: I2) -> Self::Output;
  }
  
impl<'a, I1: 'a, I2: 'a, O, F, Fut> XFn2<'a, I1, I2, O> for F
    where
    F: Fn(I1, I2) -> Fut,
    Fut: Future<Output = O> + 'a + Send,
{
    type Output = Fut;
    fn call_me(&self, x1: I1, x2: I2) -> Fut {
        self(x1, x2)
    }
}



// use std::marker::PhantomData;

// use futures::Future;
// use tracing::{info, warn};
// use anyhow::{Result, anyhow, Context as AnyhowContext};
// use tokio::{ task::JoinHandle, sync::{mpsc, oneshot}};


// use crate::spawn_with_name;



// pub fn start_actor<E, F1, F2, O1, F3, Req, Rsp>(
//     entity: E, 
//     wait_next: F1,      // async fn wait_next(&mut E) -> O1
//     handle_next: F2,    // async fn handle_next(&mut E, O1) -> Result<()>
//     handle_invoke: F3,  // async fn handle_invoke(&mut UdpEntity, Req) -> Rsp 
// ) -> (Invoker<Req, Rsp>, Wait4Completed<E>)
// where
//     E: ActorEntity ,

//     for<'a> F1: XFn1<'a, &'a mut E, O1> + Send + Sync + 'static + Copy,
//     for<'a> <F1 as XFn1<'a, &'a mut E, O1>>::Output: 'a,

//     for<'a> F2: XFn2<'a, &'a mut E, O1, Result<()>> + Send + Sync + 'static + Copy,
//     for<'a> <F2 as XFn2<'a, &'a mut E, O1, Result<()>>>::Output: 'a,

//     for<'a> F3: XFn2<'a, &'a mut E, Req, Rsp> + Send + Sync + 'static + Copy,
//     for<'a> <F3 as XFn2<'a, &'a mut E, Req, Rsp>>::Output: 'a,

//     O1: Send + 'static,
//     Req: Send + 'static,
//     Rsp: Send + 'static,
// {
//     let (op_tx, op_rx) = mpsc::channel(128);

//     let mut task = ActorTask {
//         op_rx,
//         actor: Actor {
//             entity,
//             wait_next,
//             handle_next,
//             handle_invoke,
//             none: PhantomData::default(),
//             none2: PhantomData::default(),
//             none3: PhantomData::default(),
//         },
//     };
    
//     let name = task.actor.entity.name();
//     let task_handle = spawn_with_name(name, async move {
//         let r = run_actor(&mut task).await;
//         if let Err(e) = r {
//             warn!("finish with err [{:?}]", e)
//         }
//         task.actor.entity
//     });
    
//     (Invoker { op_tx}, Wait4Completed{ task_handle })
// }




// pub trait ActorEntity: Send + 'static{
//     fn name(&self) -> String;
// }

// pub struct Wait4Completed<E> {
//     task_handle: JoinHandle<E>,
// }

// impl<E> Wait4Completed<E> {
//     pub async fn wait_for_completed(self) -> Result<E> {
//         let entity = self.task_handle.await?;
//         Ok(entity)
//     }
// }

// #[derive(Clone)]
// pub struct Invoker<Req, Rsp> {
//     op_tx: mpsc::Sender<Op<Req, Rsp>>,
//     // none: PhantomData<A>,
// }

// impl<Req, Rsp> Invoker<Req, Rsp> {

//     pub async fn invoke(&self, req: Req) -> Result<Rsp> 
//     {
//         let (tx, rx) = oneshot::channel();
//         self.op_tx.send(Op::Invoke(req, tx)).await
//         .map_err(|_x|anyhow!("send request error"))?;
//         let rsp = rx.await.with_context(||"recv response but error")?;
//         Ok(rsp)
//     }

//     pub async fn shutdown(&self) {
//         let _r = self.op_tx.send(Op::Shutdown).await
//         .map_err(|_x|anyhow!("send request error"));
//     }
// }


// async fn run_actor<E, F1, O1, F2, F3, Req, Rsp>(task: &mut ActorTask<E, F1, F2, O1, F3, Req, Rsp>) -> Result<()>
// where
//     E: ActorEntity ,

//     for<'a> F1: XFn1<'a, &'a mut E, O1> + Send + Sync + 'static + Copy,
//     for<'a> <F1 as XFn1<'a, &'a mut E, O1>>::Output: 'a,

//     for<'a> F2: XFn2<'a, &'a mut E, O1, Result<()>> + Send + Sync + 'static + Copy,
//     for<'a> <F2 as XFn2<'a, &'a mut E, O1, Result<()>>>::Output: 'a,

//     for<'a> F3: XFn2<'a, &'a mut E, Req, Rsp> + Send + Sync + 'static + Copy,
//     for<'a> <F3 as XFn2<'a, &'a mut E, Req, Rsp>>::Output: 'a,

//     O1: Send + 'static,
//     Req: Send + 'static,
//     Rsp: Send + 'static,
// {

//     loop {
//         tokio::select! {
//             r = task.actor.wait_next.call_me(&mut task.actor.entity) => {
//                 task.actor.handle_next.call_me(&mut task.actor.entity, r).await?;
//             }
//             r = task.op_rx.recv() => {
//                 match r {
//                     Some(op) => {
//                         let shutdown = handle_op(task.actor.handle_invoke, &mut task.actor.entity, op).await?;
//                         if shutdown {
//                             info!("got shutdown");
//                             break;
//                         }
//                     },
//                     None => {
//                         info!("no one care, done");
//                         break;
//                     }
//                 }
//             },
//         }
//     }
//     Ok(())
// }

// async fn handle_op<E, F3, Req, Rsp>(func: F3, entity: &mut E, op: Op<Req, Rsp>) -> Result<bool>
// where
//     E: ActorEntity ,

//     for<'a> F3: XFn2<'a, &'a mut E, Req, Rsp> + Send + Sync + 'static + Copy,
//     for<'a> <F3 as XFn2<'a, &'a mut E, Req, Rsp>>::Output: 'a,

//     Req: Send + 'static,
//     Rsp: Send + 'static,
// {
//     match op {
//         Op::Shutdown => return Ok(true),
//         Op::Invoke(req, tx) => {
//             let rsp = func.call_me(entity, req).await;
//             let _r = tx.send(rsp);
//         }
//     }
    
//     Ok(false)
// }


// struct Actor<E, F1, F2, O1, F3, Req, Rsp> {
//     entity: E,
//     wait_next: F1,
//     handle_next: F2,
//     handle_invoke: F3,
//     none: PhantomData<O1>,
//     none2: PhantomData<Req>,
//     none3: PhantomData<Rsp>,
// }

// struct ActorTask<E, F1, F2, O1, F3, Req, Rsp> {
//     op_rx: mpsc::Receiver<Op<Req, Rsp>>,
//     actor: Actor<E, F1, F2, O1, F3, Req, Rsp>,
// }

// enum Op<Req, Rsp> {
//     Shutdown,
//     Invoke(Req, oneshot::Sender<Rsp>),
// }


// // refer from https://stackoverflow.com/questions/70746671/how-to-bind-lifetimes-of-futures-to-fn-arguments-in-rust
// pub trait XFn1<'a, I: 'a, O> {
//     type Output: Future<Output = O> + 'a + Send;
//     fn call_me(&self, session: I) -> Self::Output;
//   }
  
// impl<'a, I: 'a, O, F, Fut> XFn1<'a, I, O> for F
//     where
//     F: Fn(I) -> Fut,
//     Fut: Future<Output = O> + 'a + Send,
// {
//     type Output = Fut;
//     fn call_me(&self, x: I) -> Fut {
//         self(x)
//     }
// }

// pub trait XFn2<'a, I1: 'a, I2: 'a, O> {
//     type Output: Future<Output = O> + 'a + Send;
//     fn call_me(&self, x1: I1, x2: I2) -> Self::Output;
//   }
  
// impl<'a, I1: 'a, I2: 'a, O, F, Fut> XFn2<'a, I1, I2, O> for F
//     where
//     F: Fn(I1, I2) -> Fut,
//     Fut: Future<Output = O> + 'a + Send,
// {
//     type Output = Fut;
//     fn call_me(&self, x1: I1, x2: I2) -> Fut {
//         self(x1, x2)
//     }
// }

