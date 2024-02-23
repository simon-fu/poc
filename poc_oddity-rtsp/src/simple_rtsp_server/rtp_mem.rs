
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use bytes::Bytes;
use oddity_sdp_protocol::{CodecInfo, Direction, Kind, Protocol, TimeRange};
use tokio::time::Instant;
use video_rs::Reader;
use video_rs::RtpBuf;
use video_rs::RtpMuxer;
use video_rs::StreamInfo;

use crate::oddity_rtsp_server::media::video::reader::backend::make_reader_with_sane_settings;
use crate::oddity_rtsp_server as thiz_root;
use thiz_root::media::MediaDescriptor;

pub use oddity_sdp_protocol::Sdp;

use anyhow::Result;




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


#[derive(Clone)]
pub struct RtpMemPacket {
    ts: Duration,
    ch_id: u8,
    data: Bytes,
}

impl RtpMemPacket {
    // pub fn time_milli(&self) -> i64 {
    //     self.ts.as_millis() as i64
    // }

    pub fn ch_id(&self) -> u8 {
        self.ch_id
    }

    pub fn data(&self) -> &Bytes {
        &self.data
    }
    
    fn from_rtp_buf(index: usize, ts: Duration, buf: RtpBuf) -> Self {
        let ch_id = (index << 1) as u8;

        match buf {
            RtpBuf::Rtp(data) => Self {
                ts,
                ch_id: ch_id + 0,
                data: Bytes::from(data),
            },
            RtpBuf::Rtcp(data) => Self {
                ts,
                ch_id: ch_id + 1,
                data: Bytes::from(data),
            },
        }
    }
}

pub struct RtpMemReader {
    data: Arc<RtpMemData>,
    cursor: RtpMemCursor,
    ts_base: Option<TsBase>,
}

impl RtpMemReader {
    pub fn data(&self) -> &Arc<RtpMemData> {
        &self.data
    }

    pub async fn pace_read(&mut self,) -> Option<RtpMemPacket> {
        let r = self.read_next();
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

            return Some(packet.clone())
        }

        None
    }

    pub fn read_next(&mut self) -> Option<RtpMemPacket> {
        self.data.read_at(&mut self.cursor)
    }
}

pub struct RtpMemData {
    sdp: Bytes,
    video: Option<MemTrack>,
    audio: Option<MemTrack>,
}

impl RtpMemData {
    pub fn sdp(&self) -> &Bytes {
        &self.sdp
    }

    pub fn track_index_of(&self, control: &str) -> Option<usize> {
        if let Some(track) = self.video.as_ref() {
            if control == track.control {
                return Some(track.info.index)
            }
        }

        if let Some(track) = self.audio.as_ref() {
            if control == track.control {
                return Some(track.info.index)
            }
        }

        None
    }

    pub fn read_at(&self, cursor: &mut RtpMemCursor) -> Option<RtpMemPacket> {
        let video = self.video.as_ref().map(|x|x.packets.get(cursor.video)).unwrap_or(None);

        let audio = self.audio.as_ref().map(|x|x.packets.get(cursor.audio)).unwrap_or(None);
        
        if let (Some(video), Some(audio)) = (video, audio) {
            if video.ts <= audio.ts {
                cursor.video += 1;
                return Some(video.clone())
            } else {
                cursor.audio += 1;
                return Some(audio.clone())
            }
        } else if let Some(video) = video {

            cursor.video += 1;
            return Some(video.clone())

        } else if let Some(audio) = audio {

            cursor.audio += 1;
            return Some(audio.clone())
        } else {
            return None
        }
    }

    pub fn make_reader(self: &Arc<Self>) -> RtpMemReader {
        RtpMemReader {
            data: self.clone(),
            cursor: Default::default(),
            ts_base: None,
        }
    }

    // pub fn make_source(self: &Arc<Self>) -> RtpMemSource {
    //     RtpMemSource {
    //         data: self.clone(),
    //         video_cursor: 0,
    //         audio_cursor: 0,
    //         ts_base: None,
    //     }
    // }
}

#[derive(Default, Debug)]
pub struct RtpMemCursor {
    video: usize,
    audio: usize,
}

pub async fn load_rtp_data(filename: &str, max_frames: u64) -> Result<Arc<RtpMemData>> {
    // let filename = "/tmp/sample-data/sample.mp4";
    let source_name = "MemSource";
    let source_descriptor = MediaDescriptor::File(filename.into());

    // 因为 .h264 不支持 reader.seek_to_start(); 这里创建了两个reader

    let mut video_reader = make_reader_with_sane_settings(source_descriptor.clone().into()).await?;

    let mut audio_reader = make_reader_with_sane_settings(source_descriptor.clone().into()).await?;

    let r = tokio::task::spawn_blocking(move || {

        let video = match best_media_index(&video_reader, ffmpeg_next::media::Type::Video) {
            Some(index) => {
                // let _r = video_reader.seek_to_start();
                Some(load_track(&mut video_reader, index, "Video", max_frames)?)
            },
            None => None,
        };
    
        let audio = match best_media_index(&audio_reader, ffmpeg_next::media::Type::Audio) {
            Some(index) => {
                // TODO： 
                //   按道理 audio_reader 是新创建的，不需要 seek_to_start
                //   但实际上，如果不 seek_to_start ， mp4 文件读出来的时间戳是负数
                let _r = audio_reader.seek_to_start();
                Some(load_track(&mut audio_reader, index, "Audio", max_frames)?)
            },
            None => None,
        };
    
        let sdp = make_sdp(source_name, &video, &audio)?;
    
    
    
        Ok(Arc::new(RtpMemData {
            sdp: sdp.to_string().into(),
            video,
            audio,
        }))
    }).await?;
    r

}

