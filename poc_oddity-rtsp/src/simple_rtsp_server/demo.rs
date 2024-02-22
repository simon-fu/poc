
use std::sync::Arc;
use anyhow::{Context, Result};
use futures::Future;
use tokio::net::TcpListener;
use tracing::info;
use super::{mem_source::{load_rtp_data, RtpMemData, RtpMemSource}, run_simple_rtsp_server, RtspPlayDesc, RtspServerCallback};

pub async fn run_demo() -> Result<()> {
    let listen_addr = "0.0.0.0:5554";
    let file_url = "/tmp/sample-data/sample.mp4";

    
    let example = Example {
        data: load_rtp_data(file_url).await?,
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
                source: self.data.make_source(),
            }))
        };

        async move {
            result
        }

    }
}

