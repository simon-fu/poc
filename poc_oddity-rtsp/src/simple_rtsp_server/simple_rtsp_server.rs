

use std::{fmt, sync::Arc};

use anyhow::{Context, Result};
use bytes::Bytes;
use futures::{Future, SinkExt};
use oddity_rtsp_protocol::{AsServer, Channel, Codec, Lower, MaybeInterleaved, Method, Parameter, Range, Request, Response, Status, Transport};
use rand::Rng;
use tokio::net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::codec::{self, FramedRead, FramedWrite};
use tracing::{debug, warn};


pub async fn run_simple_rtsp_server<C: RtspServerCallback>(listener: TcpListener, callback: C) -> Result<()>  {

    let supported_methods = SupportedMethods::new(vec![
        Method::Options,
        Method::Describe,
        Method::Setup,
        Method::Play,
        Method::Teardown,
    ]);

    let shared = Arc::new(ServerShared {
        callback,
        supported_methods_str: supported_methods.to_header_value(),
        supported_methods,
    });

    loop {
        let (socket, addr) = listener.accept().await
        .with_context(||"accept failed")?;
        debug!("connected from [{addr}]");

        let mut conn = Connection::new(socket, shared.clone());

        spawn_with_name(addr.to_string(), async move {
            let r = conn_task(&mut conn).await;
            debug!("finished with [{r:?}]");
        });
    }
}


struct ServerShared<C> {
    callback: C,
    supported_methods: SupportedMethods,
    supported_methods_str: String,
}

pub struct RtspPlayDesc<S> {
    pub sdp: Bytes,
    pub num_tracks: usize,
    pub source: S,
}

pub type OnRequestPlayResult<S> = Result<Option<RtspPlayDesc<S>>>;
pub trait RtspServerCallback: Send + Sync + 'static {
    
    type MediaSource: RtspMediaSource + Send + Sync + 'static;

    fn on_request_play(&self, path: &str) -> impl Future<Output = OnRequestPlayResult<Self::MediaSource>> + Send + Sync + 'static;
}



pub trait RtspMediaSource {

    // return track index
    fn on_setup_track(&mut self, control: &str) -> Option<usize>;
    
    // async fn on_start_play(&mut self) -> Result<()>; 
    fn on_start_play(&mut self) -> impl Future<Output = Result<()>> + Send + Sync;

    // async fn on_inbound_rtp(&mut self, packet: RtspChPacket) -> Result<()>;
    fn on_inbound_rtp(&mut self, packet: RtspChPacket) -> impl Future<Output = Result<()>> + Send + Sync;

    // Must be CANCEL SAFETY
    // async fn read_outbound_rtp(&mut self) -> Result<Option<RtspChPacket>>;
    fn read_outbound_rtp(&mut self) -> impl Future<Output = Result<Option<RtspChPacket>> > + Send + Sync;
}

#[derive(Clone)]
pub struct RtspChPacket { 
    pub ch_id: u8,
    pub data: Bytes,
}

impl fmt::Display for RtspChPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ch {}, len {}", self.ch_id, self.data.len())
    }
}


async fn conn_task<C>(conn: &mut Connection<C>) ->Result<()> 
where
    C: RtspServerCallback,
{
    let mut state = State::<C>::Probing(Probe{});
    while !conn.is_teardown() {
        state = match state {
            State::Probing(s) => s.serve(conn).await?,
            State::PrePlaying(s) => s.serve(conn).await?,
            State::InPlaying(s) => s.serve(conn).await?,
            State::TearDown => break,
        };
    }
    Ok(())
}

type Inbound = FramedRead<OwnedReadHalf, Codec<AsServer>>;
type Outbound = FramedWrite<OwnedWriteHalf, Codec<AsServer>>;

struct Connection<C> {
    sid: SessionId,
    inbound: Inbound,
    outbound: Outbound,
    packet: Option<MaybeInterleaved<Request>>,
    is_teardown: bool,
    shared: Arc<ServerShared<C>>,
}

