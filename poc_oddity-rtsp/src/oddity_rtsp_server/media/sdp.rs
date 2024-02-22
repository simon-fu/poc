use std::error;
use std::fmt;

use oddity_sdp_protocol::{CodecInfo, Direction, Kind, Protocol, TimeRange};

use crate::oddity_rtsp_server as thiz_root;
use thiz_root::media::video::reader;
use thiz_root::media::video::rtp_muxer;
use thiz_root::media::MediaDescriptor;

pub use oddity_sdp_protocol::Sdp;

/// Create a new SDP description for the given media descriptor. The
/// SDP contents can be used over RTSP when the client requested a
/// stream description.
///
/// Note: This function only handles the most appropriate video stream
/// and tosses any audio or other streams.
///
/// # Arguments
///
/// * `name` - Name of stream.
/// * `descriptor` - Media stream descriptor.
pub async fn create(name: &str, descriptor: &MediaDescriptor) -> Result<Sdp, SdpError> {
    const ORIGIN_DUMMY_HOST: [u8; 4] = [0, 0, 0, 0];
    const TARGET_DUMMY_HOST: [u8; 4] = [0, 0, 0, 0];
    const TARGET_DUMMY_PORT: u16 = 0;

    tracing::trace!("sdp: initializing reader");
    let reader = reader::backend::make_reader_with_sane_settings(descriptor.clone().into())
        .await
        .map_err(SdpError::Media)?;
    let best_video_stream = reader.best_video_stream_index().map_err(SdpError::Media)?;

    let best_audio_stream = reader
        .input
        .streams()
        .best(ffmpeg_next::media::Type::Audio)
        .ok_or(ffmpeg_next::Error::StreamNotFound)
        .map_err(video_rs::Error::from)
        .map_err(SdpError::Media)?
        .index();

    tracing::debug!(best_video_stream, best_audio_stream, "sdp: initialized reader");

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

    let sdp = sdp.with_media(
        Kind::Video,
        TARGET_DUMMY_PORT,
        Protocol::RtpAvp,
        codec_info,
        Direction::ReceiveOnly,
    );
    // sdp.media[0].tags.push(oddity_sdp_protocol::Tag::Property("control:streamid=0".into()));
    
    // let format = 97;
    // sdp.media.push(oddity_sdp_protocol::Media {
    //     kind: Kind::Audio,
    //     port: TARGET_DUMMY_PORT,
    //     protocol: Protocol::RtpAvp,
    //     format,
    //     tags: vec![
    //         oddity_sdp_protocol::Tag::Value(
    //             "rtpmap".to_string(),
    //             format!("{format} {}", "MPEG4-GENERIC/48000/2"),
    //         ),
    //         oddity_sdp_protocol::Tag::Value(
    //             "fmtp".to_string(),
    //             format!(
    //                 "{format} {}", 
    //                 "profile-level-id=1;mode=AAC-hbr;sizelength=13;indexlength=3;indexdeltalength=3; config=119056E500"
    //             ),
    //         ),
    //         oddity_sdp_protocol::Tag::Property(Direction::ReceiveOnly.to_string()),
    //         // oddity_sdp_protocol::Tag::Property("control:streamid=1".into()),
    //     ],
    // });

    tracing::trace!(%sdp, "generated sdp");
    Ok(sdp)
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
