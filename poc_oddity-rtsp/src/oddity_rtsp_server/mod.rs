// from crate oddity-rtsp, commit 27480007

pub mod app;
mod media;
mod net;
mod runtime;
mod session;
mod source;


use std::process;


use app::config::AppConfig;
pub use app::App;

pub use app::config as rtsp_server_config;

use tokio::signal::ctrl_c;


macro_rules! on_error_exit {
    ($expr:expr) => {
        match $expr {
            Ok(ret) => ret,
            Err(err) => {
                println!("\x1b[1m\x1b[91mError:\x1b[0m {}", err);
                process::exit(1);
            }
        }
    };
}


pub async fn run(config: AppConfig) {
    // on_error_exit!(initialize_tracing());
    // on_error_exit!(initialize_media());

    // let config = on_error_exit!(initialize_and_read_config());
    // tracing::debug!(?config, "loaded config file");

    tracing::trace!("starting app");
    let mut app = on_error_exit!(App::start(config).await);
    tracing::trace!("started app");

    tracing::trace!("waiting for ctrl+C...");
    on_error_exit!(ctrl_c().await);

    tracing::trace!("stopping app");
    app.stop().await;
    tracing::trace!("stopped app");
}

// fn initialize_tracing() -> Result<(), Box<dyn Error + Send + Sync>> {
//     tracing_subscriber::fmt()
//         .with_env_filter(tracing_subscriber::EnvFilter::from_env("LOG"))
//         .try_init()
// }

// fn initialize_media() -> Result<(), Box<dyn Error>> {
//     video::init()
// }

// fn initialize_and_read_config() -> Result<AppConfig, ConfigError> {
//     let config_file = env::args().nth(1).unwrap_or("default.yaml".to_string());
//     let config_file = Path::new(&config_file);
//     tracing::trace!(config_file=%config_file.display(), "loading config");

//     AppConfig::from_file(config_file)
// }
