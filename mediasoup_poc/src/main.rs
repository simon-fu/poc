
use anyhow::Result;


pub(crate) mod init;
pub(crate) mod cmd_server;


fn main() -> Result<()> {
    return init::init_log_and_async(async move {
        let cmd = 1;
        match cmd {
            1 => cmd_server::run().await,
            _ => Ok(())
        }
    })?

    // return init::init_log_and_async(sub_rooms::run())?
}