impl<C> Connection<C> {
    pub fn new(socket: TcpStream, shared: Arc<ServerShared<C>>) -> Self {
        let (read, write) = socket.into_split();
        Self { 
            sid: SessionId::generate(),
            inbound: codec::FramedRead::new(read, Codec::<AsServer>::new()),
            outbound: codec::FramedWrite::new(write, Codec::<AsServer>::new()),
            packet: None,
            is_teardown: false,
            shared,
        }
    }

    pub async fn send_response(&mut self,  msg: Response) -> Result<()> {
        debug!("S -> C: Response {msg}");
        self.outbound.send(MaybeInterleaved::Message(msg)).await
        .with_context(||"send response failed")?;
        Ok(())

        // send_msg(&mut self.outbound, msg).await
    }

    pub fn is_teardown(&self) -> bool {
        self.is_teardown
    }

    pub async fn send_interleave(&mut self, packet: RtspChPacket) -> Result<()> {
        // debug!("S -> C: Interleaved ch {}, len {}", packet.ch_id, packet.data.len());
        self.outbound.send(MaybeInterleaved::Interleaved{channel: packet.ch_id, payload: packet.data}).await?;
        Ok(())
    }

    pub async fn read_request(&mut self) -> Result<Option<Request>> {
        while !self.is_teardown() {
            self.wait_packet().await?;
            let r = self.next_request().await?;
            if let Some(req) = r {
                return Ok(Some(req))
            }
        }
        Ok(None)
    }

    // CANCEL SAFETY
    pub async fn wait_packet(&mut self) -> Result<()> {
        let packet = self.inbound.next().await
        .with_context(||"read packet but got None")?
        .with_context(||"read packet but connection broken")?;

        debug!("S <- C: {packet}");
        
        self.packet = Some(packet);
        Ok(())
    }

    pub async fn next_packet(&mut self) -> Result<Option<MaybeInterleaved<Request>>> {
        if let Some(packet) = self.packet.take() {
            match &packet {
                MaybeInterleaved::Message(req) => {
                    if !is_request_require_supported(&req) {
                        let rsp = reply_option_not_supported(&req);
                        self.send_response(rsp).await?;
                        return Ok(None)
                    }

                    if !self.shared.supported_methods.is_supported(&req.method) {
                        let rsp = reply_method_not_supported(req);
                        self.send_response(rsp).await?;
                        return Ok(None)
                    }

                    match req.method {
                        Method::Options => {
                            let rsp = reply_to_options_with_supported_methods_value(req, &self.shared.supported_methods_str);

                            self.send_response(rsp).await?;
                            return Ok(None)
                        }
                        Method::Teardown => {
                            self.reply_teardown(req).await?;
                            return Ok(None)
                        }
                        Method::Redirect => {
                            // tracing::trace!("handling REDIRECT request");
                            let rsp = reply_method_not_valid(req);
                            self.send_response(rsp).await?;
                            return Ok(None)
                        }
                        _ => {}
                    }                    
                },
                MaybeInterleaved::Interleaved { channel: _, payload:_ } => {},
            }
            return Ok(Some(packet))
        }
        Ok(None)
    }

    async fn next_request(&mut self) -> Result<Option<Request>> {
        if let Some(packet) = self.next_packet().await? {
            match packet {
                MaybeInterleaved::Message(req) => {
                    return Ok(Some(req))
                },
                MaybeInterleaved::Interleaved { channel, payload } => {
                    let len = payload.len();
                    tracing::debug!("ignored received interleaved data, channel {channel}, len {len}");
                },
            }
        }
        Ok(None)
    }
    
    async fn reply_teardown(&mut self, request: &Request) -> Result<()> {
        self.is_teardown = true;
        let rsp = reply_to_teardown(request);
        // ignore error for that client had closed connection after sending teardown
        let _r = self.send_response(rsp).await;
        Ok(())

    }

}

enum State<C: RtspServerCallback> {
    Probing(Probe),
    PrePlaying(PrePlaying<C::MediaSource>),
    InPlaying(InPlaying<C::MediaSource>),
    TearDown,
}

