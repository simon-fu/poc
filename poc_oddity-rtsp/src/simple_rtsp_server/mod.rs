/*
    - rtsp spec: https://datatracker.ietf.org/doc/html/rfc2326
        - transport 格式见 https://datatracker.ietf.org/doc/html/rfc2326#section-12.39

    TODO:
    - 支持 video only， audio only source
    - 检查请求头里的 session id 是否一致, 不一致返回 reply_session_not_found
    - reply play with RtpInfo
    - 支持 udp 传输
    - mem data 的 sdp 里 audio AAC 的 fmtp 等参数动态生成
*/

mod simple_rtsp_server;
pub use simple_rtsp_server::*;

mod rtp_mem;
mod demo;
pub use demo::*;

