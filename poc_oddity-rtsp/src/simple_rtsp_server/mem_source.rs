use std::error;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::Future;
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
    
    fn on_start_play(&mut self) -> impl Future<Output = Result<()>> + Send + Sync {
        async {
            Ok(())
        }
    }

    // Must be CANCEL SAFETY
    fn read_outbound_rtp(&mut self) -> impl Future<Output = Result<Option<RtspChPacket>> > + Send + Sync {
        async {
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
    
            Ok(None)
            // loop {
            //     tokio::time::sleep(std::time::Duration::MAX/4).await;
            // }
        }
        
    }

    fn on_inbound_rtp(&mut self, packet: RtspChPacket) -> impl Future<Output = Result<()>> + Send + Sync {
        async move {
            tracing::debug!("received rtp, {packet}");
            Ok(())
        }
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
