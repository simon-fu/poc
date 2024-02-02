use std::num::{NonZeroU32, NonZeroU8};

use anyhow::{anyhow, Context, Result};
use mediasoup::{router::RouterOptions, rtp_parameters::{MimeTypeAudio, MimeTypeVideo, RtcpFeedback, RtpCodecCapability, RtpCodecParametersParameters}, worker::{WorkerLogLevel, WorkerLogTag, WorkerSettings}, worker_manager::WorkerManager};
use tracing::debug;

pub async fn run() -> Result<()> {
    debug!("hello produce");
    let worker_mgr = WorkerManager::new();

    let worker = {
        let mut settings = WorkerSettings::default();
        settings.log_level = WorkerLogLevel::Debug;
        settings.log_tags = vec![
            WorkerLogTag::Info,
            WorkerLogTag::Ice,
            WorkerLogTag::Dtls,
            WorkerLogTag::Rtp,
            WorkerLogTag::Srtp,
            WorkerLogTag::Rtcp,
            WorkerLogTag::Rtx,
            WorkerLogTag::Bwe,
            WorkerLogTag::Score,
            WorkerLogTag::Simulcast,
            WorkerLogTag::Svc,
            WorkerLogTag::Sctp,
            WorkerLogTag::Message,
        ];

        worker_mgr.create_worker(settings).await
        .with_context(||"create worker failed")?
    };

    let router = worker.create_router(RouterOptions::new(media_codecs())).await
    .map_err(|e|anyhow!("create router failed. {e:?}"))?;

    let rtp_capa = router.rtp_capabilities();

    debug!("rtp capabilities: {rtp_capa:#?}");
    
    Ok(())
}


fn media_codecs() -> Vec<RtpCodecCapability> {
    vec![
        RtpCodecCapability::Audio {
            mime_type: MimeTypeAudio::Opus,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(48000).unwrap(),
            channels: NonZeroU8::new(2).unwrap(),
            parameters: RtpCodecParametersParameters::from([("useinbandfec", 1_u32.into())]),
            rtcp_feedback: vec![RtcpFeedback::TransportCc],
        },
        RtpCodecCapability::Video {
            mime_type: MimeTypeVideo::Vp8,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(90000).unwrap(),
            parameters: RtpCodecParametersParameters::default(),
            rtcp_feedback: vec![
                RtcpFeedback::Nack,
                RtcpFeedback::NackPli,
                RtcpFeedback::CcmFir,
                RtcpFeedback::GoogRemb,
                RtcpFeedback::TransportCc,
            ],
        },
    ]
}

