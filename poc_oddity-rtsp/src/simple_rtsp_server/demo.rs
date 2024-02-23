
use std::sync::Arc;
use anyhow::{Context, Result};
use futures::Future;
use tokio::net::TcpListener;
use tracing::info;
use super::{run_simple_rtsp_server, RtpChPacket, RtspMediaSource, RtspPlayDesc, RtspServerCallback};

use super::rtp_mem::{load_rtp_data, RtpMemData, RtpMemReader};

pub async fn run_demo() -> Result<()> {
    let listen_addr = "0.0.0.0:5554";
    let file_url = "/tmp/sample-data/sample.mp4";
    // let file_url = "/tmp/output.h264";


    let example = Example {
        data: load_rtp_data(file_url, u64::MAX).await?,
    };

    let listener = TcpListener::bind(listen_addr).await
    .with_context(||format!("listen faild, [{listen_addr}]"))?;

    info!("rtsp listen on [{listen_addr}]");

    run_simple_rtsp_server(listener, example).await
}

struct Example {
    data: Arc<RtpMemData>,
}

impl RtspServerCallback for Example {
    type MediaSource = RtpMemSource;

    fn on_request_play(&self, path: &str) -> impl Future<Output = Result<Option<RtspPlayDesc<Self::MediaSource>>>> + Send + Sync + 'static {
        
        let result = if path != "/example" {
            Ok(None)
        } else {
            Ok(Some(RtspPlayDesc {
                sdp: self.data.sdp().clone(), // hardcode_sdp_content(),
                num_tracks: 2,
                source: RtpMemSource {
                    reader: self.data.make_reader(),
                }
            }))
        };

        async move {
            result
        }

    }
}

pub struct RtpMemSource {
    reader: RtpMemReader,
} 


impl RtspMediaSource for RtpMemSource {
    fn on_setup_track(&mut self, control: &str) -> Option<usize> {
        self.reader.data().track_index_of(control)
    }
    
    fn on_start_play(&mut self) -> impl Future<Output = Result<()>> + Send + Sync {
        async {
            Ok(())
        }
    }

    // Must be CANCEL SAFETY
    fn read_outbound_rtp(&mut self) -> impl Future<Output = Result<Option<RtpChPacket>> > + Send + Sync {
        async {
            let r = self.reader.pace_read().await
            .map(|x|RtpChPacket {
                ch_id: x.ch_id(),
                data: x.data().clone(),
            });
            Ok(r)
            // loop {
            //     tokio::time::sleep(std::time::Duration::MAX/4).await;
            // }
        }
        
    }

    fn on_inbound_rtp(&mut self, packet: RtpChPacket) -> impl Future<Output = Result<()>> + Send + Sync {
        async move {
            tracing::debug!("received rtp, {packet}");
            Ok(())
        }
    }
}