// trait StateHandler<C: RtspServerCallback> {
//     fn handel_request(&mut self,  conn: &mut Connection<C>) ->Result<Response> ;
// }



struct Probe {

}

impl Probe {

    pub async fn serve<C>(self, conn: &mut Connection<C>) ->Result<State<C>> 
    where
        C: RtspServerCallback,
    {
        'outter: while let Some(req) = conn.read_request().await? {

            debug!("probe: recv req {}", req.method);

            let request = &req;
            match request.method {
                Method::Describe => {
                    if !is_request_one_of_content_types_supported(request) {
                        debug!(
                            %request,
                            "none of content types accepted by client are supported, \
                             server only supports `application/sdp`",
                          );
                        let rsp = reply_not_acceptable(request);
                        conn.send_response(rsp).await?;
                        continue 'outter;
                    }

                    let r = conn.shared.callback.on_request_play(request.path()).await?;
                    let desc = match r {
                        Some(desc) => desc,
                        None => {
                            let rsp = reply_not_found(request);
                            conn.send_response(rsp).await?;
                            continue 'outter;
                        },
                    };

                    if let Ok(s) = std::str::from_utf8(&desc.sdp) {
                        debug!("sdp={s}");
                    }

                    let rsp = reply_to_describe_with_media_sdp(request, desc.sdp);
                    conn.send_response(rsp).await?;
    
                    return Ok(State::PrePlaying(PrePlaying {
                        base_path: request.path().to_string(),
                        channels: vec![Xtrans::Empty; desc.num_tracks << 1],
                        session: desc.source,
                        num_setup_tracks: 0,
                    }));    
                },
                _ => {
                    let rsp = reply_method_not_valid(request);
                    conn.send_response(rsp).await?;
                }
            };
        }
    
        Ok(State::TearDown)
    }
}

#[derive(Clone)]
enum Xtrans {
    Empty,
    Interleaved(u8),
}

struct PrePlaying<S> {
    base_path: String,
    channels: Vec<Xtrans>,
    num_setup_tracks: usize,
    session: S,
}

