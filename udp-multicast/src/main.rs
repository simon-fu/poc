
use std::{net::{SocketAddr, Ipv4Addr, Ipv6Addr}, str::FromStr};
use anyhow::{Result, Context};
use tokio::net::UdpSocket;
use tracing::info;


#[tokio::main]
async fn main() -> Result<()>{
    tracing_subscriber::fmt().init();
    let multi_addr = "236.27.138.11:1234";
    // let if_addr = Some("192.168.50.229");
    // let if_addr = Some("127.0.0.1");
    let if_addr = None;

    let socket= bind_multicast(multi_addr, if_addr)?;
    let socket = UdpSocket::from_std(socket)?;

    let mut buf = vec![0_u8; 2000];
    loop {
        let (n, remote_addr) = socket.recv_from(&mut buf).await?;
        if n == 0 {
            info!("recv zero udp, remote [{}]", remote_addr);
            break;
        }

        info!("recv udp from [{}], bytes [{}]", remote_addr, n)
    }
    Ok(())
}

/// Bind socket to multicast address with IP_MULTICAST_LOOP and SO_REUSEADDR Enabled
/// from https://github.com/henninglive/tokio-udp-multicast-chat/blob/master/src/main.rs
/// from https://github.com/bluejekyll/multicast-example/blob/master/src/lib.rs
fn bind_multicast(
    multi_addr: &str,
    if_addr: Option<&str>,
) -> Result<std::net::UdpSocket> {
    use socket2::{Domain, Type, Protocol, Socket};

    let multi_addr : SocketAddr = multi_addr.parse()
    .with_context(||format!("invalid socket addr [{}]", multi_addr))?;

    


    match multi_addr {
        SocketAddr::V4(multi_addr) => { 
            
            let domain = Domain::IPV4;
            // let interface = Ipv4Addr::new(0, 0, 0, 0);

            let interface : Option<Ipv4Addr> = parse_opt(&if_addr)?;

            let interface= interface.unwrap_or_else(||Ipv4Addr::new(0, 0, 0, 0));

            let if_addr = SocketAddr::new(interface.into(), multi_addr.port());

            let socket = Socket::new(
                domain,
                Type::DGRAM,
                Some(Protocol::UDP),
            )?;
            socket.set_reuse_address(true)?;
            // socket.bind(&socket2::SockAddr::from(if_addr))?;
            socket.bind(&socket2::SockAddr::from(multi_addr))?;
            // socket.set_multicast_loop_v4(false)?;  // 好像没什么用
            info!("udp addr: multicast [{}], bind ipv4 [{:?}]", multi_addr, if_addr);

            // join to the multicast address, with all interfaces
            socket.join_multicast_v4(
                multi_addr.ip(),
                &interface,
            )?;

            Ok(socket.into())
        },
        SocketAddr::V6(multi_addr) => {
            
            let domain = Domain::IPV6;
            // let interface = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0);

            let interface : Option<Ipv6Addr> = parse_opt(&if_addr)?;

            let interface= interface.unwrap_or_else(||Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0));

            let if_addr = SocketAddr::new(interface.into(), multi_addr.port());

            let socket = Socket::new(
                domain,
                Type::DGRAM,
                Some(Protocol::UDP),
            )?;
            // reuse address 是允许多个进程监听同一个地址:端口，但是同一个进程绑定两次会有问题？
            // reuse port 是多个socket负载均衡
            // 参考： https://stackoverflow.com/questions/14388706/how-do-so-reuseaddr-and-so-reuseport-differ
            socket.set_reuse_address(true)?;
            // socket.bind(&socket2::SockAddr::from(if_addr))?;
            socket.bind(&socket2::SockAddr::from(multi_addr))?;
            // socket.set_multicast_loop_v6(true)?;
            info!("udp addr: multicast [{}], bind ipv6 [{:?}]", multi_addr, if_addr);

            // join to the multicast address, with all interfaces (ipv6 uses indexes not addresses)
            socket.join_multicast_v6(
                multi_addr.ip(),
                0,
            )?;

            Ok(socket.into())
        },
    }
}


fn parse_opt<T>(addr: &Option<&str>) -> Result<Option<T>> 
where
    T: FromStr,
    <T as FromStr>::Err: Send + Sync + std::error::Error + 'static,
{
    let r = addr.map(|x|x.parse::<T>())
    .map_or(Ok(None), |x| x.map(Some))?;
    Ok(r)
}
