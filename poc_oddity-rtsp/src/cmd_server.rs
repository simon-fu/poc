
use anyhow::Result;
use tracing::debug;

use crate::oddity_rtsp_server::rtsp_server_config as cfg;

pub async fn run() -> Result<()> {
    debug!("helloworld");

    let cmd = 2;
    match cmd {
        1 => run_oddity().await?,
        2 => super::simple_rtsp_server::run_demo().await?,
        _ => {},
    }
    
    Ok(())
}

async fn run_oddity() -> Result<()> {
    debug!("helloworld");

    let config = cfg::AppConfig {
        server: cfg::Server {
            host: "127.0.0.1".into(),
            port: 5554,
        },
        media: vec![
            cfg::Item { 
                name: "Big Buck Bunny".into(), 
                path: "/example".into(), 
                kind: cfg::MediaKind::File, 
                // source: "https://storage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4".into(),
                source: "/tmp/sample-data/sample.mp4".into(),
            }, 
        ],
    };

    crate::oddity_rtsp_server::run(config).await;
    
    Ok(())
}