impl<S> PrePlaying<S> 
where
    S: RtspMediaSource
{
    pub async fn serve<C>(mut self, conn: &mut Connection<C>) ->Result<State<C>> 
    where
        C: RtspServerCallback<MediaSource = S>,
    {        
        let mut got_playing = false;

        while let Some(req) = conn.read_request().await? {

            let request = &req;
            let rsp = match request.method {
                Method::Setup => {
                    self.handle_setup(conn, request)?
                },
                Method::Play => {
                    match request.range() {
                        Some(Ok(_)) | None => {
                            got_playing = true;
                            self.session.on_start_play().await?;
                            reply_to_play_nortp(request, Range::new_for_live())
                        },
                        
                        Some(Err(oddity_rtsp_protocol::Error::RangeUnitNotSupported { value: _ }))
                        | Some(Err(oddity_rtsp_protocol::Error::RangeTimeNotSupported { value: _ })) => {
                            // "client provided range header format that is not supported"
                            reply_not_implemented(request)
                        }
                        Some(Err(_error)) => {
                            // "failed to parse range header (bad request)"
                            reply_bad_request(request)
                        }
                    }
                },
                _ => {
                    reply_method_not_valid(request)
                }
            };
    
            // send_msg(&mut conn.outbound, rsp).await?;
            conn.send_response(rsp).await?;

            if got_playing {
                return Ok(State::InPlaying(InPlaying {
                    channels: self.channels,
                    session: self.session,
                }))
            }
        }
    
        Ok(State::TearDown)
    
    }

    fn handle_setup<C>(&mut self, conn: &mut Connection<C>, request: &Request) -> Result<Response> {
        let transports = match request.transport() {
            Ok(v) => v,
            Err(e) => {
                // If the client did not provide a valid transport header value, then there
                // no way to reach it and we return "Unsupported Transport".
                warn!("setup transport error: [{e:?}]");
                return Ok(reply_unsupported_transport(request))
            },
        };

        debug!(path = request.path(), ?transports, "resolved transport");

        let control = match strip_base_path(request.path(), &self.base_path) {
            Some(v) => v,
            None => {
                debug!("expect path start with [{}], but [{}]", self.base_path, request.path());
                return Ok(reply_bad_request(request))
            },
        };

        let control = control.trim();

        let track_index = if control.is_empty() {
            self.num_setup_tracks
        } else {
            match self.session.on_setup_track(control) {
                Some(d) => d,
                None => {
                    debug!("NOT found track from control [{control}]");
                    return Ok(reply_bad_request(request))
                },
            }
        };

        let mut ch_index = track_index << 1;
        if ch_index >= self.channels.len() {
            debug!("exceed range of track index [{track_index}]");
            return Ok(reply_bad_request(request))
        }

        let end_ch_index = ch_index + 2;

        let mut selected_transport = None;
        for item in transports {
            if item.lower_protocol() == Some(&Lower::Tcp) {

                for param in item.parameters_iter() {
                    if let Parameter::Interleaved(v) = param {
                        match v {
                            Channel::Single(cid) => {
                                if ch_index < end_ch_index {
                                    self.channels[ch_index] = Xtrans::Interleaved(*cid);
                                    ch_index += 1;
                                }
                            }
                            Channel::Range(start, end) => {
                                let mut cid = *start;
                                while ch_index < end_ch_index && cid < *end {
                                    self.channels[ch_index] = Xtrans::Interleaved(cid);
                                    ch_index += 1;
                                    cid += 1;
                                }
                            }
                        }
                    }
                }
                if ch_index > (end_ch_index - 2) {
                    selected_transport = Some(item);
                    break;    
                }
            }
        }
        

        let xtrans = match selected_transport {
            Some(v) => v,
            None => return Ok(reply_unsupported_transport(request)),
        };

        let rsp = reply_to_setup(request, &conn.sid, &xtrans);
        self.num_setup_tracks += 1;
        Ok(rsp)

    }
}

fn strip_base_path<'a>(path: &'a str, base: &str) -> Option<&'a str> {
    let s1 = path.strip_prefix(base)?;
    if let Some(s2) = s1.strip_prefix("/") {
        return Some(s2)
    }
    Some(s1)
}

struct InPlaying<S> {
    channels: Vec<Xtrans>,
    session: S,
}

impl<S> InPlaying<S> 
where
    S: RtspMediaSource,
{
    pub async fn serve<C>(mut self, conn: &mut Connection<C>) ->Result<State<C>> 
    where
        C: RtspServerCallback<MediaSource = S>,
    {

        loop {
            if conn.is_teardown() {
                return Ok(State::TearDown)
            }
            tokio::select! {
                r = conn.wait_packet() => {
                    r?;
                    if let Some(packet) = conn.next_packet().await? {
                        self.handle_inbound_packet(conn, packet).await?;
                    }
                }
                r = self.session.read_outbound_rtp() => {
                    let packet = r?;
                    match packet {
                        Some(packet) => {
                            self.handle_outbound_packet(conn, packet).await?;
                        },
                        None => {
                            // TODO: send end of stream 
                            break;
                        }
                    }
                }
            }            
        }

        Ok(State::TearDown)
    }

    async fn handle_outbound_packet<C>(&mut self, conn: &mut Connection<C>, mut packet: RtspChPacket) -> Result<()> {
        if let Some(ch) = self.channels.get(packet.ch_id as usize) {
            match ch {
                Xtrans::Empty => {},
                Xtrans::Interleaved(ch_id) => {
                    packet.ch_id = *ch_id;
                    conn.send_interleave(packet).await?;
                },
            }
        }
        Ok(())
    }

    async fn handle_inbound_packet<C>(&mut self, conn: &mut Connection<C>, packet: MaybeInterleaved<Request>) -> Result<()> {
        match packet {
            MaybeInterleaved::Message(req) => {
                self.handle_inbound_req(conn, &req).await
            },
            MaybeInterleaved::Interleaved { channel, payload } => {                
                let rtp = RtspChPacket {
                    ch_id: channel,
                    data: payload,
                };
                self.session.on_inbound_rtp(rtp).await?;
                Ok(())
            },
        }
    }

    async fn handle_inbound_req<C>(&mut self, conn: &mut Connection<C>, request: &Request) -> Result<()> {
        let rsp = match request.method {
            _ => {
                reply_method_not_valid(request)
            }
        };
        // let rsp = send_msg(&mut conn.outbound, rsp).await?;
        conn.send_response(rsp).await?;
        Ok(())
    }
    
}