fn load_track(reader: &mut Reader, index: usize, name: &str, max_frames: u64) -> Result<MemTrack> {

    // let val = reader
    // .input
    // .stream(index).with_context(||"NOT found stream")?
    // .time_base();
    // println!("track[{index}]: time_base {val:?}");

    let info = reader.stream_info(index)?;
    let control = format!("streamid={index}");

    let mut muxer = RtpMuxer::new()?
    .with_stream(info.clone())?;

    let packets = {
        let mut packets = Vec::new();

        for num in 0..max_frames {
            let frame = match reader.read(index){
                Ok(v) => v,
                Err(_e) => break,
            };
            let pts = Duration::from(frame.pts());
            println!("{name} frame {num}: pts {}, dts {}", pts.as_millis(), frame.dts().as_secs());
            let rtp_packets = muxer.mux(frame)?;
            packets.extend(rtp_packets.into_iter().map(|x| RtpMemPacket::from_rtp_buf(index, pts.clone(), x)));
        }
        println!("{name} rtp packets {}", packets.len());
        packets
    };

    Ok(MemTrack {
        info,
        control,
        packets,
    })
}


fn best_media_index(reader: &Reader, kind: ffmpeg_next::media::Type) -> Option<usize> {
    reader
        .input
        .streams()
        .best(kind)
        .map(|x|x.index())
}

struct MemTrack {
    info: StreamInfo,
    control: String,
    packets: Vec<RtpMemPacket>,
}



fn make_sdp(name: &str, video: &Option<MemTrack>, audio: &Option<MemTrack>) -> Result<Sdp> {

    const ORIGIN_DUMMY_HOST: [u8; 4] = [0, 0, 0, 0];
    const TARGET_DUMMY_HOST: [u8; 4] = [0, 0, 0, 0];
    const TARGET_DUMMY_PORT: u16 = 0;

    const FMT_RTP_PAYLOAD_DYNAMIC: usize = 96;

    let mut format = FMT_RTP_PAYLOAD_DYNAMIC;

    let mut sdp = Sdp::new(
        ORIGIN_DUMMY_HOST.into(),
        name.to_string(),
        TARGET_DUMMY_HOST.into(),
        TimeRange::Live,
    );

    if let Some(track) = video {

        let muxer = RtpMuxer::new()?
        .with_stream(track.info.clone())?;

        let (sps, pps) = muxer
        .parameter_sets_h264()
        .into_iter()
        // The `parameter_sets` function will return an error if the
        // underlying stream codec is not supported, we filter out
        // the stream in that case, and return `CodecNotSupported`.
        .filter_map(Result::ok)
        .next()
        .with_context(||"video is NOT H264")?;

        let codec_info = CodecInfo::h264(sps, pps.as_slice(), muxer.packetization_mode());

        sdp = sdp.with_media(
            Kind::Video,
            TARGET_DUMMY_PORT,
            Protocol::RtpAvp,
            codec_info,
            Direction::ReceiveOnly,
        );

        if let Some(last) = sdp.media.last_mut() {
            last.tags.push(oddity_sdp_protocol::Tag::Property(format!("control:{}", track.control)));
            last.format = format;
            format +=1;
        }
    }

    if let Some(track) = audio {
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
                oddity_sdp_protocol::Tag::Property(format!("control:{}", track.control)),
                // oddity_sdp_protocol::Tag::Property("control:streamid=1".into()),
                // oddity_sdp_protocol::Tag::Property("control:1234".into()),
            ],
        });
        // format += 1;
    }

    Ok(sdp)
}


#[tokio::test]
async fn test_poc() -> Result<()> {
    let file_url = "/tmp/sample-data/sample.mp4";
    // let file_url = "/tmp/output.h264";
    let mut reader = load_rtp_data(file_url, 16).await.unwrap().make_reader();

    let mut num = 0_u64;
    while let Some(mem) = reader.read_next() {
        num += 1;
        let milli = mem.ts.as_millis();
        let ch_id = mem.ch_id;
        let data_len = mem.data.len();
        println!("rtp {num}: milli {milli}, ch_id {ch_id}, len {data_len}");
    }

    Ok(())
}

#[test]
fn test_ffmpeg_input() {
    let file_url = "/tmp/sample-data/sample.mp4";
    let max_frames = 16_u64;

    let mut input = ffmpeg_next::format::input_with_dictionary(&file_url, Default::default()).unwrap();

    // input
    //     .seek(i64::min_value(), ..)
    //     .unwrap();

    for num in 0..max_frames {
        let (stream, packet) = input.packets().next().unwrap();
        println!("Packet No.{num}: track {}, pts {:?}, dts {:?}", stream.index(), packet.pts(), packet.dts());
    }
}

