use std::{net::SocketAddr};
use anyhow::Result;
use tokio::{net::UdpSocket};

use super::{ActorEntity, Invoker, Wait4Completed, start_actor, AsyncHandler};


pub struct UdpInvoker(Invoker<UdpEntity>);

impl UdpInvoker {
    pub async fn call_i32(&self, msg: i32) -> Result<i32> {
        let r = self.0.invoke(msg).await?;
        Ok(r)
    }
}

pub fn start_udp_actor(socket: UdpSocket) -> (UdpInvoker, Wait4Completed<UdpEntity>) {
    let entity = UdpEntity {
        buf: vec![0; 1700],
        socket,
    };

    let r = start_actor(
        entity, 
        wait_next, 
        handle_next, 
        // handle_invoke,
    );

    (UdpInvoker(r.0), r.1)
}

pub struct UdpEntity {
    socket: UdpSocket,
    buf: Vec<u8>,
}


async fn wait_next(entity: &mut UdpEntity) -> Result<(usize, SocketAddr)> {
    entity.socket.recv_from(&mut entity.buf).await.map_err(|e|e.into())
}

async fn handle_next(_entity: &mut UdpEntity, _r: Result<(usize, SocketAddr)>) -> Result<()> {
    Ok(())
}

// async fn handle_invoke(_entity: &mut UdpEntity, _req: InvokeReq) -> InvokeRsp {
//     InvokeRsp{}
// }


// struct InvokeReq{}
// struct InvokeRsp{}

impl ActorEntity for UdpEntity {
    fn name(&self) -> String {
        "UdpActor".into()
    }
}

#[async_trait::async_trait]
impl AsyncHandler<i32> for UdpEntity {
    type Response = i32;

    async fn handle(&mut self, msg: i32) -> Self::Response {
        msg
    }
}