// fn hardcode_sdp_content() -> String {
// "v=0
// o=- 0 0 IN IP4 127.0.0.1
// s=Big Buck Bunny, Sunflower version
// c=IN IP4 127.0.0.1
// t=0 0
// m=video 0 RTP/AVP 96
// b=AS:640
// a=rtpmap:96 H264/90000
// a=fmtp:96 packetization-mode=1; sprop-parameter-sets=Z0LAFdoCAJbARAAAAwAEAAADAPA8WLqA,aM4yyA==; profile-level-id=42C015
// m=audio 0 RTP/AVP 97
// b=AS:96
// a=rtpmap:97 MPEG4-GENERIC/48000/2
// a=fmtp:97 profile-level-id=1;mode=AAC-hbr;sizelength=13;indexlength=3;indexdeltalength=3; config=119056E500".into()
// // a=control:streamid=0
// }

struct SupportedMethods {
    methods: Vec<Method>,
}

impl SupportedMethods {
    pub fn new(methods: Vec<Method>) -> Self {
        Self { methods }
    }

    pub fn is_supported(&self, method: &Method) -> bool {
        for item in self.methods.iter() {
            if item == method {
                return true;
            }
        }
        false
    }

    pub fn to_header_value(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for SupportedMethods {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        
        if let Some(last) = self.methods.last() {
            
            let len_minus_1 = self.methods.len()-1;
            for method in &self.methods[..len_minus_1] {
                write!(f, "{method}, ")?;
            }

            write!(f, "{last}")?;
        }

        Ok(())
    }
}

static SERVER: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[inline]
fn is_request_require_supported(request: &Request) -> bool {
    // We don't support any features at this point
    request.require().is_none()
}

#[inline]
fn is_request_one_of_content_types_supported(request: &Request) -> bool {
    // We only support SDP
    request.accept().contains(&"application/sdp")
}


// #[inline]
// fn reply_to_options_with_supported_methods(request: &Request) -> Response {
//     reply_to_options_with_supported_methods_value(request, "OPTIONS, DESCRIBE, SETUP, PLAY, TEARDOWN")
// }

#[inline]
fn reply_to_options_with_supported_methods_value(request: &Request, val: impl ToString) -> Response {
    Response::ok()
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .with_header("Public", val)
        .build()
}

#[inline]
fn reply_to_describe_with_media_sdp(request: &Request, contents: Bytes) -> Response {
    Response::ok()
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        // .with_sdp(sdp_contents)
        .with_body(Bytes::from(contents), "application/sdp")
        .build()
}

#[inline]
fn reply_to_setup(request: &Request, session_id: &SessionId, transport: &Transport) -> Response {
    Response::ok()
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .with_header("Session", session_id)
        .with_header("Transport", transport)
        .build()
}

#[inline]
fn reply_to_teardown(request: &Request) -> Response {
    Response::ok()
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .build()
}

#[inline]
fn reply_to_play_nortp(request: &Request, range: Range) -> Response {
    Response::ok()
        .with_cseq_of(request)
        // .with_rtp_info([rtp_info])
        .with_header("Server", SERVER)
        .with_header("Range", range)
        .build()
}

// #[inline]
// fn reply_to_play(request: &Request, range: Range, rtp_info: RtpInfo) -> Response {
//     Response::ok()
//         .with_cseq_of(request)
//         .with_rtp_info([rtp_info])
//         .with_header("Server", SERVER)
//         .with_header("Range", range)
//         .build()
// }

#[inline]
fn reply_bad_request(request: &Request) -> Response {
    Response::error(Status::BadRequest)
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .build()
}

#[inline]
fn reply_option_not_supported(request: &Request) -> Response {
    tracing::debug!(
    %request,
    "client asked for feature that is not supported");
    Response::error(Status::OptionNotSupported)
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .build()
}

#[inline]
fn reply_method_not_supported(request: &Request) -> Response {
    tracing::warn!(
    %request,
    method = %request.method,
    "client sent unsupported request");

    Response::error(Status::MethodNotAllowed)
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .build()
}

#[inline]
fn reply_method_not_valid(request: &Request) -> Response {
    tracing::warn!(
    %request,
    method = %request.method,
    "client tried server-only method in request to server; \
    does client think it is server?");
    Response::error(Status::MethodNotValidInThisState)
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .build()
}

#[inline]
fn reply_not_acceptable(request: &Request) -> Response {
    tracing::debug!(
    %request,
    "server does not support a presentation format acceptable \
    by client");
    Response::error(Status::NotAcceptable)
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .build()
}

#[inline]
fn reply_not_found(request: &Request) -> Response {
    // tracing::debug!(
    // %request,
    // path = request.path(),
    // "path not registered as media item");
    Response::error(Status::NotFound)
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .build()
}

// #[inline]
// fn reply_aggregate_operation_not_allowed(request: &Request) -> Response {
//     tracing::debug!(
//     %request,
//     "refusing to do aggregate request");
//     Response::error(Status::AggregateOperationNotAllowed)
//         .with_cseq_of(request)
//         .with_header("Server", SERVER)
//         .build()
// }

#[inline]
fn reply_unsupported_transport(request: &Request) -> Response {
    tracing::debug!(
    %request,
    "unsupported transport");
    Response::error(Status::UnsupportedTransport)
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .build()
}

// #[inline]
// fn reply_session_not_found(request: &Request) -> Response {
//     tracing::debug!(
//     %request,
//     "session not found");
//     Response::error(Status::SessionNotFound)
//         .with_cseq_of(request)
//         .with_header("Server", SERVER)
//         .build()
// }

#[inline]
fn reply_not_implemented(request: &Request) -> Response {
    tracing::debug!(
    %request,
    "not implemented");
    Response::error(Status::NotImplemented)
        .with_cseq_of(request)
        .with_header("Server", SERVER)
        .build()
}

// #[inline]
// fn reply_header_field_not_valid(request: &Request) -> Response {
//     tracing::debug!(
//     %request,
//     "header field not valid");
//     Response::error(Status::HeaderFieldNotValid)
//         .with_cseq_of(request)
//         .with_header("Server", SERVER)
//         .build()
// }

// #[inline]
// fn reply_internal_server_error(request: &Request) -> Response {
//     Response::error(Status::InternalServerError)
//         .with_cseq_of(request)
//         .with_header("Server", SERVER)
//         .build()
// }


#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    const SESSION_ID_LEN: u32 = 8;

    pub fn generate() -> SessionId {
        SessionId(
            rand::thread_rng()
                .sample(rand::distributions::Uniform::from(
                    10_u32.pow(Self::SESSION_ID_LEN - 1)..10_u32.pow(Self::SESSION_ID_LEN),
                ))
                .to_string(),
        )
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for SessionId {
    fn from(session_id: &str) -> Self {
        SessionId(session_id.to_string())
    }
}



#[inline]
pub fn spawn_with_name<I, T>(name: I, fut: T) -> tokio::task::JoinHandle<T::Output>
where
    I: Into<String>,
    T: std::future::Future + Send + 'static,
    T::Output: Send + 'static,
{
    let name:String = name.into();
    let span = tracing::span!(parent:None, tracing::Level::INFO, "", s = &name[..]);
    tokio::spawn(tracing::Instrument::instrument(fut, span))
}



