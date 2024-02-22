/*
    - rtsp spec: https://datatracker.ietf.org/doc/html/rfc2326
        - transport 格式见 https://datatracker.ietf.org/doc/html/rfc2326#section-12.39

    TODO:
    - 检查请求头里的 session id 是否一致, 不一致返回 reply_session_not_found
    - reply play with RtpInfo
    - 支持 udp 传输
*/

use std::{fmt, sync::Arc};

use anyhow::{Context, Result};
use bytes::Bytes;
use futures::SinkExt;
// use futures::SinkExt;
use oddity_rtsp_protocol::{AsServer, Channel, Codec, Lower, MaybeInterleaved, Method, Parameter, Range, Request, Response, Status, Transport};
use rand::Rng;
use tokio::net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::codec::{self, FramedRead, FramedWrite};
use tracing::{debug, info, warn};

use self::source_util::{load_rtp_data, RtpMemData, RtpMemSource};

pub async fn run_demo() -> Result<()> {
    let listen_addr = "0.0.0.0:5554";

    let listener = TcpListener::bind(listen_addr).await
    .with_context(||format!("listen faild, [{listen_addr}]"))?;

    info!("rtsp listen on [{listen_addr}]");

    let supported_methods = SupportedMethods::new(vec![
        Method::Options,
        Method::Describe,
        Method::Setup,
        Method::Play,
        Method::Teardown,
    ]);

    let shared = Arc::new(ServerShared {
        callback: Example {
            data: load_rtp_data("/tmp/sample-data/sample.mp4").await?,
        },
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

struct Example {
    data: Arc<RtpMemData>,
}

impl RtspServerCallback for Example {
    type MediaSource = RtpMemSource;

    async fn on_request_play(&self, path: &str) -> Result<Option<RtspPlayDesc<Self::MediaSource>>> {        
        if path != "/example" {
            return Ok(None)
        }

        Ok(Some(RtspPlayDesc {
            sdp: self.data.sdp().clone(), // hardcode_sdp_content(),
            num_tracks: 2,
            source: self.data.make_source(),
        }))
    }
}



mod source_util {
    use std::error;
    use std::fmt;
    use std::sync::Arc;
    use std::time::Duration;

    use bytes::Bytes;
    use oddity_sdp_protocol::{CodecInfo, Direction, Kind, Protocol, TimeRange};
    use tokio::time::Instant;
    use video_rs::RtpBuf;
    use video_rs::RtpMuxer;
    use video_rs::StreamInfo;

    use crate::oddity_rtsp_server::media::video::reader::backend::make_reader_with_sane_settings;
    use crate::oddity_rtsp_server as thiz_root;
    use thiz_root::media::video::reader;
    use thiz_root::media::video::rtp_muxer;
    use thiz_root::media::MediaDescriptor;

    pub use oddity_sdp_protocol::Sdp;

    use anyhow::{Context, Result};

    use super::RtspChPacket;
    use super::RtspMediaSource;

    


    struct TsBase {
        instant: Instant,
        ts: Duration,
    }

    impl TsBase {
        fn check(&self, next: Duration) -> Option<Duration> {
            if next > self.ts {
                let ts_delta = next - self.ts;
                let elapsed = self.instant.elapsed();
                if ts_delta > elapsed {
                    return Some(ts_delta - elapsed)
                }
            }
            None
        }
    }
    

    pub struct RtpMemData {
        sdp: Bytes,
        video_packets: Vec<MemPacket>,
        audio_packets: Vec<MemPacket>,
        video_track: MemTrack,
        audio_track: MemTrack,
    }

    impl RtpMemData {
        pub fn sdp(&self) -> &Bytes {
            &self.sdp
        }

        pub fn make_source(self: &Arc<Self>) -> RtpMemSource {
            RtpMemSource {
                data: self.clone(),
                video_cursor: 0,
                audio_cursor: 0,
                ts_base: None,
            }
        }
    }

    pub struct RtpMemSource {
        data: Arc<RtpMemData>,
        video_cursor: usize,
        audio_cursor: usize,
        ts_base: Option<TsBase>,
    } 
    
    impl RtpMemSource {
        fn do_read_next(&mut self) -> Option<MemPacket> {
            let video = self.data.video_packets.get(self.video_cursor);
            let audio = self.data.audio_packets.get(self.audio_cursor);
            
            if let (Some(video), Some(audio)) = (video, audio) {
                if video.ts <= audio.ts {
                    self.video_cursor += 1;
                    return Some(video.clone())
                } else {
                    self.audio_cursor += 1;
                    return Some(audio.clone())
                }
            } else if let Some(video) = video {

                self.video_cursor += 1;
                return Some(video.clone())

            } else if let Some(audio) = audio {

                self.audio_cursor += 1;
                return Some(audio.clone())
            } else {
                return None
            }
        }
    }
    
    impl RtspMediaSource for RtpMemSource {
        fn on_setup_track(&mut self, control: &str) -> Option<usize> {
            if control == self.data.video_track.control {
                Some(self.data.video_track.info.index)
            } else  if control == self.data.audio_track.control {
                Some(self.data.audio_track.info.index)
            } else {
                None
            }
        }

        async fn on_start_play(&mut self) -> Result<()> {
            Ok(())
        }
    
        async fn read_outbound_rtp(&mut self) -> Result<Option<RtspChPacket>> {

            let r = self.do_read_next();
            if let Some(packet) = r {
                match &self.ts_base {
                    Some(ts_base) => {
                        if let Some(d) = ts_base.check(packet.ts) {
                            tokio::time::sleep(d).await;
                        }
                    },
                    None => {
                        self.ts_base = Some(TsBase {
                            instant: Instant::now(),
                            ts: packet.ts,
                        });
                    },
                }

                return Ok(Some(packet.packet.clone()))
            }

            loop {
                tokio::time::sleep(std::time::Duration::MAX/4).await;
            }
        }

        async fn on_inbound_rtp(&mut self, packet: RtspChPacket) -> Result<()> {
            tracing::debug!("received rtp, {packet}");
            Ok(())
        }
    }

    #[derive(Clone)]
    struct MemPacket {
        ts: Duration,
        packet: RtspChPacket,
    }

    impl MemPacket {
        fn from_rtp_buf(index: usize, ts: Duration, buf: RtpBuf) -> Self {
            let ch_id = (index << 1) as u8;

            let packet = match buf {
                RtpBuf::Rtp(data) => RtspChPacket {
                    ch_id: ch_id + 0,
                    data: Bytes::from(data),
                },
                RtpBuf::Rtcp(data) => RtspChPacket {
                    ch_id: ch_id + 1,
                    data: Bytes::from(data),
                },
            };
            Self {
                ts,
                packet,
            }
        }
    }
    
    #[tokio::test]
    async fn testp_poc() {
        let mut source  =load_rtp_data("/tmp/sample-data/sample.mp4").await.unwrap()
        .make_source();

        let mut num = 0_u64;
        while let Some(mem) = source.do_read_next() {
            num += 1;
            let milli = mem.ts.as_millis();
            let ch_id = mem.packet.ch_id;
            let data_len = mem.packet.data.len();
            println!("rtp {num}: milli {milli}, ch_id {ch_id}, len {data_len}");
        }
    }

    pub async fn load_rtp_data(filename: &str) -> Result<Arc<RtpMemData>> {
        // let filename = "/tmp/sample-data/sample.mp4";
        let source_name = "Big Buck Bunny";
        let source_descriptor = MediaDescriptor::File(filename.into());

        // let description = crate::oddity_rtsp_server::media::sdp::create(&source_name, &source_descriptor).await
        // .with_context(||format!("failed to create sdp from [{filename}]"))?;

        let mem_sdp = create_sdp(&source_name, &source_descriptor).await
        .with_context(||format!("failed to create sdp from [{filename}]"))?;

        println!("sdp: {}", mem_sdp.sdp);
        println!("best_stream.video {:?}, best_stream.audio {:?}", mem_sdp.best.video, mem_sdp.best.audio);
        

        let mut video_muxer = {
            let info = mem_sdp.best.video.info.clone();
            tokio::task::spawn_blocking(|| {
                let muxer = RtpMuxer::new()?;
                let muxer = muxer.with_stream(info)?;
                Result::<RtpMuxer, video_rs::Error>::Ok(muxer)
            }).await??
        };

        let mut audio_muxer = {
            let info = mem_sdp.best.audio.info.clone();
            tokio::task::spawn_blocking(|| {
                let muxer = RtpMuxer::new()?;
                let muxer = muxer.with_stream(info)?;
                Result::<RtpMuxer, video_rs::Error>::Ok(muxer)
            }).await??
        };

        // let mut reader = StreamReader::new(&source_descriptor).await
        // .with_context(||format!("failed to load [{filename}]"))?;
        let mut reader = make_reader_with_sane_settings(source_descriptor.clone().into()).await?;

        let max_frames = 99999999_u64;
        
        let (video_packets, audio_packets) = tokio::task::spawn_blocking(move || {
            let video_packets = {
                let name = "video";
                let index = mem_sdp.best.video.info.index;
                let muxer = &mut video_muxer;
                let mut packets = Vec::new();

                reader.seek_to_start()?;
                for num in 0..max_frames {
                    let frame = match reader.read(index){
                        Ok(v) => v,
                        Err(_e) => break,
                    };
                    let pts = Duration::from(frame.pts());
                    println!("{name} frame {num}: pts {}, dts {}", pts.as_millis(), frame.dts().as_secs());
                    let rtp_packets = muxer.mux(frame)?;
                    packets.extend(rtp_packets.into_iter().map(|x| MemPacket::from_rtp_buf(index, pts.clone(), x)));
                }
                println!("{name} rtp packets {}", packets.len());
                packets
            };

            let audio_packets = {
                let name = "audio";
                let index = mem_sdp.best.audio.info.index;
                let muxer = &mut audio_muxer;
                let mut packets = Vec::new();

                reader.seek_to_start()?;
                for num in 0..max_frames {
                    let frame = match reader.read(index){
                        Ok(v) => v,
                        Err(_e) => break,
                    };
                    let pts = Duration::from(frame.pts());
                    println!("{name} frame {num}: pts {}, dts {}", pts.as_millis(), frame.dts().as_secs());
                    let rtp_packets = muxer.mux(frame)?;
                    packets.extend(rtp_packets.into_iter().map(|x| MemPacket::from_rtp_buf(index, pts.clone(), x)));
                }
                println!("{name} rtp packets {}", packets.len());
                packets
            };

            Result::<_, video_rs::Error>::Ok((video_packets, audio_packets))
        }).await??;


        Ok(Arc::new(RtpMemData {
            sdp: mem_sdp.sdp.to_string().into(),
            video_packets,
            audio_packets,
            video_track: mem_sdp.best.video,
            audio_track: mem_sdp.best.audio,
        }))

    }

    struct MemTrack {
        info: StreamInfo,
        control: String,
    }

    impl fmt::Debug for MemTrack {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("MemTrack")
            .field("index", &self.info.index)
            .field("control", &self.control)
            .finish()
        }
    }

    struct BestStreamInfo {
        video: MemTrack,
        audio: MemTrack,
    }


    struct MemSdp {
        sdp: Sdp,
        best: BestStreamInfo,
    }

    async fn create_sdp(name: &str, descriptor: &MediaDescriptor) -> Result<MemSdp, SdpError> {

        const ORIGIN_DUMMY_HOST: [u8; 4] = [0, 0, 0, 0];
        const TARGET_DUMMY_HOST: [u8; 4] = [0, 0, 0, 0];
        const TARGET_DUMMY_PORT: u16 = 0;
    
        tracing::trace!("sdp: initializing reader");
        let reader = reader::backend::make_reader_with_sane_settings(descriptor.clone().into())
            .await
            .map_err(SdpError::Media)?;
        let best_video_stream = reader.best_video_stream_index().map_err(SdpError::Media)?;
        
        let video_info = reader.stream_info(best_video_stream)
        .map_err(SdpError::Media)?;


    
        let best_audio_stream = reader
            .input
            .streams()
            .best(ffmpeg_next::media::Type::Audio)
            .ok_or(ffmpeg_next::Error::StreamNotFound)
            .map_err(video_rs::Error::from)
            .map_err(SdpError::Media)?
            .index();

        let audio_info = reader.stream_info(best_audio_stream)
        .map_err(SdpError::Media)?;

        tracing::debug!(best_video_stream, best_audio_stream, video_info.index, audio_info.index, "sdp: initialized reader");
    
        let video_track = MemTrack {
            control: format!("streamid={}", video_info.index),
            info: video_info,
        };

        let audio_track = MemTrack {
            control: format!("streamid={}", audio_info.index),
            info: audio_info,
        };

        tracing::trace!("sdp: initializing muxer");
        let muxer = rtp_muxer::make_rtp_muxer()
            .await
            .and_then(|muxer| muxer.with_stream(reader.stream_info(best_video_stream)?))
            .map_err(SdpError::Media)?;
        tracing::trace!("sdp: initialized muxer");
    
        let (sps, pps) = muxer
            .parameter_sets_h264()
            .into_iter()
            // The `parameter_sets` function will return an error if the
            // underlying stream codec is not supported, we filter out
            // the stream in that case, and return `CodecNotSupported`.
            .filter_map(Result::ok)
            .next()
            .ok_or(SdpError::CodecNotSupported)?;
        tracing::trace!("sdp: found SPS and PPS");
    
        // Since the previous call to `parameter_sets_h264` can only
        // return a result if the underlying stream is H.264, we can
        // assume H.264 from this point onwards.
        let codec_info = CodecInfo::h264(sps, pps.as_slice(), muxer.packetization_mode());
    
        let sdp = Sdp::new(
            ORIGIN_DUMMY_HOST.into(),
            name.to_string(),
            TARGET_DUMMY_HOST.into(),
            // Since we support only live streams or playback on repeat,
            // all streams are basically "live".
            TimeRange::Live,
        );
    
        let mut sdp = sdp.with_media(
            Kind::Video,
            TARGET_DUMMY_PORT,
            Protocol::RtpAvp,
            codec_info,
            Direction::ReceiveOnly,
        );
        sdp.media[0].tags.push(oddity_sdp_protocol::Tag::Property(format!("control:{}", video_track.control)));
        // sdp.media[0].tags.push(oddity_sdp_protocol::Tag::Property("control:streamid=0".into()));
        
        let format = 96 + 1; // 97
        sdp.media.push(oddity_sdp_protocol::Media {
            kind: Kind::Audio,
            port: TARGET_DUMMY_PORT,
            protocol: Protocol::RtpAvp,
            format,
            tags: vec![
                oddity_sdp_protocol::Tag::Value(
                    "rtpmap".to_string(),
                    format!("{format} {}", "MPEG4-GENERIC/48000/2"),
                ),
                oddity_sdp_protocol::Tag::Value(
                    "fmtp".to_string(),
                    format!(
                        "{format} {}", 
                        "profile-level-id=1;mode=AAC-hbr;sizelength=13;indexlength=3;indexdeltalength=3; config=119056E500"
                    ),
                ),
                oddity_sdp_protocol::Tag::Property(Direction::ReceiveOnly.to_string()),
                oddity_sdp_protocol::Tag::Property(format!("control:{}", audio_track.control)),
                // oddity_sdp_protocol::Tag::Property("control:streamid=1".into()),
                // oddity_sdp_protocol::Tag::Property("control:1234".into()),
            ],
        });
    
        tracing::trace!(%sdp, "generated sdp");
        Ok(MemSdp {
            sdp,
            best: BestStreamInfo {
                video: video_track,
                audio: audio_track,
            },
        })
    }

    #[derive(Debug)]
    pub enum SdpError {
        CodecNotSupported,
        Media(video_rs::Error),
    }

    impl fmt::Display for SdpError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                SdpError::CodecNotSupported => write!(f, "codec not supported"),
                SdpError::Media(error) => write!(f, "media error: {}", error),
            }
        }
    }

    impl error::Error for SdpError {}
    
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

pub trait RtspServerCallback {
    
    type MediaSource: RtspMediaSource;

    async fn on_request_play(&self, path: &str) -> Result<Option<RtspPlayDesc<Self::MediaSource>>>;
}



pub trait RtspMediaSource {

    // return track index
    fn on_setup_track(&mut self, control: &str) -> Option<usize>;
    
    async fn on_start_play(&mut self) -> Result<()>; 

    async fn on_inbound_rtp(&mut self, packet: RtspChPacket) -> Result<()>;

    // Must be CANCEL SAFETY
    async fn read_outbound_rtp(&mut self) -> Result<Option<RtspChPacket>>;
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



